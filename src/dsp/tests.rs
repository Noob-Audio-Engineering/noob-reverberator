//! What the engine is asked to prove about itself.

use super::decay::{Band, Curve, Shape};
use super::loss::{Loss, Scratch};

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
        let mut scratch = Scratch::default();
        loss.fit(FS, m, &curve, &mut scratch);
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
    let mut scratch = Scratch::default();
    loss.fit(FS, m, &curve, &mut scratch);
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
        let mut scratch = Scratch::default();
        loss.fit(FS, m, &curve, &mut scratch);
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

/// The tape's heads have to land where the heads are.
///
/// A multi-head echo whose repeats are evenly spaced is a plain delay with a
/// shorter time --- it sounds like an echo either way, which is why this
/// measures where the repeats actually arrive rather than trusting the taps.
#[test]
fn the_tape_repeats_at_its_heads() {
    use super::tape::Tape;

    const FS: f32 = 48_000.0;
    let mut tape = Tape::new(FS, 2_200.0);
    // Wobble off, so an arrival is where it was put and not where it drifted.
    tape.set(400.0, 4, 0.0, 0.0, 0.0);

    let n = (FS * 0.6) as usize;
    let mut ir = vec![0.0f32; n];
    for (i, s) in ir.iter_mut().enumerate() {
        *s = tape.process(if i == 0 { 1.0 } else { 0.0 });
    }

    // Where the energy actually is: every sample above a tenth of the peak.
    let peak = ir.iter().fold(0.0f32, |a, b| a.max(b.abs()));
    let mut arrivals: Vec<f32> = Vec::new();
    let mut i = 0;
    while i < n {
        if ir[i].abs() > peak * 0.1 {
            arrivals.push(i as f32 / FS * 1000.0);
            i += (FS * 0.02) as usize; // one arrival per burst
        } else {
            i += 1;
        }
    }
    println!("  tape arrivals (ms): {arrivals:?}");
    // Four heads at a quarter, a half, three quarters and all of 400 ms.
    let want = [100.0f32, 200.0, 300.0, 400.0];
    assert_eq!(
        arrivals.len(),
        want.len(),
        "expected four repeats, got {arrivals:?}"
    );
    for (got, w) in arrivals.iter().zip(want.iter()) {
        assert!(
            (got - w).abs() < 2.0,
            "a head landed at {got:.1} ms and belongs at {w:.1} ms"
        );
    }
}

/// The choir has to put its resonances where the vowel's formants are, and
/// leave them there when the pitch of what it is filtering changes --- which
/// is the whole difference between a vowel and a filter sweep.
#[test]
fn the_choir_puts_its_formants_where_the_vowel_is() {
    use super::formant::Choir;
    use std::f32::consts::TAU;

    const FS: f32 = 48_000.0;

    // Measured by driving it with a sine at each frequency and reading the
    // level out: a formant is a place where more comes back than went in.
    let gain_at = |vowel: f32, f: f32| -> f32 {
        let mut c = Choir::new(FS);
        c.set(vowel, 0.0, 1.0, 1.0);
        let mut peak = 0.0f32;
        for i in 0..12_000 {
            let x = (TAU * f * i as f32 / FS).sin();
            let (l, _) = c.process(x, x);
            if i > 6_000 {
                peak = peak.max(l.abs());
            }
        }
        20.0 * peak.max(1e-9).log10()
    };

    // "Ah" is 730 and 1090 Hz; "Ee" is 270 and 2290. Each should be louder at
    // its own first formant than the other vowel is there.
    let ah_at_730 = gain_at(0.0, 730.0);
    let ee_at_730 = gain_at(2.0, 730.0);
    let ee_at_2290 = gain_at(2.0, 2_290.0);
    let ah_at_2290 = gain_at(0.0, 2_290.0);
    println!("  Ah@730 {ah_at_730:.1} dB, Ee@730 {ee_at_730:.1} dB");
    println!("  Ee@2290 {ee_at_2290:.1} dB, Ah@2290 {ah_at_2290:.1} dB");
    assert!(
        ah_at_730 > ee_at_730 + 3.0,
        "Ah should be louder than Ee at 730 Hz: {ah_at_730:.1} against {ee_at_730:.1}"
    );
    assert!(
        ee_at_2290 > ah_at_2290 + 3.0,
        "Ee should be louder than Ah at 2290 Hz: {ee_at_2290:.1} against {ah_at_2290:.1}"
    );
}

/// Every preset has to name things that exist.
///
/// A preset is a mode name and a list of parameter ids, both written by hand.
/// A typo in either produces a button that half works --- the mode changes and
/// one control does not, or nothing happens at all --- and it is exactly the
/// kind of fault nobody finds, because a reverb after a preset always sounds
/// like *something*.
#[test]
fn every_preset_names_a_mode_and_parameters_that_exist() {
    use super::mode::MODES;
    use super::params::param_specs;
    use super::preset::PRESETS;

    let ids: Vec<String> = param_specs().into_iter().map(|s| s.id).collect();
    for p in PRESETS {
        assert!(
            MODES.iter().any(|m| m.name == p.mode),
            "preset \"{}\" starts from mode \"{}\", which is not in the table",
            p.name,
            p.mode
        );
        for (id, v) in p.set {
            assert!(
                ids.iter().any(|k| k == id),
                "preset \"{}\" sets \"{id}\", which is not a parameter",
                p.name
            );
            let spec = param_specs()
                .into_iter()
                .find(|s| &s.id == id)
                .expect("just checked it is there");
            assert!(
                *v >= spec.min - 1e-4 && *v <= spec.max + 1e-4,
                "preset \"{}\" sets \"{id}\" to {v}, outside its range {}..{}",
                p.name,
                spec.min,
                spec.max
            );
        }
    }
    // And each one has to be a different sound from the mode it starts from,
    // or it is a name over a mode. A preset built entirely on controls that
    // turned out to be unwired --- which several of these nearly were, since
    // Bloom, Ducking, Auto Gate, Thickness and the attack fade all reached no
    // stage at all until recently --- resolves perfectly and sounds like the
    // bare mode, and everything above this line would pass.
    for p in PRESETS {
        let i = MODES
            .iter()
            .position(|m| m.name == p.mode)
            .expect("just checked the mode is there");
        let mut bare = plain_settings(i);
        super::engine::Reverb::apply_mode(&mut bare, i);
        bare.mix = 1.0;

        let mut with = bare;
        for (id, v) in p.set {
            apply_one(&mut with, id, *v);
        }
        // `mix` is set by nearly every preset and is a level, so a preset that
        // moved nothing else would still differ. Held equal, this compares
        // the reverb rather than how much of it there is.
        with.mix = 1.0;

        let a = render_engine(&bare, 1.5);
        let b = render_engine(&with, 1.5);
        let diff = energy(
            &a.iter()
                .zip(b.iter())
                .map(|(x, y)| x - y)
                .collect::<Vec<_>>(),
        );
        assert!(
            diff / energy(&a).max(1e-9) > 1e-3,
            "preset \"{}\" sounds like plain \"{}\" --- every control it moves \
             is either unwired or does nothing at these settings",
            p.name,
            p.mode
        );
    }

    let json = super::preset::factory_json();
    let n = json.as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(n, PRESETS.len(), "factory_json dropped presets");
    println!(
        "  {} presets, all resolving and none of them its bare mode; \
         factory_json carries {n}",
        PRESETS.len()
    );
}

/// Put one preset entry on to a [`Settings`], in the plain units the preset
/// table is written in.
///
/// The engine reads these through the parameter bridge, which divides the
/// percentages; a preset says `("duck", 70.0)` and the engine wants `0.7`.
fn apply_one(s: &mut super::engine::Settings, id: &str, v: f32) {
    use super::colour::Era;
    use super::shape::Kind as ShapeKind;
    match id {
        "mix" => s.mix = v / 100.0,
        "output" => s.output = v / 100.0,
        "decay" => {
            s.decay = v;
            s.curve.base = v;
        }
        "size" => s.size = v,
        "density" => s.density = v / 100.0,
        "attack" => s.attack_ms = v,
        "predelay" => s.predelay_ms = v,
        "predelay_sync" => {
            s.predelay_sync = v as usize;
            s.tempo = Some(120.0);
        }
        "predelay_offset" => s.predelay_offset = v,
        "width" => s.width = v / 100.0,
        "mod_rate" => s.mod_rate = v,
        "mod_depth" => s.mod_depth = v / 100.0,
        "mod_random" => s.mod_random = v / 100.0,
        "mod_chaos" => s.mod_chaos = v / 100.0,
        "early_level" => s.early_level = v / 100.0,
        "early_size" => s.early_size = v,
        "early_absorb" => s.early_absorb = v / 100.0,
        "early_taps" => s.early_taps = v as usize,
        "shift" => s.shift = v,
        "shift_mix" => s.shift_mix = v / 100.0,
        "shift_window" => s.shift_window = v,
        "shape" => s.shape = ShapeKind::from_index(v as usize),
        "shape_hold" => s.shape_hold = v,
        "era" => s.era = Era::from_index(v as usize),
        "era_amount" => s.era_amount = v / 100.0,
        "tension" => s.tension = v / 100.0,
        "sections" => s.sections = v as usize,
        "lines" => s.lines = v as usize,
        "tape_mix" => s.tape_mix = v / 100.0,
        "tape_time" => s.tape_time = v,
        "tape_heads" => s.tape_heads = v as usize,
        "tape_feedback" => s.tape_feedback = v / 100.0,
        "tape_wobble" => s.tape_wobble = v / 100.0,
        "tape_drive" => s.tape_drive = v / 100.0,
        "choir_amount" => s.choir_amount = v / 100.0,
        "choir_vowel" => s.choir_vowel = v,
        "choir_spread" => s.choir_spread = v / 100.0,
        "choir_resonance" => s.choir_resonance = v / 100.0,
        "distance" => s.distance = v / 100.0,
        "thickness" => s.thickness = v / 100.0,
        "duck" => s.duck = v / 100.0,
        "duck_release" => s.duck_release = v,
        "gate" => s.gate = v / 100.0,
        "gate_hold" => s.gate_hold = v,
        "bloom" => s.bloom = v / 100.0,
        "bloom_time" => s.bloom_time = v,
        "bloom_swell" => s.bloom_swell = v,
        _ => {
            let rest = id
                .strip_prefix("band")
                .unwrap_or_else(|| panic!("a preset sets `{id}`, which this test cannot apply"));
            let (n, field) = rest.split_once('_').expect("bandN_field");
            let b = &mut s.curve.bands[n.parse::<usize>().expect("a band number") - 1];
            match field {
                "on" => b.on = v > 0.5,
                "shape" => b.shape = super::decay::Shape::from_index(v as usize),
                "freq" => b.freq = v,
                "mult" => b.mult = v,
                "width" => b.width = v,
                _ => panic!("a preset sets `{id}`, whose field this test cannot apply"),
            }
        }
    }
}

/// A tilt has to tilt: longer at the bottom, shorter at the top, from one
/// control.
///
/// It is the first shape here that needs **two** filters on every delay line,
/// and the first whose amount is a difference between two ends rather than a
/// value at a centre. Both are easy to get half right in a way that still
/// produces a reverb, so this measures the tail rather than the intent.
#[test]
fn a_tilt_lengthens_one_end_and_shortens_the_other() {
    use super::fdn::{Fdn, prime_lengths};
    use super::measure::t60_in_band;

    const FS: f32 = 48_000.0;
    let mut curve = Curve {
        base: 2.0,
        ..Default::default()
    };
    curve.bands[0] = Band {
        on: true,
        shape: Shape::Tilt,
        freq: 800.0,
        // Four to one between the ends.
        mult: 4.0,
        width: 1.0,
    };

    // What the curve itself claims, before any filter is involved. A positive
    // tilt brightens --- the shared crate's convention, and Noob-Q's --- so
    // the top rings longer, and the number on the control is the **ratio
    // between the ends**.
    let lo = curve.t60(60.0);
    let hi = curve.t60(12_000.0);
    println!(
        "  drawn: {lo:.3} s at 60 Hz, {hi:.3} s at 12 kHz, ratio {:.2}",
        hi / lo
    );
    assert!(hi > curve.base, "the top should be longer than the base");
    assert!(
        lo < curve.base,
        "the bottom should be shorter than the base"
    );
    assert!(
        (hi / lo / 4.0 - 1.0).abs() < 0.12,
        "a tilt of four should span four between its ends, and spans {:.2}",
        hi / lo
    );

    let mut fdn = Fdn::new(8, 8192, FS);
    let mut lens = Vec::new();
    prime_lengths(8, 900.0, 2_400.0, &mut lens);
    fdn.set_lengths(&lens, &curve);

    let n = (FS * 8.0) as usize;
    let mut ir = vec![0.0f32; n];
    let mut inject = vec![0.0f32; 8];
    let mut out = vec![0.0f32; 8];
    for (i, s) in ir.iter_mut().enumerate() {
        let x = if i == 0 { 1.0 } else { 0.0 };
        for (k, v) in inject.iter_mut().enumerate() {
            *v = if k % 2 == 0 { x } else { -x };
        }
        fdn.process(&inject, &mut out);
        *s = out.iter().sum::<f32>() * 0.25;
    }

    let mut worst = 0.0f32;
    for &f in &[125.0f32, 500.0, 2_000.0, 8_000.0] {
        let Some(got) = t60_in_band(&ir, FS, f) else {
            panic!("{f} Hz did not decay far enough to be measured");
        };
        let want = curve.t60_over_octave(f);
        let err = (got / want).log2().abs();
        println!("  tilt {f:>5.0} Hz  asked {want:>6.3} s  measured {got:>6.3} s  ({err:.3} oct)");
        worst = worst.max(err);
    }
    assert!(worst < 0.25, "worst {worst:.3} octaves");
}

/// Ducking has to pull the wet down **while the dry is loud** and let it back
/// as the dry falls away. Getting it backwards, or driving it from the wet
/// instead, still produces a reverb that moves --- so this measures which way
/// round it moves.
#[test]
fn ducking_gets_out_of_the_way_and_comes_back() {
    use super::duck::Ducker;

    const FS: f32 = 48_000.0;
    let mut d = Ducker::new(FS);
    d.set(0.8, 5.0, 120.0);

    // A second of loud dry, then a second of silence.
    let mut during = 1.0f32;
    let mut after = 0.0f32;
    for i in 0..(FS as usize * 2) {
        let dry = if i < FS as usize { 0.9 } else { 0.0 };
        let g = d.next(dry);
        // The last tenth of each half, once the detector has settled.
        if i > (FS * 0.9) as usize && i < FS as usize {
            during = during.min(g);
        }
        if i > (FS * 1.9) as usize {
            after = after.max(g);
        }
    }
    println!("  ducking: {during:.3} while loud, {after:.3} after");
    assert!(
        during < 0.4,
        "it should duck while the dry is loud: {during:.3}"
    );
    assert!(after > 0.95, "it should come back afterwards: {after:.3}");
}

/// The gate has to close on the tail once the signal has gone, and its
/// threshold has to follow the signal rather than being an absolute number
/// --- so the same performance printed twelve decibels quieter gates the
/// same way.
#[test]
fn the_gate_closes_after_the_signal_and_follows_its_level() {
    use super::duck::AutoGate;

    const FS: f32 = 48_000.0;
    let run = |level: f32| -> (f32, f32) {
        let mut g = AutoGate::new(FS);
        g.set(1.0, 100.0);
        let mut open = 0.0f32;
        let mut shut = 1.0f32;
        for i in 0..(FS as usize * 2) {
            let dry = if i < FS as usize { level } else { 0.0 };
            let v = g.next(dry);
            if i > (FS * 0.9) as usize && i < FS as usize {
                open = open.max(v);
            }
            if i > (FS * 1.9) as usize {
                shut = shut.min(v);
            }
        }
        (open, shut)
    };

    let (open_loud, shut_loud) = run(0.9);
    let (open_quiet, shut_quiet) = run(0.9 / 4.0); // twelve decibels down
    println!("  gate loud : open {open_loud:.3}, shut {shut_loud:.3}");
    println!("  gate quiet: open {open_quiet:.3}, shut {shut_quiet:.3}");
    assert!(
        open_loud > 0.95 && open_quiet > 0.95,
        "it must open for both"
    );
    assert!(
        shut_loud < 0.05 && shut_quiet < 0.05,
        "it must shut for both"
    );
}

/// A bloom has to **grow**, and it grows the way the machine is played: while
/// a note is held.
///
/// My first attempt at this test fed a fifty-millisecond burst and asked for
/// the output to be louder later than sooner. It never can be. The generator
/// passes the input through its delay at full level and only scales what
/// comes *back*, so the first repeat is as loud as the note and no later one
/// can exceed it --- that would need a feedback above unity, which is not a
/// bloom, it is a fault. What accumulates is energy over successive passes,
/// and a burst gives it nothing to accumulate.
///
/// So this holds a sound, which is what somebody does with the machine, and
/// asks whether **energy accumulates**.
///
/// With noise and not a tone, and by RMS and not a peak. A tone through a
/// feedback loop is a comb, and where the tone sits between the comb's teeth
/// decides the level far more than the feedback does --- measured with a
/// 220 Hz note through a 90 ms loop, raising the feedback *lowered* the peak,
/// because the note moved towards a null. That is the measurement reporting on
/// where one frequency happens to land, not on whether the machine builds.
#[test]
fn a_bloom_grows_while_a_note_is_held() {
    use super::bloom::Bloom;

    const FS: f32 = 48_000.0;
    let run = |amount: f32, swell_ms: f32| -> (f32, f32) {
        let mut b = Bloom::new(FS, 800.0);
        b.set(amount, 90.0, swell_ms);
        let n = (FS * 1.5) as usize;
        let mut out = vec![0.0f32; n];
        let mut rng = 0x1234_5678u32;
        for (i, o) in out.iter_mut().enumerate() {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let noise = (rng >> 8) as f32 / 8_388_608.0 - 1.0;
            // A quarter of a second of silence first, so there is an onset to
            // trigger the swell rather than the run starting mid-sound.
            let x = if i > (FS * 0.25) as usize {
                noise * 0.25
            } else {
                0.0
            };
            *o = b.process(x);
        }
        let rms = |from: f32, to: f32| {
            let a = (FS * from) as usize;
            let z = ((FS * to) as usize).min(n);
            let s: f64 = out[a..z].iter().map(|v| (*v as f64) * (*v as f64)).sum();
            (s / (z - a) as f64).sqrt() as f32
        };
        (rms(0.30, 0.40), rms(1.1, 1.45))
    };

    let (early, late) = run(0.9, 600.0);
    println!("  bloom on : {early:.3} early, {late:.3} late");
    // A feedback reaching about 0.8 raises a broadband RMS by roughly
    // `1/sqrt(1 - g²)`, which is 1.7, and the swell is already past its peak
    // by the late window --- so 1.35 is close to what the arithmetic predicts
    // rather than a bound picked for comfort. Measured: 1.47.
    assert!(
        late > early * 1.35,
        "a held sound should build: {early:.3} then {late:.3}"
    );

    // Against the same loop with no swell at all: `amount` at nothing leaves
    // the generator out of the path entirely, so this compares growing
    // feedback against none.
    let (flat_early, flat_late) = run(0.0, 600.0);
    println!("  bloom off: {flat_early:.3} early, {flat_late:.3} late");
    let grew = late / early.max(1e-6);
    let flat = flat_late / flat_early.max(1e-6);
    assert!(
        grew > flat * 1.4,
        "the swell should build more than a bare path: {grew:.2} against {flat:.2}"
    );
}

/// A spring has to **chirp**: high frequencies must come back before low ones.
///
/// That is the entire difference between a spring and a small dark plate, and
/// it is invisible in a decay measurement --- a chirping coil and a
/// non-chirping one decay identically. So this measures *when* each band
/// arrives, by filtering the impulse response into two bands and finding
/// where each one's energy first appears.
#[test]
fn a_spring_returns_the_top_of_the_band_first() {
    use super::spring::Spring;
    use super::svf::{Kind, Svf};

    const FS: f32 = 48_000.0;
    let curve = Curve {
        base: 2.0,
        ..Default::default()
    };
    let mut spring = Spring::new(FS, 120.0);
    // A slack coil: the more dispersion, the further apart the two arrivals.
    spring.set(0.85, 140, 30.0, &curve);

    let n = (FS * 0.5) as usize;
    let mut ir = vec![0.0f32; n];
    for (i, s) in ir.iter_mut().enumerate() {
        *s = spring.process(if i == 0 { 1.0 } else { 0.0 });
    }

    // When does a band's energy first reach a tenth of its own peak?
    let arrival = |f: f32| -> f32 {
        let mut a = Svf::default();
        let mut b = Svf::default();
        a.set(Kind::BandPass, FS, f, 1.2, 0.0);
        b.set(Kind::BandPass, FS, f, 1.2, 0.0);
        let band: Vec<f32> = ir.iter().map(|&x| b.process(a.process(x))).collect();
        let peak = band.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let at = band
            .iter()
            .position(|v| v.abs() > peak * 0.1)
            .unwrap_or(band.len());
        at as f32 / FS * 1000.0
    };

    let high = arrival(4_000.0);
    let low = arrival(300.0);
    println!("  spring: 4 kHz at {high:.1} ms, 300 Hz at {low:.1} ms");
    assert!(
        low > high + 1.0,
        "the bottom of the band should arrive later than the top: {low:.1} ms against {high:.1} ms"
    );

    // And a tight coil should chirp less than a slack one, which is what the
    // Tension control is for.
    let mut tight = Spring::new(FS, 120.0);
    tight.set(0.15, 140, 30.0, &curve);
    let mut ir2 = vec![0.0f32; n];
    for (i, s) in ir2.iter_mut().enumerate() {
        *s = tight.process(if i == 0 { 1.0 } else { 0.0 });
    }
    let spread_of = |ir: &[f32]| {
        let one = |f: f32| {
            let mut a = Svf::default();
            let mut b = Svf::default();
            a.set(Kind::BandPass, FS, f, 1.2, 0.0);
            b.set(Kind::BandPass, FS, f, 1.2, 0.0);
            let band: Vec<f32> = ir.iter().map(|&x| b.process(a.process(x))).collect();
            let peak = band.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            band.iter().position(|v| v.abs() > peak * 0.1).unwrap_or(0) as f32 / FS * 1000.0
        };
        one(300.0) - one(4_000.0)
    };
    let slack_spread = low - high;
    let tight_spread = spread_of(&ir2);
    println!("  spread: slack {slack_spread:.1} ms, tight {tight_spread:.1} ms");
    assert!(
        slack_spread > tight_spread,
        "a slacker coil should chirp further: {slack_spread:.1} against {tight_spread:.1}"
    );
}

/// Diffusion has to make the echoes **denser**, which is not the same as
/// making them quieter or later.
///
/// Measured as the crest factor of the impulse response: a handful of
/// discrete repeats is a few tall spikes over silence, and a diffuse burst is
/// many small ones. A chain that only attenuated would keep its shape and
/// lower everything, so this compares the ratio of the peak to the energy
/// rather than either alone.
#[test]
fn diffusion_turns_a_few_echoes_into_many() {
    use super::diffuse::Diffuser;

    let crest = |amount: f32| -> f32 {
        let mut d = Diffuser::new(6, 2_048, 1);
        d.set_scale(1.0, 900.0);
        d.set_amount(amount);
        let n = 8_192;
        let mut out = vec![0.0f32; n];
        for (i, o) in out.iter_mut().enumerate() {
            *o = d.process(if i == 0 { 1.0 } else { 0.0 });
        }
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let rms = (out.iter().map(|v| v * v).sum::<f32>() / n as f32).sqrt();
        peak / rms.max(1e-9)
    };

    let sparse = crest(0.0);
    let dense = crest(1.0);
    println!("  crest factor: sparse {sparse:.1}, dense {dense:.1}");
    assert!(
        dense < sparse * 0.7,
        "diffusion should flatten the burst: {sparse:.1} then {dense:.1}"
    );
}

/// The two kinds of modulation have to be two kinds.
///
/// A sweep is periodic: its own value a cycle later is nearly itself. A random
/// walk is not, and that is the whole distinction the survey draws --- one
/// detunes the tail and one does not. Measured by correlating each against
/// itself one period later.
#[test]
fn a_sweep_repeats_and_a_walk_does_not() {
    use super::modulate::Modulator;

    const FS: f32 = 48_000.0;
    const RATE: f32 = 2.0;
    let period = (FS / RATE) as usize;

    let correlation = |random: f32| -> f32 {
        let mut m = Modulator::new(7);
        m.set_rate(FS, RATE);
        let n = period * 6;
        let v: Vec<f32> = (0..n).map(|_| m.next(1.0, random, 0.0)).collect();
        // Against itself one period later, over the settled part.
        let a = &v[period..n - period];
        let b = &v[period * 2..n];
        let num: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let da: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let db: f32 = b.iter().map(|y| y * y).sum::<f32>().sqrt();
        num / (da * db).max(1e-9)
    };

    let sweep = correlation(0.0);
    let walk = correlation(1.0);
    println!("  correlation one period later: sweep {sweep:.3}, walk {walk:.3}");
    assert!(sweep > 0.95, "a sweep should repeat: {sweep:.3}");
    assert!(walk < 0.5, "a walk should not: {walk:.3}");
}

/// Each envelope shape has to be the shape it is named after.
///
/// They are all one gain over time, so a wrong one still produces a reverb
/// that moves --- gate and ramp differ only in whether the middle is flat, and
/// reverse and swell only in whether the end falls. Measured by driving an
/// onset and reading the gain at three points.
#[test]
fn every_envelope_shape_has_the_shape_it_is_named_after() {
    use super::shape::{Kind as ShapeKind, Shaper};

    const FS: f32 = 48_000.0;
    const HOLD: f32 = 400.0;

    // The gain a fifth, a half and nine tenths of the way through the shape,
    // and well past the end of it.
    let trace = |kind: ShapeKind| -> [f32; 4] {
        let mut sh = Shaper::new(FS);
        sh.set(kind, HOLD);
        let hold_n = (HOLD * 0.001 * FS) as usize;
        let n = hold_n * 3;
        let mut at = [0.0f32; 4];
        let marks = [
            (hold_n as f32 * 0.2) as usize,
            hold_n / 2,
            (hold_n as f32 * 0.9) as usize,
            hold_n * 2,
        ];
        // A short burst to trigger it, then silence: the shape runs on from
        // the onset whatever happens afterwards.
        for i in 0..n {
            let x = if i < 64 { 0.8 } else { 0.0 };
            let g = sh.next(x);
            for (k, &m) in marks.iter().enumerate() {
                if i == m {
                    at[k] = g;
                }
            }
        }
        at
    };

    let gate = trace(ShapeKind::Gate);
    let reverse = trace(ShapeKind::Reverse);
    let ramp = trace(ShapeKind::Ramp);
    let swoosh = trace(ShapeKind::Swoosh);
    let swell = trace(ShapeKind::Swell);
    let off = trace(ShapeKind::Off);
    println!("  gate    {gate:.2?}");
    println!("  reverse {reverse:.2?}");
    println!("  ramp    {ramp:.2?}");
    println!("  swoosh  {swoosh:.2?}");
    println!("  swell   {swell:.2?}");
    println!("  off     {off:.2?}");

    // Off is one throughout, or it is not off.
    assert!(
        off.iter().all(|g| (g - 1.0).abs() < 0.01),
        "off moved: {off:.2?}"
    );
    // A gate holds up and then drops.
    assert!(gate[1] > 0.9 && gate[3] < 0.1, "gate: {gate:.2?}");
    // Reverse climbs into the note and holds.
    assert!(
        reverse[0] < 0.2 && reverse[2] > 0.6 && reverse[3] > 0.9,
        "reverse: {reverse:.2?}"
    );
    // A ramp only falls.
    assert!(
        ramp[0] > 0.6 && ramp[2] < 0.3 && ramp[3] < 0.1,
        "ramp: {ramp:.2?}"
    );
    // A swoosh goes up and comes back.
    assert!(
        swoosh[1] > swoosh[0] && swoosh[1] > swoosh[3],
        "swoosh: {swoosh:.2?}"
    );
    // A swell arrives and stays.
    assert!(
        swell[0] < 0.4 && swell[2] > 0.7 && swell[3] > 0.9,
        "swell: {swell:.2?}"
    );
}

/// The eras have to do what the control says they do: narrow the band, and
/// coarsen the signal.
///
/// Both, separately. An era that only rolled the top off would be a tone
/// control, and one that only quantised would be a distortion; the survey's
/// whole point about Valhalla's COLOR is that it is a bandwidth *and* a word
/// length applied to whatever topology is running.
#[test]
fn the_eras_narrow_the_band_and_coarsen_the_signal() {
    use super::colour::{Colour, Era};
    use std::f32::consts::TAU;

    const FS: f32 = 48_000.0;

    // How much of a sine at `f` survives.
    let through = |era: Era, f: f32| -> f32 {
        let mut c = Colour::new(FS);
        c.set(era, 1.0, FS);
        let mut peak = 0.0f32;
        for i in 0..8_000 {
            let x = (TAU * f * i as f32 / FS).sin() * 0.5;
            let (l, _) = c.process(x, x);
            if i > 4_000 {
                peak = peak.max(l.abs());
            }
        }
        20.0 * (peak / 0.5).max(1e-9).log10()
    };

    for era in [Era::Seventies, Era::Eighties, Era::Broken] {
        let low = through(era, 500.0);
        let high = through(era, 18_000.0);
        println!("  {era:?}: {low:.1} dB at 500 Hz, {high:.1} dB at 18 kHz");
        assert!(low > -6.0, "{era:?} should pass the middle: {low:.1} dB");
        assert!(
            high < low - 12.0,
            "{era:?} should narrow the band: {low:.1} then {high:.1}"
        );
    }
    // And "Now" is not an era: it does nothing at all.
    let now_low = through(Era::Now, 500.0);
    let now_high = through(Era::Now, 18_000.0);
    println!("  Now: {now_low:.1} dB at 500 Hz, {now_high:.1} dB at 18 kHz");
    assert!(
        now_low.abs() < 0.2 && now_high.abs() < 0.2,
        "Now coloured something"
    );

    // The word length: a signal far below the step must be quantised to
    // nothing, which an era that only filtered would leave alone.
    let quantised = |era: Era| -> f32 {
        let mut c = Colour::new(FS);
        c.set(era, 1.0, FS);
        let mut peak = 0.0f32;
        for i in 0..4_000 {
            // Well under one step of twelve bits.
            let x = (TAU * 300.0 * i as f32 / FS).sin() * 1e-4;
            let (l, _) = c.process(x, x);
            if i > 2_000 {
                peak = peak.max(l.abs());
            }
        }
        peak
    };
    let tiny_seventies = quantised(Era::Seventies);
    let tiny_now = quantised(Era::Now);
    println!("  a signal under one step: seventies {tiny_seventies:.2e}, now {tiny_now:.2e}");
    assert!(
        tiny_seventies < tiny_now * 0.5,
        "the word length should swallow it: {tiny_seventies:.2e} against {tiny_now:.2e}"
    );
}

/// The early reflections have to arrive when a room that size would deliver
/// them, and there have to be the number that were asked for.
///
/// A pattern of the right density at the wrong times is a chorus, not a room,
/// and nothing else here would notice: the tail measures the same either way.
#[test]
fn the_early_pattern_matches_the_room_it_was_given() {
    use super::early::Early;

    const FS: f32 = 48_000.0;
    const C: f32 = 343.0;

    let first_arrival = |size_m: f32| -> f32 {
        let mut e = Early::new(FS, 400.0);
        e.build(size_m, 0.2, 24, 1.0);
        let n = (FS * 0.4) as usize;
        let mut at = 0.0f32;
        for i in 0..n {
            let (l, r) = e.process(if i == 0 { 1.0 } else { 0.0 });
            if l.abs() + r.abs() > 1e-4 {
                at = i as f32 / FS * 1000.0;
                break;
            }
        }
        at
    };

    // A bigger room's first wall is further away, so its first reflection is
    // later --- and roughly in proportion, because sound travels at one speed.
    let small = first_arrival(4.0);
    let big = first_arrival(24.0);
    println!("  first reflection: 4 m room {small:.1} ms, 24 m room {big:.1} ms");
    assert!(small > 0.0 && big > small * 3.0, "{small:.1} then {big:.1}");
    // The shortest path in a room whose longest wall is `L` cannot be shorter
    // than about a tenth of a wall, nor longer than a couple of them.
    for (size, at) in [(4.0f32, small), (24.0, big)] {
        let ms_per_metre = 1000.0 / C;
        assert!(
            at > size * ms_per_metre * 0.1 && at < size * ms_per_metre * 3.0,
            "a {size} m room answered in {at:.1} ms"
        );
    }

    // And the count is the count.
    for want in [1usize, 8, 24, 64] {
        let mut e = Early::new(FS, 400.0);
        e.build(10.0, 0.3, want, 1.0);
        assert_eq!(e.taps(), want, "asked for {want} taps");
    }
}

/// The state variable filters have to agree with an independent implementation
/// of the same shapes.
///
/// `noob-band-shapes` carries the analogue prototypes and knows nothing about
/// this engine, so it is something for these filters to disagree with --- and
/// the reverb's whole claim rests on their magnitude being right, because the
/// decay is read straight out of it.
#[test]
fn the_loss_filters_agree_with_the_shared_prototypes() {
    use super::svf::{Kind, Svf};
    use noob_band_shapes::{Kind as BandKind, SHELF_Q, magnitude_db};

    const FS: f32 = 48_000.0;
    let pairs = [
        (Kind::Bell, BandKind::Bell, 1.0f32, "bell"),
        (Kind::LowShelf, BandKind::LowShelf, SHELF_Q, "low shelf"),
        (Kind::HighShelf, BandKind::HighShelf, SHELF_Q, "high shelf"),
    ];
    let mut worst = 0.0f32;
    let mut worst_at = ("", 0.0f32);
    for (k, bk, q, name) in pairs {
        for &f0 in &[80.0f32, 1_000.0, 6_000.0] {
            for &g in &[-12.0f32, -3.0, 3.0, 12.0] {
                let mut s = Svf::default();
                s.set(k, FS, f0, q, g);
                let mut f = 20.0f32;
                while f < FS / 20.0 {
                    let mine = 20.0 * s.magnitude(FS, f).max(1e-12).log10();
                    // At the frequency the digital filter actually realises,
                    // not at the one it was asked for. A topology-preserving
                    // transform maps the analogue prototype onto the digital
                    // one through `tan`, so comparing at the raw frequency
                    // measures the bilinear warping instead of the filter ---
                    // it read 0.21 dB apart at 2.3 kHz, all of it warping.
                    let warp = |x: f32| (std::f32::consts::PI * x / FS).tan();
                    let equiv = f0 * warp(f) / warp(f0);
                    let theirs = magnitude_db(bk, equiv, f0, q, g);
                    let e = (mine - theirs).abs();
                    if e > worst {
                        worst = e;
                        worst_at = (name, f);
                    }
                    f *= 1.1;
                }
            }
        }
    }
    println!(
        "  worst {worst:.4} dB, on a {} at {:.0} Hz",
        worst_at.0, worst_at.1
    );
    assert!(worst < 0.05, "{worst:.3} dB apart on a {}", worst_at.0);
}

/// Doing the fit on another thread has to give the same filters as doing it
/// here.
///
/// The split is the whole reason the audio thread is cheap now, and it is the
/// kind of change that can quietly alter the sound: the worker measures with
/// its *own* filters, so a fit that depended on the state of the caller's
/// would come back subtly different. This checks the numbers land in the same
/// place, and that they actually arrive.
#[test]
fn the_fit_is_the_same_wherever_it_is_done() {
    use super::fitter::{Fitter, Job};
    use super::loss::{Loss, Scratch};

    const FS: f32 = 48_000.0;
    let mut curve = Curve {
        base: 3.0,
        ..Default::default()
    };
    curve.bands[0] = Band {
        on: true,
        shape: Shape::LowShelf,
        freq: 250.0,
        mult: 2.2,
        width: 1.0,
    };
    curve.bands[1] = Band {
        on: true,
        shape: Shape::HighShelf,
        freq: 5_000.0,
        mult: 0.45,
        width: 1.0,
    };

    let mut lens = [0.0f32; super::fdn::MAX_LINES];
    for (i, l) in lens.iter_mut().enumerate() {
        *l = 700.0 + i as f32 * 137.0;
    }

    // Here.
    let mut here = Loss::default();
    let mut scratch = Scratch::default();
    here.fit(FS, lens[0] as usize, &curve, &mut scratch);

    // There.
    let mut fitter = Fitter::new();
    assert!(
        fitter.request(Job {
            fs: FS,
            curve,
            lens,
            lines: 4,
            plate_laps: [9_000.0, 9_400.0],
            spring_trip: 1_600.0,
        }),
        "the worker would not take the job"
    );
    let mut done = None;
    for _ in 0..600 {
        if let Some(d) = fitter.collect() {
            done = Some(d);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let done = done.expect("the fit never came back");

    let mut there = Loss::default();
    there.apply(FS, &curve, &done.net[0]);

    let mut worst = 0.0f32;
    let mut f = 20.0f32;
    while f < 20_000.0 {
        let a = here.t60(FS, lens[0] as usize, f);
        let b = there.t60(FS, lens[0] as usize, f);
        worst = worst.max((a / b).log2().abs());
        f *= 1.1;
    }
    println!("  worst {worst:.6} octaves between the two");
    assert!(worst < 1e-4, "the two fits differ by {worst:.5} octaves");
}

/// A settings block with everything quiet, for the end-to-end tests.
fn plain_settings(mode: usize) -> super::engine::Settings {
    use super::colour::Era;
    use super::shape::Kind as ShapeKind;
    super::engine::Settings {
        bypass: false,
        mix: 1.0,
        output: 1.0,
        freeze: false,
        mode,
        decay: 2.0,
        size: 1.0,
        density: 0.8,
        attack_ms: 0.0,
        predelay_ms: 0.0,
        predelay_sync: 0,
        predelay_offset: 1.0,
        tempo: None,
        width: 1.0,
        mod_rate: 0.0,
        mod_depth: 0.0,
        mod_random: 0.0,
        mod_chaos: 0.0,
        early_level: 0.0,
        early_size: 8.0,
        early_absorb: 0.35,
        early_taps: 24,
        shift: 0.0,
        shift_mix: 0.0,
        shift_window: 16_384.0,
        shape: ShapeKind::Off,
        shape_hold: 400.0,
        era: Era::Now,
        era_amount: 0.0,
        tension: 0.6,
        sections: 100,
        lines: 12,
        distance: 0.0,
        thickness: 0.0,
        duck: 0.0,
        duck_release: 250.0,
        gate: 0.0,
        gate_hold: 300.0,
        bloom: 0.0,
        bloom_time: 120.0,
        bloom_swell: 600.0,
        tape_mix: 0.0,
        tape_time: 320.0,
        tape_heads: 3,
        tape_feedback: 0.35,
        tape_wobble: 0.3,
        tape_drive: 0.3,
        choir_amount: 0.0,
        choir_vowel: 0.0,
        choir_spread: 0.0,
        choir_resonance: 0.5,
        curve: Curve {
            base: 2.0,
            ..Default::default()
        },
    }
}

/// Run an impulse through the whole engine and give back the wet output.
fn render_engine(s: &super::engine::Settings, secs: f32) -> Vec<f32> {
    use super::engine::Reverb;
    const FS: f32 = 48_000.0;
    let mut rev = Reverb::new(FS);
    rev.configure(s);
    let n = (FS * secs) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    l[0] = 1.0;
    r[0] = 1.0;
    rev.process(s, &mut l, &mut r);
    l.iter().zip(r.iter()).map(|(a, b)| 0.5 * (a + b)).collect()
}

fn energy(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

/// **Every mode has to make a sound, and none may make an unreasonable one.**
///
/// This is the test that would have caught a mode wired to an architecture
/// that produces nothing, a fit that never arrived, or a stage that turns its
/// input into infinity --- none of which the per-module tests can see, because
/// each one is right on its own.
#[test]
fn every_mode_produces_a_finite_tail() {
    use super::mode::MODES;

    for (i, m) in MODES.iter().enumerate() {
        let mut s = plain_settings(i);
        super::engine::Reverb::apply_mode(&mut s, i);
        s.mix = 1.0;
        let out = render_engine(&s, 2.0);
        assert!(
            out.iter().all(|v| v.is_finite()),
            "{} produced something that is not a number",
            m.name
        );
        let peak = out.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak < 16.0, "{} peaked at {peak}", m.name);
        // Something has to come out. An impulse into a reverb that returns
        // silence is a reverb that is not connected.
        let e = energy(&out[480..]);
        assert!(e > 1e-7, "{} returned silence ({e:.2e})", m.name);
    }
    println!("  {} modes, all of them audible and finite", MODES.len());
}

/// The controls that act on the whole plug-in, which nothing else tests
/// because they are not an algorithm.
///
/// Each is easy to leave unwired --- the reverb still sounds like a reverb
/// with a dead Width or a Freeze that does not --- and each is the sort of
/// thing somebody notices in a session rather than in a benchmark.
#[test]
fn the_global_controls_do_what_they_say() {
    use super::engine::Reverb;

    const FS: f32 = 48_000.0;

    // Bypass: what goes in comes out, sample for sample.
    let mut s = plain_settings(0);
    s.bypass = true;
    let mut l = vec![0.0f32; 2_048];
    let mut r = vec![0.0f32; 2_048];
    l[0] = 1.0;
    r[0] = 0.5;
    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    rev.process(&s, &mut l, &mut r);
    assert_eq!(l[0], 1.0, "bypass changed the input");
    assert_eq!(r[0], 0.5, "bypass changed the input");
    assert!(l[1..].iter().all(|v| *v == 0.0), "bypass added a tail");

    // Mix: at nothing, the output is the input. At everything, it is not.
    let mut dry = plain_settings(0);
    dry.mix = 0.0;
    let out_dry = render_engine(&dry, 0.5);
    let mut wet = plain_settings(0);
    wet.mix = 1.0;
    let out_wet = render_engine(&wet, 0.5);
    let tail_dry = energy(&out_dry[480..]);
    let tail_wet = energy(&out_wet[480..]);
    println!("  tail energy: dry {tail_dry:.2e}, wet {tail_wet:.2e}");
    assert!(
        tail_dry < 1e-9,
        "a dry mix still had a tail: {tail_dry:.2e}"
    );
    assert!(tail_wet > 1e-6, "a wet mix had none: {tail_wet:.2e}");

    // Pre-delay: nothing comes back before it.
    let mut pre = plain_settings(0);
    pre.predelay_ms = 80.0;
    pre.mix = 1.0;
    let out = render_engine(&pre, 0.6);
    let before = (FS * 0.070) as usize;
    let quiet = energy(&out[64..before]);
    let after = energy(&out[(FS * 0.09) as usize..(FS * 0.2) as usize]);
    println!("  pre-delay: {quiet:.2e} before 70 ms, {after:.2e} after 90 ms");
    assert!(
        after > quiet * 20.0,
        "the pre-delay did not hold the tail off: {quiet:.2e} then {after:.2e}"
    );

    // Width: at nothing the two sides are the same signal; wider, they differ.
    let render_sides = |width: f32| -> f32 {
        let mut s = plain_settings(0);
        s.width = width;
        s.mix = 1.0;
        let mut rev = Reverb::new(FS);
        rev.configure(&s);
        let n = (FS * 0.5) as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        l[0] = 1.0;
        r[0] = 1.0;
        rev.process(&s, &mut l, &mut r);
        let side: Vec<f32> = l.iter().zip(r.iter()).map(|(a, b)| 0.5 * (a - b)).collect();
        energy(&side[480..])
    };
    let mono = render_sides(0.0);
    let stereo = render_sides(1.0);
    println!("  side energy: width 0 {mono:.2e}, width 1 {stereo:.2e}");
    assert!(mono < 1e-9, "width at nothing still had a side: {mono:.2e}");
    assert!(stereo > mono * 100.0, "width did nothing: {stereo:.2e}");

    // Freeze: the tank stops being fed. Testing that from silence would pass
    // for the wrong reason --- a frozen empty tank is silent, and so is a
    // broken one --- so this rings it first, then freezes, then plays into it
    // and checks the new note does not get in while the old tail carries on.
    let mut s = plain_settings(0);
    s.mix = 1.0;
    s.decay = 8.0;
    s.curve.base = 8.0;
    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    let n = (FS * 0.4) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    l[0] = 1.0;
    r[0] = 1.0;
    rev.process(&s, &mut l, &mut r);
    let ringing = energy(&l[n - 4_800..]);

    // Frozen, and played into.
    let mut f = s;
    f.freeze = true;
    rev.configure(&f);
    let mut l2 = vec![0.0f32; n];
    let mut r2 = vec![0.0f32; n];
    for i in 0..64 {
        l2[i] = 1.0;
        r2[i] = 1.0;
    }
    rev.process(&f, &mut l2, &mut r2);
    let after_freeze = energy(&l2[n - 4_800..]);

    // The same again, not frozen, so there is something to compare against.
    let mut rev2 = Reverb::new(FS);
    rev2.configure(&s);
    let mut l3 = vec![0.0f32; n];
    let mut r3 = vec![0.0f32; n];
    l3[0] = 1.0;
    r3[0] = 1.0;
    rev2.process(&s, &mut l3, &mut r3);
    let mut l4 = vec![0.0f32; n];
    let mut r4 = vec![0.0f32; n];
    for i in 0..64 {
        l4[i] = 1.0;
        r4[i] = 1.0;
    }
    rev2.process(&s, &mut l4, &mut r4);
    let after_open = energy(&l4[n - 4_800..]);

    println!(
        "  ringing {ringing:.2e}; then a note in: frozen {after_freeze:.2e}, open {after_open:.2e}"
    );
    assert!(ringing > 1e-6, "the tank was not ringing to begin with");
    assert!(
        after_freeze > 1e-8,
        "freezing stopped the tail as well as the input: {after_freeze:.2e}"
    );
    assert!(
        after_open > after_freeze * 2.0,
        "the new note got into a frozen tank: frozen {after_freeze:.2e}, open {after_open:.2e}"
    );
}

/// The parameter list and the engine's index table have to agree.
///
/// They are two lists written in two files: `params.rs` says what exists and
/// `engine.rs` says what the audio thread looks up. `ParamIx::new` panics on
/// an id that is not there, and without this that panic would happen the
/// first time somebody loaded the plug-in rather than here.
///
/// It also reads the settings back, so a parameter that exists but is decoded
/// into the wrong field --- a copy-and-paste in `read_settings`, which is
/// forty lines of the same shape --- shows up as a default that is not the
/// default.
#[test]
fn every_parameter_resolves_and_reads_back_its_default() {
    use super::engine::{ParamIx, read_settings};

    let (bridge, ix) = super::build_bridge("Noob Reverberator", 48_000.0, true);
    // Constructing it twice is not wasteful: the first is the panic check and
    // the second proves it is deterministic.
    let _ = ParamIx::new(&bridge);

    let audio = bridge.take_audio().expect("the audio handle is taken once");
    let s = read_settings(&audio, &ix);

    // A handful of defaults, from the spec list, read through the whole path.
    let spec = |id: &str| {
        super::params::param_specs()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("no parameter `{id}`"))
            .default
    };
    assert!(!s.bypass, "bypass should start off");
    assert!(!s.freeze, "freeze should start off");
    assert!(
        (s.mix * 100.0 - spec("mix")).abs() < 0.01,
        "mix read {}",
        s.mix
    );
    assert!(
        (s.decay - spec("decay")).abs() < 0.01,
        "decay read {}",
        s.decay
    );
    assert!(
        (s.size * 100.0 - spec("size")).abs() < 0.01,
        "size read {}",
        s.size
    );
    assert_eq!(
        s.lines,
        spec("lines").round() as usize,
        "lines read {}",
        s.lines
    );
    assert_eq!(
        s.tape_heads,
        spec("tape_heads").round() as usize,
        "tape heads read {}",
        s.tape_heads
    );
    assert!(
        (s.bloom_time - spec("bloom_time")).abs() < 0.01,
        "bloom time read {}",
        s.bloom_time
    );
    // The curve starts flat: no band on, and the base is the decay.
    assert!(
        s.curve.bands.iter().all(|b| !b.on),
        "a band started switched on"
    );
    assert!((s.curve.base - spec("decay")).abs() < 0.01);
    println!(
        "  {} parameters, all resolved and read back",
        super::params::param_specs().len()
    );
}

/// Chaos has to be its own thing, not a rougher walk or a wobblier sweep.
///
/// VintageVerb's Chaotic modes are described as wow and flutter --- an
/// unsteady transport, which is two rates at once: a drift well under the
/// sweep's and a flutter well over it. If it were on the same axis as the
/// other two, blending it in would only make one of them noisier, and there
/// would be no reason for the control to exist.
///
/// Measured by where the modulation's own energy sits in frequency. A sweep
/// puts it all at the rate it was set to; chaos puts it somewhere else
/// entirely, at both ends.
#[test]
fn chaos_moves_at_rates_the_sweep_does_not() {
    use super::modulate::Modulator;
    use rustfft::{FftPlanner, num_complex::Complex};

    const FS: f32 = 48_000.0;
    const RATE: f32 = 3.0;
    const N: usize = 1 << 15;

    // The spectrum of the modulation itself, decimated so a fraction of a
    // hertz is resolvable in a reasonable block.
    let spectrum = |chaos: f32| -> Vec<f32> {
        let mut m = Modulator::new(11);
        m.set_rate(FS, RATE);
        const DEC: usize = 64;
        let mut buf: Vec<Complex<f32>> = Vec::with_capacity(N);
        let mut acc = 0.0f32;
        for i in 0..N * DEC {
            let v = m.next(1.0, 0.0, chaos);
            acc += v;
            if i % DEC == DEC - 1 {
                buf.push(Complex::new(acc / DEC as f32, 0.0));
                acc = 0.0;
            }
        }
        buf.truncate(N);
        FftPlanner::new().plan_fft_forward(N).process(&mut buf);
        buf.iter().take(N / 2).map(|c| c.norm()).collect()
    };

    // Bin width after decimating by 64.
    let bin = FS / 64.0 / N as f32;
    let energy_between = |s: &[f32], lo: f32, hi: f32| -> f32 {
        let a = (lo / bin) as usize;
        let b = ((hi / bin) as usize).min(s.len() - 1);
        s[a..=b].iter().map(|v| v * v).sum::<f32>()
    };

    let steady = spectrum(0.0);
    let chaotic = spectrum(1.0);

    // The sweep's own rate, the drift below it, and the flutter above.
    let at_rate = |s: &[f32]| energy_between(s, RATE - 0.5, RATE + 0.5);
    let drift = |s: &[f32]| energy_between(s, 0.2, 1.2);
    let flutter = |s: &[f32]| energy_between(s, 7.0, 12.0);

    let sd = drift(&steady) / at_rate(&steady);
    let cd = drift(&chaotic) / at_rate(&chaotic);
    let sf = flutter(&steady) / at_rate(&steady);
    let cf = flutter(&chaotic) / at_rate(&chaotic);
    println!("  drift against the rate  : steady {sd:.4}, chaotic {cd:.4}");
    println!("  flutter against the rate: steady {sf:.4}, chaotic {cf:.4}");
    assert!(cd > sd * 20.0, "chaos should drift: {sd:.4} then {cd:.4}");
    assert!(cf > sf * 20.0, "chaos should flutter: {sf:.4} then {cf:.4}");
}

/// A synced pre-delay has to land on the division it names, at the tempo the
/// host reports --- and fall back to the free time when there is no tempo.
///
/// The arithmetic is four lines and every one of them is easy to get subtly
/// wrong: a beat is a quarter note, a triplet is two thirds of one and not a
/// third, and a dotted note is one and a half. All of those produce a delay
/// that sounds deliberate and is on the wrong subdivision.
#[test]
fn a_synced_predelay_lands_on_its_division() {
    use super::sync::{SYNC_NAMES, predelay_ms};

    // At 120 bpm a beat is 500 ms, which makes every division a round number.
    let bpm = Some(120.0f32);
    let want = [
        ("Free", 33.0f32), // the free value, untouched
        ("1/4", 500.0),
        ("1/4T", 333.333),
        ("1/4.", 750.0),
        ("1/8", 250.0),
        ("1/8T", 166.667),
        ("1/8.", 375.0),
        ("1/16", 125.0),
        ("1/16T", 83.333),
        ("1/16.", 187.5),
        ("1/32", 62.5),
    ];
    for (i, (name, ms)) in want.iter().enumerate() {
        assert_eq!(SYNC_NAMES[i], *name, "the names moved");
        let got = predelay_ms(i, 33.0, bpm, 1.0);
        assert!(
            (got - ms).abs() < 0.5,
            "{name} at 120 bpm gave {got:.1} ms and should give {ms:.1}"
        );
    }

    // The offset, which is what makes a synced delay usable rather than
    // merely correct: half a division early, two late.
    assert!((predelay_ms(4, 0.0, bpm, 0.5) - 125.0).abs() < 0.5);
    assert!((predelay_ms(4, 0.0, bpm, 2.0) - 500.0).abs() < 0.5);

    // Twice the tempo, half the time.
    assert!((predelay_ms(1, 0.0, Some(240.0), 1.0) - 250.0).abs() < 0.5);

    // And with nothing to sync to, the free time rather than an invented
    // tempo.
    for (i, name) in SYNC_NAMES.iter().enumerate() {
        assert!(
            (predelay_ms(i, 41.0, None, 1.0) - 41.0).abs() < 1e-6,
            "{name} invented a tempo"
        );
    }
    println!(
        "  {} divisions, all landing where they say",
        SYNC_NAMES.len()
    );
}

/// **Every mode has to decay at the length it advertises.**
///
/// The mode table is a list of claims in seconds, and nothing else checks
/// them: a mode whose architecture ignores its `decay`, or whose lines are
/// scaled so the tank is a different size than the one asked for, still comes
/// out audible and finite and would pass the test above. This one measures
/// the tail each mode actually produces and holds it against its own entry.
///
/// The tolerance is wide on purpose --- half an octave either way. The point
/// is that the number in the table governs the sound, not that a hall lands
/// on a millisecond, and the architectures reach their decay by different
/// routes.
#[test]
fn every_mode_decays_at_the_length_it_claims() {
    use super::mode::{Arch, MODES};

    const FS: f32 = 48_000.0;
    let mut checked = 0;
    let mut worst = (0.0f32, "");
    for (i, m) in MODES.iter().enumerate() {
        // Early reflections are taps and have no tail to measure; a mode that
        // says so is not making a claim this test can hold it to.
        if m.arch == Arch::Early {
            continue;
        }
        let mut s = plain_settings(i);
        super::engine::Reverb::apply_mode(&mut s, i);
        s.mix = 1.0;
        // The characters that deliberately move the tail's level over time
        // are turned off: an envelope or a swell is measured as a decay rate
        // by anything that fits a line to the level, and that is a real
        // effect rather than a wrong one.
        s.shape = super::shape::Kind::Off;
        s.bloom = 0.0;
        s.attack_ms = 0.0;
        let ir = render_engine(&s, (m.decay * 2.5).clamp(2.0, 14.0));
        let Some(t60) = super::measure::t60_in_band(&ir, FS, 1_000.0) else {
            panic!("{} gave a tail nothing could be measured on", m.name);
        };
        let octaves = (t60 / m.decay).log2().abs();
        if octaves > worst.0 {
            worst = (octaves, m.name);
        }
        assert!(
            octaves < 0.5,
            "{} asks for {:.2} s and gives {:.2} s ({:.2} octaves out)",
            m.name,
            m.decay,
            t60,
            octaves
        );
        checked += 1;
    }
    println!(
        "  {checked} modes decay as advertised, worst {:.2} octaves ({})",
        worst.0, worst.1
    );
}

/// **What a mode declares has to reach the output.**
///
/// Each mode is a row of numbers, and every one of them is a wire that can be
/// left unconnected: a `choir` that no stage reads, a `tape` the tank never
/// sees, a `chaos` that arrives at a modulator which ignores it. The engine
/// still runs, the mode still sounds like a reverb, and the panel still shows
/// the control --- the mode is simply not the mode it says it is.
///
/// So for every character a mode declares, this renders the mode twice, once
/// as written and once with that one field taken out, and requires the two to
/// differ. It is a wiring test rather than a sound test: it says the field is
/// connected to something, not that what it is connected to is good.
#[test]
fn every_character_a_mode_declares_reaches_the_output() {
    use super::engine::{Reverb, Settings};
    use super::mode::MODES;

    // Name, whether the mode declares it, and how to take it out.
    type Off = fn(&mut Settings);
    type Declared = fn(&super::mode::Mode) -> bool;
    type Character = (&'static str, Declared, Off);
    let characters: [Character; 10] = [
        ("early", |m| m.early > 0.0, |s| s.early_level = 0.0),
        ("shift", |m| m.shift_mix > 0.0, |s| s.shift_mix = 0.0),
        ("era", |m| m.era_amount > 0.0, |s| s.era_amount = 0.0),
        ("tape", |m| m.tape > 0.0, |s| s.tape_mix = 0.0),
        ("choir", |m| m.choir > 0.0, |s| s.choir_amount = 0.0),
        ("distance", |m| m.distance > 0.0, |s| s.distance = 0.0),
        ("thickness", |m| m.thickness > 0.0, |s| s.thickness = 0.0),
        ("bloom", |m| m.bloom > 0.0, |s| s.bloom = 0.0),
        ("chaos", |m| m.chaos > 0.0, |s| s.mod_chaos = 0.0),
        (
            "shape",
            |m| m.shape != super::shape::Kind::Off,
            |s| s.shape = super::shape::Kind::Off,
        ),
    ];

    let mut seen = std::collections::BTreeMap::new();
    for (i, m) in MODES.iter().enumerate() {
        let mut base = plain_settings(i);
        Reverb::apply_mode(&mut base, i);
        base.mix = 1.0;
        let with = render_engine(&base, 1.5);

        for (name, declared, remove) in characters.iter() {
            if !declared(m) {
                continue;
            }
            let mut without = base;
            remove(&mut without);
            let out = render_engine(&without, 1.5);
            let diff = energy(
                &with
                    .iter()
                    .zip(out.iter())
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>(),
            );
            let level = energy(&with).max(1e-9);
            assert!(
                diff / level > 1e-3,
                "{} declares {name} and takes it out unnoticed ({:.2e} against {:.2e})",
                m.name,
                diff,
                level
            );
            *seen.entry(*name).or_insert(0) += 1;
        }
    }

    // And every character has to be declared by somebody, or it is a field
    // with a page control and no mode behind it.
    for (name, _, _) in characters.iter() {
        assert!(seen.contains_key(name), "no mode declares {name}");
    }
    let list: Vec<String> = seen.iter().map(|(k, v)| format!("{k}x{v}")).collect();
    println!("  characters wired: {}", list.join(" "));
}

/// **Every parameter has to change the sound.**
///
/// This is the test I did not have, and four controls were dead without a
/// single failure to show for it: Bloom, Ducking, Auto Gate and Thickness all
/// arrived in `Settings`, sat in the equality check that decides whether to
/// reconfigure, and appeared on the panel with a working knob --- while
/// reaching no stage at all. Three of them even had passing unit tests,
/// because a test that builds the stage itself and calls `set` on it is right
/// about the stage and says nothing about whether the engine ever calls it.
///
/// So this one goes the other way round: it starts from the parameter list
/// the plug-in publishes, moves each parameter, and requires the output to
/// move too. A control that changes nothing is either unwired or should not
/// be on the panel.
///
/// **The match below has to stay exhaustive.** A parameter it does not know
/// how to move fails the test rather than being skipped, which is what makes
/// this hold for parameters that do not exist yet.
#[test]
fn every_parameter_changes_the_sound() {
    use super::colour::Era;
    use super::engine::{Reverb, Settings};
    use super::mode::{Arch, MODES};
    use super::shape::Kind as ShapeKind;

    // A base with every *additive* section already up, because half of these
    // parameters are the detail of a section that is off by default: Bloom
    // Time does nothing at Bloom 0, and Tape Heads does nothing with no tape,
    // so a sweep from the defaults would report them dead and be wrong.
    //
    // The three that *mute* --- the envelope shape, the ducker and the auto
    // gate --- are deliberately left off here even though they are sections
    // like the others. With them on, the tail after the input stops is
    // silenced three times over, and every parameter whose effect is in the
    // tail reads as dead: that is how this test first accused the tape's
    // feedback, whose repeats were being gated away. Each of them is switched
    // on by its own arm below instead, along with the detail control that
    // needs it.
    fn live() -> Settings {
        let mut s = plain_settings(0);
        s.mix = 1.0;
        s.decay = 2.0;
        s.curve.base = 2.0;
        s.mod_rate = 0.8;
        s.mod_depth = 0.3;
        s.mod_random = 0.4;
        s.mod_chaos = 0.4;
        s.early_level = 0.4;
        s.shift = 3.0;
        s.shift_mix = 0.4;
        s.era = Era::Eighties;
        s.era_amount = 0.5;
        s.distance = 0.4;
        s.thickness = 0.5;
        s.bloom = 0.5;
        s.tape_mix = 0.4;
        s.choir_amount = 0.5;
        s.predelay_ms = 20.0;
        s
    }

    // How to move each one. `None` for a parameter that is not swept here ---
    // `mode` is the whole table rather than one control, and is taken on its
    // own at the end.
    fn move_it(s: &mut Settings, id: &str) -> Option<()> {
        match id {
            "bypass" => s.bypass = true,
            "mix" => s.mix = 0.2,
            "output" => s.output = 0.4,
            "freeze" => s.freeze = true,
            "mode" => return None,
            "decay" => {
                s.decay = 6.0;
                s.curve.base = 6.0;
            }
            "size" => s.size = 2.5,
            "density" => s.density = 0.1,
            "attack" => s.attack_ms = 300.0,
            "predelay" => s.predelay_ms = 120.0,
            "predelay_sync" => {
                s.predelay_sync = 4;
                s.tempo = Some(120.0);
            }
            "predelay_offset" => {
                s.predelay_sync = 4;
                s.tempo = Some(120.0);
                s.predelay_offset = 1.7;
            }
            "width" => s.width = 0.0,
            "mod_rate" => s.mod_rate = 4.0,
            "mod_depth" => s.mod_depth = 0.9,
            "mod_random" => s.mod_random = 1.0,
            "mod_chaos" => s.mod_chaos = 1.0,
            "early_level" => s.early_level = 1.0,
            "early_size" => s.early_size = 30.0,
            "early_absorb" => s.early_absorb = 0.9,
            "early_taps" => s.early_taps = 60,
            "shift" => s.shift = -7.0,
            "shift_mix" => s.shift_mix = 1.0,
            "shift_window" => s.shift_window = 4_096.0,
            "shape" => s.shape = ShapeKind::Reverse,
            "shape_hold" => {
                s.shape = ShapeKind::Gate;
                s.shape_hold = 1_400.0;
            }
            "era" => s.era = Era::Seventies,
            "era_amount" => s.era_amount = 1.0,
            "tension" => s.tension = 0.15,
            "sections" => s.sections = 30,
            "lines" => s.lines = 6,
            "tape_mix" => s.tape_mix = 1.0,
            "tape_time" => s.tape_time = 90.0,
            "tape_heads" => s.tape_heads = 1,
            "tape_feedback" => s.tape_feedback = 0.8,
            "tape_wobble" => s.tape_wobble = 1.0,
            "tape_drive" => s.tape_drive = 1.0,
            "choir_amount" => s.choir_amount = 1.0,
            "choir_vowel" => s.choir_vowel = 3.0,
            "choir_spread" => s.choir_spread = 1.0,
            "choir_resonance" => s.choir_resonance = 1.0,
            "distance" => s.distance = 1.0,
            "thickness" => s.thickness = 1.0,
            "duck" => s.duck = 1.0,
            "duck_release" => {
                s.duck = 1.0;
                s.duck_release = 2_000.0;
            }
            "gate" => s.gate = 1.0,
            "gate_hold" => {
                s.gate = 1.0;
                s.gate_hold = 30.0;
            }
            "bloom" => s.bloom = 1.0,
            "bloom_time" => s.bloom_time = 400.0,
            "bloom_swell" => s.bloom_swell = 1_800.0,
            _ => {
                // The bands are generated, so they are matched by shape.
                let Some(rest) = id.strip_prefix("band") else {
                    panic!("`{id}` is a parameter this test does not know how to move");
                };
                let (n, field) = rest.split_once('_').expect("bandN_field");
                // The parameters count from one and the array from zero.
                let n: usize = n.parse().expect("a band number");
                let b = &mut s.curve.bands[n - 1];
                b.on = true;
                match field {
                    "on" => {}
                    "shape" => b.shape = super::decay::Shape::LowShelf,
                    "freq" => b.freq = 220.0,
                    "mult" => b.mult = 4.0,
                    "width" => b.width = 3.0,
                    _ => panic!("`{id}` has a field this test does not know"),
                }
                // A band that is only switched on does nothing until it also
                // asks for a decay other than the one it already has.
                if field != "mult" {
                    b.mult = 2.5;
                }
            }
        }
        Some(())
    }

    // A signal with a start and a stop, so the dynamics parameters have
    // something to act on: an impulse never lets a gate close or a ducker
    // recover, and neither does a tone that never ends.
    fn render(s: &Settings) -> Vec<f32> {
        const FS: f32 = 48_000.0;
        let n = (FS * 2.5) as usize;
        let mut rev = Reverb::new(FS);
        rev.configure(s);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let mut seed = 0x1234_5678u32;
        for i in 0..n {
            let v = if i < (FS * 0.4) as usize {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.5
            } else {
                0.0
            };
            l[i] = v;
            r[i] = v * 0.8;
        }
        rev.process(s, &mut l, &mut r);
        // Kept apart rather than summed: Width moves the two channels against
        // each other and would vanish in a mono sum.
        l.into_iter().chain(r).collect()
    }

    // Every architecture, because a parameter belonging to one of them is not
    // dead for being silent in the others: Tension is a spring's, and the
    // network has no use for it.
    let arches = [Arch::Network, Arch::Plate, Arch::Spring, Arch::Early];
    let mode_for = |a: Arch| MODES.iter().position(|m| m.arch == a).expect("an arch");

    let ids: Vec<String> = super::params::param_specs()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert!(ids.len() > 50, "only {} parameters", ids.len());

    let mut references = Vec::new();
    for a in arches {
        let mut s = live();
        // The architecture is the mode's, so this picks a mode that uses each
        // one and leaves every other setting where `live` put it.
        s.mode = mode_for(a);
        let out = render(&s);
        references.push((s, out));
    }

    let mut swept = 0;
    let mut worst: (f32, String) = (f32::MAX, String::new());
    for id in ids.iter() {
        let mut best = 0.0f32;
        let mut moved = false;
        for (base, reference) in references.iter() {
            let mut s = *base;
            if move_it(&mut s, id).is_none() {
                continue;
            }
            moved = true;
            let out = render(&s);
            let diff = energy(
                &reference
                    .iter()
                    .zip(out.iter())
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>(),
            );
            best = best.max(diff / energy(reference).max(1e-9));
        }
        if !moved {
            continue;
        }
        assert!(
            best > 1e-3,
            "`{id}` was moved and the output did not follow ({best:.2e} relative, in any \
             architecture) --- either it is not wired or it should not be a control"
        );
        if best < worst.0 {
            worst = (best, id.clone());
        }
        swept += 1;
    }

    // And the mode parameter, which is not one control but the whole table.
    // Two modes are allowed to land on the same tail level --- they differ in
    // more than level --- but most of them may not, or the table is a list of
    // names over one sound.
    let mut heard = std::collections::BTreeSet::new();
    for i in 0..MODES.len() {
        let mut s = live();
        Reverb::apply_mode(&mut s, i);
        s.mode = i;
        heard.insert((energy(&render(&s)) * 1e6) as i64);
    }
    assert!(
        heard.len() * 4 >= MODES.len() * 3,
        "{} modes produced only {} distinguishable tails",
        MODES.len(),
        heard.len()
    );

    println!(
        "  {swept} parameters swept, all audible; {} modes, {} distinct; \
         faintest was `{}` at {:.1e}",
        MODES.len(),
        heard.len(),
        worst.1,
        worst.0
    );
}

/// **A driven tank has to thicken without moving its decay.**
///
/// Thickness puts a saturator in the feedback path of all three tanks, which
/// is the one place a nonlinearity can undo the fit: the loss filters are
/// solved for a loss per trip, and that arithmetic assumes the trip is
/// linear. Squeeze the trip and every mode decays at a length other than the
/// one it asks for --- quietly, because the reverb still sounds like a
/// reverb.
///
/// So this holds the two halves of the design apart. The decay must not move
/// when the tank is driven, because [`super::drive::saturate`] has unity
/// slope at the origin and a dying tail lives at the origin. And the loud
/// part must be compressed, or the control is a no-op that happens to pass
/// the first half.
#[test]
fn a_driven_tank_thickens_without_moving_its_decay() {
    use super::mode::{Arch, MODES};

    const FS: f32 = 48_000.0;

    // The curve alone, before any tank: unity slope at zero, and squashing
    // above it. This is the property the rest of the test depends on.
    let small = super::drive::saturate(1e-4, 1.0, 1.0) / 1e-4;
    assert!(
        (small - 1.0).abs() < 1e-3,
        "a quiet sample came back at {small:.5} of itself, so the fit is off"
    );
    let loud = super::drive::saturate(1.0, 1.0, 1.0);
    assert!(
        (0.2..0.35).contains(&loud),
        "a unit sample came back at {loud:.3}, which is not a squeeze"
    );

    // The two halves are measured with two different signals, and that is
    // the design rather than a convenience. An impulse leaves the tank at a
    // peak of about 0.03, which is squarely in the linear part of the curve
    // --- that is the small-signal case the decay fit is solved for, and the
    // decay must not move there. A sustained signal near full scale drives it
    // to about 1.0, which is where the squeeze is supposed to happen.
    // Measuring both on an impulse, which is what this test did first, checks
    // the saturator at a level where it is meant to do nothing and concludes
    // it does nothing.
    fn sustained(s: &super::engine::Settings, secs: f32) -> Vec<f32> {
        use super::engine::Reverb;
        let mut rev = Reverb::new(FS);
        rev.configure(s);
        let n = (FS * secs) as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let mut seed = 1u32;
        for i in 0..n {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let v = ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.7;
            l[i] = v;
            r[i] = v;
        }
        rev.process(s, &mut l, &mut r);
        l
    }

    for arch in [Arch::Network, Arch::Plate, Arch::Spring] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let mut s = plain_settings(i);
        s.mix = 1.0;
        s.decay = 2.0;
        s.curve.base = 2.0;

        let mut t60 = [0.0f32; 2];
        let mut peak = [0.0f32; 2];
        for (k, thickness) in [0.0f32, 1.0].into_iter().enumerate() {
            s.thickness = thickness;
            let ir = render_engine(&s, 6.0);
            t60[k] = super::measure::t60_in_band(&ir, FS, 1_000.0)
                .unwrap_or_else(|| panic!("{arch:?} at thickness {thickness} gave no tail"));
            peak[k] = sustained(&s, 2.0)
                .iter()
                .fold(0.0f32, |a, b| a.max(b.abs()));
        }

        let moved = (t60[1] / t60[0]).log2().abs();
        assert!(
            moved < 0.15,
            "{arch:?} decays in {:.2} s clean and {:.2} s driven ({moved:.2} octaves), \
             so the saturator is eating the fit",
            t60[0],
            t60[1]
        );
        assert!(
            peak[1] < peak[0] * 0.9,
            "{arch:?} peaked at {:.3} clean and {:.3} driven under a loud signal, \
             so nothing was squeezed",
            peak[0],
            peak[1]
        );
        println!(
            "  {arch:?}: {:.2} s clean, {:.2} s driven ({moved:.3} octaves); \
             loud peak {:.3} -> {:.3}",
            t60[0], t60[1], peak[0], peak[1]
        );
    }
}

/// **What the page draws has to be there, for every architecture.**
///
/// The curves and the spectrum are the third thing that can be published,
/// wired and empty --- the same shape of bug as a knob that reaches no stage,
/// and harder to see, because a curve that is flat and a curve that is
/// missing look alike on a dark panel. There are two publishers as well, the
/// plug-in and the standalone, so a stream can work in one and not the other.
///
/// The realised curve is deliberately absent on the early-reflection
/// architecture and that is the point of the `NaN`: taps have no tail, so
/// there is no decay to read back out of the filters, and a zero there would
/// draw a curve claiming the reverb dies instantly. This requires the absence
/// to be the honest kind.
#[test]
fn every_architecture_publishes_what_the_page_draws() {
    use super::engine::Reverb;
    use super::mode::{Arch, MODES};

    const FS: f32 = 48_000.0;
    // The frequencies the curve is drawn over, from the stream's own metadata
    // rather than a second list that can disagree with it.
    let lo = super::loss::FIT_BOTTOM;
    let hi = super::loss::FIT_TOP;

    for arch in [Arch::Network, Arch::Plate, Arch::Spring, Arch::Early] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let mut s = plain_settings(i);
        Reverb::apply_mode(&mut s, i);
        s.mix = 1.0;
        let mut rev = Reverb::new(FS);
        rev.configure(&s);

        // Before any audio, the spectrum must be silent rather than full of
        // whatever the buffer was built with.
        let mut spec = vec![0.0f32; 64];
        rev.fill_spectrum(&mut spec);
        assert!(
            spec.iter().all(|v| v.is_finite()),
            "{arch:?} published a spectrum that is not a number before any audio"
        );

        // Two seconds of noise, then read the curves back out of the filters
        // that are running.
        let n = (FS * 2.0) as usize;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let mut seed = 7u32;
        for k in 0..n {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let v = ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.5;
            l[k] = v;
            r[k] = v;
        }
        rev.process(&s, &mut l, &mut r);

        rev.fill_spectrum(&mut spec);
        assert!(
            spec.iter().all(|v| v.is_finite()),
            "{arch:?} published a spectrum that is not a number"
        );
        let moved = spec.iter().filter(|v| v.abs() > 1e-6).count();
        assert!(
            moved * 4 > spec.len(),
            "{arch:?} published a spectrum with only {moved} of {} points on it, \\
             so the analyser is not reading the tail",
            spec.len()
        );

        // The realised curve, over the band the stream says it covers.
        let points = 48;
        let mut real = Vec::with_capacity(points);
        for k in 0..points {
            let t = k as f32 / (points - 1) as f32;
            let f = lo * (hi / lo).powf(t);
            real.push(rev.realised_t60(f));
        }
        if arch == Arch::Early {
            assert!(
                real.iter().all(|v| v.is_nan()),
                "{arch:?} has no tail, so its realised curve must be absent \\
                 rather than a number the panel would draw"
            );
            println!("  {arch:?}: no realised curve, which is the honest answer");
            continue;
        }
        assert!(
            real.iter().all(|v| v.is_finite() && *v > 0.0),
            "{arch:?} published a realised decay that is not a positive time"
        );
        // And it has to be the decay this mode actually asks for, not a curve
        // of the right shape at the wrong scale: read back at 1 kHz it should
        // land near the mode's own entry.
        let mid = rev.realised_t60(1_000.0);
        let out = (mid / MODES[i].decay).log2().abs();
        assert!(
            out < 0.5,
            "{arch:?} draws {mid:.2} s at 1 kHz where its mode asks for {:.2} s",
            MODES[i].decay
        );
        println!(
            "  {arch:?}: realised curve {:.2}..{:.2} s, {mid:.2} s at 1 kHz against {:.2} asked",
            real.iter().cloned().fold(f32::MAX, f32::min),
            real.iter().cloned().fold(0.0f32, f32::max),
            MODES[i].decay
        );
    }
}

/// **The stream order is a contract, and two files hold it by hand.**
///
/// `src/plugin.rs` and `src/bin/standalone.rs` each publish these by index,
/// with their own `S_METER = 0` and so on, because a stream is addressed by
/// position on the wire. Nothing makes those two lists agree with this one:
/// inserting a stream rather than appending it silently points every reader
/// at the wrong data, and the page draws a spectrum on the decay axis without
/// anything failing.
///
/// So the order is written down once, here, and both publishers are checked
/// against it. A new stream goes on the end and gets a line added below.
#[test]
fn the_streams_are_in_the_order_both_publishers_expect() {
    let ids: Vec<String> = super::streams().into_iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec!["meter", "asked", "realised", "spectrum"],
        "the stream order moved --- `S_METER` and its friends in src/plugin.rs \
         and src/bin/standalone.rs are indices into this list, and a saved \
         session addresses them by position, so append rather than insert"
    );
    println!(
        "  {} streams, in the order both publishers index",
        ids.len()
    );
}

/// **A reverb's decay is in seconds, not in samples.**
///
/// Everything else here runs at 48 kHz, and almost every number in the engine
/// is derived from the sample rate: the delay lengths, the loss fit's target
/// loss per trip, the modulator rates, the pre-delay buffer, the envelope
/// coefficients. Any one of them left in samples gives a plug-in that decays
/// for half as long at 96 kHz --- and 48 kHz is the one rate at which such a
/// bug is invisible, so testing only there proves the least.
#[test]
fn the_decay_is_the_same_at_every_sample_rate() {
    use super::engine::Reverb;
    use super::mode::{Arch, MODES};

    for arch in [Arch::Network, Arch::Plate, Arch::Spring] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let mut row = Vec::new();
        for fs in [44_100.0f32, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let mut s = plain_settings(i);
            s.mix = 1.0;
            s.decay = 2.5;
            s.curve.base = 2.5;

            let mut rev = Reverb::new(fs);
            rev.configure(&s);
            let n = (fs * 6.0) as usize;
            let mut l = vec![0.0f32; n];
            let mut r = vec![0.0f32; n];
            l[0] = 1.0;
            r[0] = 1.0;
            rev.process(&s, &mut l, &mut r);
            let ir: Vec<f32> = l.iter().zip(r.iter()).map(|(a, b)| 0.5 * (a + b)).collect();

            let t60 = super::measure::t60_in_band(&ir, fs, 1_000.0)
                .unwrap_or_else(|| panic!("{arch:?} at {fs} Hz gave no measurable tail"));
            // The plate reads consistently short of what it is asked for at
            // every rate --- a known limit of its lap estimate, which
            // `docs/BENCHMARK.md` reports --- so this bound is loose enough
            // to hold it. The tight check is the spread below: whatever each
            // architecture does, it has to do the same thing at every rate.
            let out = (t60 / 2.5).log2().abs();
            assert!(
                out < 0.35,
                "{arch:?} at {fs} Hz decays in {t60:.2} s where 2.50 s was asked \
                 ({out:.2} octaves out)"
            );
            row.push((fs, t60));
        }
        // And they have to agree with *each other*, not merely each be within
        // tolerance of the target: a rate that is consistently long and one
        // consistently short can both pass the check above while the plug-in
        // plainly changes when the session's rate does.
        let lo = row.iter().map(|(_, t)| *t).fold(f32::MAX, f32::min);
        let hi = row.iter().map(|(_, t)| *t).fold(0.0f32, f32::max);
        let spread = (hi / lo).log2().abs();
        assert!(
            spread < 0.2,
            "{arch:?} decays between {lo:.2} s and {hi:.2} s depending on the sample \
             rate ({spread:.2} octaves apart)"
        );
        let list: Vec<String> = row
            .iter()
            .map(|(f, t)| format!("{:.0}k:{t:.2}", f / 1000.0))
            .collect();
        println!("  {arch:?}: {} ({spread:.3} octaves apart)", list.join(" "));
    }
}

/// **How the host chops the audio must not change the audio.**
///
/// A host hands over whatever block size it likes and changes it mid-session;
/// Ableton alone will hand out 64, 128, 256 and an odd remainder at the end of
/// a loop. Anything the engine does once per block rather than once per sample
/// --- collecting a fit, publishing a curve, updating a follower --- shows up
/// as a plug-in that sounds slightly different at a different buffer setting,
/// which is close to impossible to catch by ear and trivial to catch here.
#[test]
fn the_block_size_does_not_change_the_output() {
    use super::engine::Reverb;
    use super::mode::{Arch, MODES};

    const FS: f32 = 48_000.0;
    const N: usize = 48_000;

    for arch in [Arch::Network, Arch::Plate, Arch::Spring, Arch::Early] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let mut s = plain_settings(i);
        super::engine::Reverb::apply_mode(&mut s, i);
        s.mix = 0.5;

        // The same input every time, with a start and a stop so the envelopes
        // and followers are actually moving across block boundaries.
        let mut seed = 99u32;
        let input: Vec<f32> = (0..N)
            .map(|k| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                if k < N / 3 {
                    ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.4
                } else {
                    0.0
                }
            })
            .collect();

        let render = |block: usize| -> Vec<f32> {
            let mut rev = Reverb::new(FS);
            rev.configure(&s);
            let mut out = Vec::with_capacity(N);
            let mut at = 0;
            while at < N {
                let take = block.min(N - at);
                let mut l = input[at..at + take].to_vec();
                let mut r = l.clone();
                rev.process(&s, &mut l, &mut r);
                out.extend_from_slice(&l);
                at += take;
            }
            out
        };

        let reference = render(N);
        // 64 and 128 are ordinary; 37 is prime, so it never lines up with a
        // delay length or a modulator period and is the one most likely to
        // expose per-block work.
        for block in [37usize, 64, 128, 512] {
            let out = render(block);
            let diff = reference
                .iter()
                .zip(out.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            let peak = reference.iter().fold(0.0f32, |a, b| a.max(b.abs()));
            assert!(
                diff <= peak * 1e-5 + 1e-7,
                "{arch:?} in blocks of {block} differs from one block by {diff:.3e} \
                 against a peak of {peak:.3e}, so something is done per block that \
                 should be done per sample"
            );
        }
        println!("  {arch:?}: identical in blocks of 37, 64, 128, 512 and all at once");
    }
}

/// **One bad sample from the host must not end the plug-in.**
///
/// Three of the four architectures feed back, and a feedback loop has no way
/// to get rid of a `NaN`: every sum it takes part in is `NaN` too, so one bad
/// sample circulates for ever. Measured before the guard existed, a single
/// 64-sample block of `NaN` left the network, the plate and the spring
/// producing `NaN` two seconds later --- every sample of the last half
/// second, not some of them --- and a block of `1e30` left them at infinity.
/// In a session that is a track that has gone silent, or worse a `NaN` on the
/// mix bus, until somebody reloads the plug-in. Only the early-reflection
/// architecture came through, because it is the one with no loop.
///
/// Hosts do hand out rubbish --- an uninitialised buffer on the first block, a
/// plug-in upstream that has itself gone wrong --- so this is worth a branch
/// per sample.
///
/// Recovery is measured against a **clean run of the same audio**, not merely
/// against being finite. A tank that survives by muting itself is also finite,
/// and would pass the easy version of this test.
#[test]
fn a_bad_block_from_the_host_does_not_end_the_reverb() {
    use super::engine::Reverb;
    use super::mode::{Arch, MODES};

    const FS: f32 = 48_000.0;

    for arch in [Arch::Network, Arch::Plate, Arch::Spring, Arch::Early] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let mut s = plain_settings(i);
        Reverb::apply_mode(&mut s, i);
        s.mode = i;
        s.mix = 1.0;

        // Two seconds of ordinary music, the same every time.
        let n = (FS * 2.0) as usize;
        let mut seed = 3u32;
        let music: Vec<f32> = (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.3
            })
            .collect();

        let run = |before: Option<f32>| -> Vec<f32> {
            let mut rev = Reverb::new(FS);
            rev.configure(&s);
            if let Some(bad) = before {
                let mut l = vec![bad; 64];
                let mut r = l.clone();
                rev.process(&s, &mut l, &mut r);
            }
            let mut l = music.clone();
            let mut r = music.clone();
            rev.process(&s, &mut l, &mut r);
            // The last half second only: the tank is allowed a moment to
            // flush what it was given, it is not allowed to stay broken.
            l[l.len() - (FS * 0.5) as usize..].to_vec()
        };

        let clean = run(None);
        let want = energy(&clean);
        assert!(want > 1e-4, "{arch:?} produced nothing to compare against");

        for (name, bad) in [
            ("NaN", f32::NAN),
            ("negative infinity", f32::NEG_INFINITY),
            ("infinity", f32::INFINITY),
            ("1e30", 1e30f32),
        ] {
            let out = run(Some(bad));
            let broken = out.iter().filter(|v| !v.is_finite()).count();
            assert_eq!(
                broken,
                0,
                "{arch:?} was still producing {broken} non-finite samples of {} \
                 two seconds after a block of {name}",
                out.len()
            );
            let got = energy(&out);
            let ratio = got / want;
            assert!(
                (0.9..1.1).contains(&ratio),
                "{arch:?} came back at {ratio:.3} of its clean level after a block \
                 of {name}, so it survived without recovering"
            );
        }
        println!("  {arch:?}: recovers from NaN, both infinities and 1e30");
    }
}

/// **The dry path is not delayed, which is why no latency is reported.**
///
/// A plug-in that delays its dry signal and tells the host nothing pulls its
/// whole track out of time with every other one, and it does it silently ---
/// the reverb sounds correct in solo and the mix goes soft. This plug-in
/// reports no latency, and that is only honest while the dry signal arrives
/// when it was sent.
///
/// It is easy to lose. Pre-delay, the attack fade, the bloom generator and
/// the tape all sit in front of the tank, and any of them moved one line
/// earlier --- so that it processed the input rather than the tank's feed ---
/// would delay the dry as well without changing anything a decay measurement
/// can see.
#[test]
fn the_dry_signal_is_not_delayed() {
    use super::engine::Reverb;
    use super::mode::{Arch, MODES};

    const FS: f32 = 48_000.0;

    for arch in [Arch::Network, Arch::Plate, Arch::Spring, Arch::Early] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let mut s = plain_settings(i);
        Reverb::apply_mode(&mut s, i);
        s.mode = i;
        // Dry only. Everything in front of the tank is switched well on, so
        // that anything sitting in the wrong place would show.
        s.mix = 0.0;
        s.predelay_ms = 120.0;
        s.attack_ms = 200.0;
        s.bloom = 0.8;
        s.tape_mix = 0.7;

        let mut rev = Reverb::new(FS);
        rev.configure(&s);
        let n = 4_096;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        l[0] = 1.0;
        r[0] = 1.0;
        rev.process(&s, &mut l, &mut r);

        let at = l
            .iter()
            .position(|v| v.abs() > 0.5)
            .unwrap_or_else(|| panic!("{arch:?} lost the dry signal entirely"));
        assert_eq!(
            at, 0,
            "{arch:?} put the dry signal {at} samples late, so the plug-in owes \
             the host a latency report it does not make"
        );
        // And nothing else may arrive on the dry path either.
        let after = l[1..].iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(
            after < 1e-6,
            "{arch:?} let {after:.2e} of something through at mix 0, which is wet \
             leaking into a dry signal"
        );
    }
    println!("  the dry signal arrives when it was sent, on all four architectures");
}

/// **Dropping an instance must not hang, and must not leave its worker.**
///
/// Every engine owns a thread that does the loss fit off the audio thread.
/// The worker ends when its job channel disconnects, which happens when the
/// engine is dropped --- correct by construction, but "by construction" is
/// what people say before a deadlock. A host creates and destroys instances
/// constantly: scanning, previewing a preset, undoing an insert. A drop that
/// blocks does not fail here in a way anybody sees, it freezes the DAW.
///
/// This makes and drops fifty engines, each with a fit outstanding, which is
/// the awkward case: the worker is busy or holding a result nobody will
/// collect when the receiving end goes away.
#[test]
fn dropping_an_engine_with_a_fit_in_flight_does_not_hang() {
    use super::engine::Reverb;
    use super::mode::MODES;

    const FS: f32 = 48_000.0;
    for i in 0..50 {
        let mode = i % MODES.len();
        let mut s = plain_settings(mode);
        Reverb::apply_mode(&mut s, mode);
        s.mode = mode;
        // A curve that differs each time, so each instance really does ask
        // for a fit rather than finding one already done.
        s.decay = 1.0 + (i % 9) as f32;
        s.curve.base = s.decay;
        s.curve.bands[0].on = true;
        s.curve.bands[0].mult = 1.5 + (i % 5) as f32 * 0.4;

        let mut rev = Reverb::new(FS);
        rev.configure(&s);
        // One short block: long enough to ask, far too short to collect.
        let mut l = vec![0.0f32; 64];
        let mut r = vec![0.0f32; 64];
        l[0] = 1.0;
        rev.process(&s, &mut l, &mut r);
        drop(rev);
    }
    println!("  fifty engines made and dropped mid-fit, none of them stuck");
}
