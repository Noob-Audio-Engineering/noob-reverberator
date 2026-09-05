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

/// A delay line has to return what was put into it, at the delay asked for.
///
/// Read at whole numbers of samples the answer is a stored value and the test
/// is trivial; read between them it is the interpolator, and the interpolator
/// is where an off-by-one hides. A Lagrange interpolator of any order
/// reproduces a straight line exactly, so a ramp pushed through and read back
/// at fractional delays has an answer that is known in closed form --- which
/// is the only kind of test worth writing here, because a plausible-looking
/// waveform is exactly what an off-by-one produces.
#[test]
fn a_fractional_read_lands_where_it_says() {
    use super::delay::Delay;
    let mut line = Delay::with_capacity(64);
    // A ramp, so the value at any delay is known: pushing 0,1,2,... means the
    // sample `d` back is `n - d`.
    for n in 0..40 {
        line.push(n as f32);
    }
    let last = 39.0f32;
    for &d in &[1.0f32, 1.5, 2.0, 2.25, 7.0, 7.5, 12.75, 20.0] {
        let got = line.read(d);
        let want = last - d;
        assert!(
            (got - want).abs() < 1e-3,
            "read({d}) gave {got}, and the ramp says {want}"
        );
    }
}

/// An allpass must be an allpass: whatever goes in comes out at the same
/// level, at every frequency. A chain of them is how this reverb turns a few
/// echoes into a diffuse sound, and one whose gain is not flat would colour
/// every mode that used it.
#[test]
fn the_allpass_passes_every_frequency_at_the_same_level() {
    use super::delay::Allpass;
    use std::f32::consts::TAU;
    for &f in &[100.0f32, 700.0, 3_000.0, 9_000.0] {
        let mut ap = Allpass::new(512);
        ap.set(97.0, 0.7);
        // Long enough for the transient to leave, then measure over whole
        // cycles so the average is not reading a partial one.
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        for n in 0..20_000 {
            let x = (TAU * f * n as f32 / 48_000.0).sin();
            let y = ap.process(x);
            if n > 10_000 {
                peak_in = peak_in.max(x.abs());
                peak_out = peak_out.max(y.abs());
            }
        }
        let db = 20.0 * (peak_out / peak_in).log10();
        assert!(
            db.abs() < 0.1,
            "{f} Hz came out {db:.3} dB from where it went in"
        );
    }
}

/// The claim, end to end: run an impulse through the network and read the
/// decay back out of the audio.
///
/// This shares nothing with the fit. The fit reads its own filters; this
/// filters the tail into octaves and integrates the energy backwards, the way
/// a room is measured. If the two ever disagree, the audio is right.
#[test]
fn the_tail_decays_at_the_rate_the_curve_asks_for() {
    use super::fdn::{Fdn, prime_lengths};
    use super::measure::t60_in_band;

    const FS: f32 = 48_000.0;
    let mut curve = Curve {
        base: 2.0,
        ..Default::default()
    };
    curve.bands[0] = Band {
        on: true,
        shape: Shape::LowShelf,
        freq: 250.0,
        mult: 2.0,
        width: 1.0,
    };
    curve.bands[1] = Band {
        on: true,
        shape: Shape::HighShelf,
        freq: 5_000.0,
        mult: 0.5,
        width: 1.0,
    };

    let mut fdn = Fdn::new(8, 8192, FS);
    let mut lens = Vec::new();
    prime_lengths(8, 900.0, 2_400.0, &mut lens);
    fdn.set_lengths(&lens, &curve);

    // Six seconds: the longest band asks for four, and the measurement needs
    // to see thirty-five decibels of it.
    let n = (FS * 6.0) as usize;
    let mut ir = vec![0.0f32; n];
    let mut inject = vec![0.0f32; 8];
    let mut out = vec![0.0f32; 8];
    for (i, sample) in ir.iter_mut().enumerate() {
        let x = if i == 0 { 1.0 } else { 0.0 };
        for (k, v) in inject.iter_mut().enumerate() {
            // Alternating signs so the impulse does not arrive in phase on
            // every line and cancel through the mix.
            *v = if k % 2 == 0 { x } else { -x };
        }
        fdn.process(&inject, &mut out);
        *sample = out.iter().sum::<f32>() * 0.25;
    }

    let mut worst = 0.0f32;
    let mut worst_at = 0.0f32;
    for &f in &[125.0f32, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0] {
        let Some(got) = t60_in_band(&ir, FS, f) else {
            panic!("{f} Hz band did not decay far enough to be measured");
        };
        // Against the curve averaged over the same octave the band-pass
        // covers, not against a point on it --- the measurement cannot see a
        // point, and comparing it to one is comparing two different things.
        let want = curve.t60_over_octave(f);
        let err = (got / want).log2().abs();
        println!("  {f:>7.0} Hz  asked {want:>6.3} s  measured {got:>6.3} s  ({err:.4} oct)");
        if err > worst {
            worst = err;
            worst_at = f;
        }
    }
    println!("worst {worst:.4} octaves at {worst_at:.0} Hz");
    // A quarter of an octave. Wider than the fit's own error on purpose, and
    // the reason is a property of the measurement rather than slack:
    //
    // Every band except one lands inside 0.075 octaves. The exception is the
    // band sitting on the shelf's corner, at 0.16, and it is long rather than
    // short. A band-passed energy decay is a sum of exponentials, and a `T30`
    // fitted to it leans towards the **slowest** of them, because those are
    // what is left by the time the curve has fallen thirty decibels. Where the
    // curve is steep, one octave holds decays a factor of two apart and the
    // slow side wins. Averaging the target over the octave --- which this test
    // does --- corrects the centre of the band but not that bias.
    //
    // It could be chased by modelling the expected energy curve and fitting it
    // the same way, and I have not, because that model would share its
    // assumptions with the engine and the measurement would stop being
    // independent of the thing it is measuring. Tightening this is a real
    // improvement; widening it is giving up on the claim.
    assert!(
        worst < 0.25,
        "worst {worst:.4} octaves of decay time, at {worst_at:.0} Hz"
    );
}

/// The plate is a different topology, so it gets the same measurement rather
/// than the network's result by association.
#[test]
fn the_plate_decays_at_the_rate_the_curve_asks_for() {
    use super::measure::t60_in_band;
    use super::plate::Plate;

    const FS: f32 = 48_000.0;
    let mut curve = Curve {
        base: 2.5,
        ..Default::default()
    };
    curve.bands[0] = Band {
        on: true,
        shape: Shape::HighShelf,
        freq: 4_000.0,
        mult: 0.4,
        width: 1.0,
    };

    let mut plate = Plate::new(FS);
    plate.set(1.0, 0.7, &curve);

    let n = (FS * 7.0) as usize;
    let mut ir = vec![0.0f32; n];
    for (i, s) in ir.iter_mut().enumerate() {
        let (l, r) = plate.process(if i == 0 { 1.0 } else { 0.0 });
        *s = 0.5 * (l + r);
    }

    let mut worst = 0.0f32;
    let mut worst_at = 0.0f32;
    for &f in &[250.0f32, 1_000.0, 4_000.0, 8_000.0] {
        let Some(got) = t60_in_band(&ir, FS, f) else {
            panic!("{f} Hz band did not decay far enough to be measured");
        };
        let want = curve.t60_over_octave(f);
        let err = (got / want).log2().abs();
        println!("  plate {f:>6.0} Hz  asked {want:>6.3} s  measured {got:>6.3} s  ({err:.4} oct)");
        if err > worst {
            worst = err;
            worst_at = f;
        }
    }
    // Looser than the network's, and the plate's own module says why: its
    // loop runs through allpasses whose delay depends on frequency, and the
    // loss is fitted to a single number for the lap. Worst measured is 0.20.
    assert!(
        worst < 0.25,
        "worst {worst:.4} octaves of decay time, at {worst_at:.0} Hz"
    );
}

/// Nothing may run away. Every architecture is a feedback loop, and the whole
/// reason the loss is clamped and the allpass gains are held below one is that
/// a reverb which grows is not a reverb --- it is a fault that arrives after
/// the listener has stopped watching the meter.
#[test]
fn no_architecture_runs_away_at_the_longest_decay() {
    use super::fdn::{Fdn, prime_lengths};
    use super::plate::Plate;

    const FS: f32 = 48_000.0;
    // The longest decay the curve allows, bent as hard as the bands go in the
    // direction that lengthens it.
    let mut curve = Curve {
        base: super::decay::MAX_T60,
        ..Default::default()
    };
    for b in curve.bands.iter_mut() {
        *b = Band {
            on: true,
            shape: Shape::Bell,
            freq: 1_000.0,
            mult: 8.0,
            width: 2.0,
        };
    }

    let mut fdn = Fdn::new(16, 8192, FS);
    let mut lens = Vec::new();
    prime_lengths(16, 500.0, 3_000.0, &mut lens);
    fdn.set_lengths(&lens, &curve);
    let mut inject = vec![0.0f32; 16];
    let mut out = vec![0.0f32; 16];
    let mut peak = 0.0f32;
    for i in 0..(FS as usize * 20) {
        let x = if i < 64 { 1.0 } else { 0.0 };
        inject.iter_mut().for_each(|v| *v = x);
        fdn.process(&inject, &mut out);
        peak = peak.max(out.iter().fold(0.0f32, |a, b| a.max(b.abs())));
        assert!(peak.is_finite(), "the network stopped being a number");
    }
    assert!(peak < 1e3, "the network grew to {peak}");

    let mut plate = Plate::new(FS);
    plate.set(2.0, 1.0, &curve);
    let mut peak = 0.0f32;
    for i in 0..(FS as usize * 20) {
        let (l, r) = plate.process(if i < 64 { 1.0 } else { 0.0 });
        peak = peak.max(l.abs()).max(r.abs());
        assert!(peak.is_finite(), "the plate stopped being a number");
    }
    assert!(peak < 1e3, "the plate grew to {peak}");
}

/// The shifter has to shift, and by how much the window costs it.
///
/// Measured by counting zero crossings rather than by taking an FFT peak: the
/// crossfade puts sidebands a few hertz from the fundamental, close enough
/// that a peak can land on one and read tens of cents out.
///
/// The window is a real trade and this is where its size is recorded, not
/// guessed. A tap crosses a short window often, so its warble is fast and
/// strong and the pitch it delivers is off; a long window is accurate and
/// smears the signal in time. Measured, transposing an octave:
///
/// | window | error |
/// |---|---|
/// | 1,024 | 90 cents |
/// | 4,096 | 27 cents |
/// | 16,384 | 7 cents |
///
/// Inside a reverb loop the smearing does not matter --- the diffusers have
/// already smeared everything --- so the long window is the one the shimmer
/// modes use, and this asserts the accuracy there.
#[test]
fn the_shifter_transposes_by_what_it_was_asked_for() {
    use super::pitch::Shifter;
    use std::f32::consts::TAU;

    const FS: f32 = 48_000.0;
    const F0: f32 = 440.0;
    const N: usize = 1 << 17;

    let measure = |semis: f32, win: f32| -> f32 {
        let mut sh = Shifter::new(32_768);
        sh.set_window(win);
        sh.set_semitones(semis);
        let mut crossings = 0usize;
        let mut prev = 0.0f32;
        let mut counted = 0usize;
        for i in 0..N * 2 {
            let x = (TAU * F0 * i as f32 / FS).sin();
            let y = sh.process(x);
            if i >= N {
                if prev <= 0.0 && y > 0.0 {
                    crossings += 1;
                }
                counted += 1;
                prev = y;
            }
        }
        let got = crossings as f32 * FS / counted as f32;
        1200.0 * (got / (F0 * 2f32.powf(semis / 12.0))).log2()
    };

    for &win in &[1_024.0f32, 4_096.0, 16_384.0] {
        println!(
            "  window {win:>6.0}: -12 st {:>+6.1} cents, +12 st {:>+6.1} cents",
            measure(-12.0, win),
            measure(12.0, win)
        );
    }

    for &semis in &[-24.0f32, -12.0, -5.0, 7.0, 12.0, 24.0] {
        let cents = measure(semis, 16_384.0);
        println!("  {semis:>+5.0} st at the long window: {cents:>+6.1} cents");
        // A quarter of a semitone at the long window. The two-octave
        // intervals are the worst of it --- the tap has further to travel per
        // sample, so it crosses the window more often --- and 21 cents on a
        // voice two octaves down, inside a reverb tail that is already
        // detuned by its own modulation, is not a pitch anybody is checking
        // against a tuner. What it must not be is an interval other than the
        // one asked for, which is what the sign error gave.
        assert!(
            cents.abs() < 25.0,
            "{semis} semitones came out {cents:.1} cents away"
        );
    }
}
