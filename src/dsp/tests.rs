//! What the engine is asked to prove about itself.

use super::decay::{Band, Curve, Shape};
use super::loss::Loss;

const FS: f32 = 48_000.0;

/// A flat curve is the identity case, and it has an exact answer: whatever
/// line you put it on, the tail lasts the base time.
#[test]
fn a_flat_curve_gives_the_base_decay_on_every_line() {
    let curve = Curve {
        base: 2.5,
        ..Default::default()
    };
    for m in [113usize, 691, 1789, 4093, 9973] {
        let mut loss = Loss::default();
        loss.fit(FS, m, &curve);
        for f in [50.0f32, 200.0, 1_000.0, 5_000.0, 15_000.0] {
            let got = loss.t60(FS, m, f);
            let err = (got - curve.base).abs() / curve.base;
            assert!(
                err < 1e-3,
                "line {m}, {f} Hz: asked {} s, got {got} s",
                curve.base
            );
        }
    }
}

/// The sign. A band asking for twice the decay must lengthen the tail, not
/// shorten it --- this is the one mistake in the derivation that would still
/// produce a plausible-sounding reverb, so it is worth its own test.
#[test]
fn a_band_asking_for_more_decay_gets_more() {
    let mut curve = Curve {
        base: 2.0,
        ..Default::default()
    };
    curve.bands[0] = Band {
        on: true,
        shape: Shape::Bell,
        freq: 200.0,
        mult: 2.0,
        width: 1.0,
    };
    let m = 1789;
    let mut loss = Loss::default();
    loss.fit(FS, m, &curve);
    let at_band = loss.t60(FS, m, 200.0);
    let far_off = loss.t60(FS, m, 8_000.0);
    assert!(
        at_band > far_off * 1.5,
        "200 Hz should ring far longer than 8 kHz: {at_band} s against {far_off} s"
    );
}

/// The claim the plug-in is about, on one line: the decay realised at a
/// frequency is the decay the curve asks for there.
#[test]
fn the_realised_decay_follows_the_drawn_curve() {
    let mut curve = Curve {
        base: 3.0,
        ..Default::default()
    };
    curve.bands[0] = Band {
        on: true,
        shape: Shape::LowShelf,
        freq: 250.0,
        mult: 2.5,
        width: 1.0,
    };
    curve.bands[1] = Band {
        on: true,
        shape: Shape::HighShelf,
        freq: 6_000.0,
        mult: 0.35,
        width: 1.0,
    };
    let mut worst = 0.0f32;
    let mut worst_at = 0.0f32;
    for m in [401usize, 1237, 3571, 8191] {
        let mut loss = Loss::default();
        loss.fit(FS, m, &curve);
        let mut f = super::loss::FIT_BOTTOM;
        while f < super::loss::FIT_TOP {
            let want = curve.t60(f);
            let got = loss.t60(FS, m, f);
            let err = (got / want).log2().abs();
            if err > worst {
                worst = err;
                worst_at = f;
            }
            f *= 1.1;
        }
    }
    println!("worst {worst:.5} octaves at {worst_at:.0} Hz");
    // Just above what the fit measures today, so a regression fails rather
    // than sitting comfortably inside a loose bound. It is here to be
    // tightened by a better fit and never widened by a worse one.
    assert!(
        worst < 0.02,
        "worst error {worst:.4} octaves of decay time, at {worst_at:.0} Hz"
    );
}
