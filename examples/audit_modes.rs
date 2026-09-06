//! Is each mode true to the thing it is named after?
//!
//!     cargo run --release --features plugin --example audit_modes
//!
//! Every other check here asks whether a mode does what its *table entry*
//! says. This asks the harder question: whether it does what its **name and
//! blurb** say. A mode called Shimmer that adds no octave is internally
//! consistent and still a lie; so is a Spring that does not chirp, an Echoverb
//! whose repeats cannot be counted, or a Chorale that does not sing.
//!
//! Each measurement is chosen to be falsifiable rather than flattering: a
//! number that would come out wrong if the stage behind it were unwired, which
//! is exactly how five controls in this plug-in turned out to be doing nothing.
//!
//! # What is not measured here, and why
//!
//! A spring's chirp. Three attempts at it from a whole mode's impulse response
//! all measured something else: envelope peaks read the tank's build-up, and
//! band-passed peaks read the decay curve, so Concert Hall came out chirping
//! at 79 ms and Chamber at 84 while the two springs sat at 5. Dispersion is a
//! first-arrival property of the coil, and it is measured on a bare coil in
//! `a_spring_returns_the_top_of_the_band_first`, where nothing else is in the
//! way.
//!
//! Tape repeats, likewise. Autocorrelation of the envelope found a strong
//! repeat in Magneto at 80 ms --- and an equally strong one in Concert Hall,
//! which has no tape in it at all, because a smooth decaying envelope
//! correlates with itself at any lag.
use noob_reverberator::dsp::engine::{Reverb, Settings, read_settings};
use noob_reverberator::dsp::mode::MODES;

const FS: f32 = 48_000.0;

fn settings(i: usize) -> Settings {
    let (bridge, ix) = noob_reverberator::dsp::build_bridge("audit", FS, true);
    for (id, v) in noob_reverberator::dsp::mode::param_set(i) {
        if let Some(at) = bridge.index_of(&id) {
            bridge.set_param(at, v);
        }
    }
    let audio = bridge.take_audio().expect("the audio handle");
    let mut s = read_settings(&audio, &ix);
    s.mode = i;
    s.mix = 1.0;
    s
}

fn render(s: &Settings, input: &[f32]) -> Vec<f32> {
    let mut rev = Reverb::new(FS);
    rev.configure(s);
    let mut l = input.to_vec();
    let mut r = input.to_vec();
    rev.process(s, &mut l, &mut r);
    l.iter().zip(r.iter()).map(|(a, b)| 0.5 * (a + b)).collect()
}

fn impulse(n: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; n];
    v[0] = 1.0;
    v
}

fn sine(n: usize, f: f32, secs: f32) -> Vec<f32> {
    let mut v = vec![0.0f32; n];
    let len = ((FS * secs) as usize).min(n);
    for (i, x) in v.iter_mut().enumerate().take(len) {
        let t = i as f32 / FS;
        // Faded in and out, so the burst's own edges do not spray energy
        // across the spectrum and look like a pitch shift.
        let e = (std::f32::consts::PI * i as f32 / len as f32).sin();
        *x = e * e * (std::f32::consts::TAU * f * t).sin();
    }
    v
}

fn noise(n: usize, secs: f32) -> Vec<f32> {
    let mut v = vec![0.0f32; n];
    let mut seed = 12345u32;
    for x in v.iter_mut().take(((FS * secs) as usize).min(n)) {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *x = ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.5;
    }
    v
}

/// Energy in a narrow band, by one bin of a Goertzel.
fn tone_energy(x: &[f32], f: f32) -> f32 {
    let w = std::f32::consts::TAU * f / FS;
    let (c, s) = (w.cos(), w.sin());
    let (mut q1, mut q2) = (0.0f32, 0.0f32);
    for &v in x {
        let q0 = 2.0 * c * q1 - q2 + v;
        q2 = q1;
        q1 = q0;
    }
    let (re, im) = (q1 - c * q2, s * q2);
    (re * re + im * im).sqrt() / x.len() as f32
}

/// A one-pole band-pass envelope, for arrival times and decay shape.
fn band_env(x: &[f32], f: f32, win: usize) -> Vec<f32> {
    let w = (std::f32::consts::TAU * f / FS).tan();
    let g = w / (1.0 + w);
    let (mut lp, mut bp) = (0.0f32, 0.0f32);
    let mut out = Vec::with_capacity(x.len() / win + 1);
    let mut acc = 0.0f32;
    let mut k = 0;
    for &v in x {
        let hp = v - lp - bp;
        bp += g * hp;
        lp += g * bp;
        acc += bp * bp;
        k += 1;
        if k == win {
            out.push((acc / win as f32).sqrt());
            acc = 0.0;
            k = 0;
        }
    }
    out
}

fn peak_at(env: &[f32]) -> usize {
    env.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn main() {
    let n = (FS * 6.0) as usize;
    let win = (FS * 0.005) as usize;

    println!(
        "{:<24} {:>7} {:>7} {:>7} {:>8} {:>8} {:>7} {:>7}",
        "mode", "build", "t60", "peaks", "oct+", "oct-", "early", "sat"
    );

    for (i, m) in MODES.iter().enumerate() {
        let s = settings(i);
        // **Long enough for this mode.** A fixed six-second render fitted
        // Supermassive's thirty-second decay to twelve, and I nearly softened
        // a shelf to "fix" a mode that was right --- `docs/BENCHMARK.md`, which
        // scales its render to the decay, has always had it at 29.99 s.
        let n = ((FS * (s.curve.t60(1_000.0) * 2.5).clamp(6.0, 80.0)) as usize).max(n);
        let ir = render(&s, &impulse(n));

        // Build-up: where the whole-band envelope peaks, in milliseconds.
        let env = band_env(&ir, 1_000.0, win);
        let build = peak_at(&env) as f32 * 5.0;

        let t60 = noob_reverberator::dsp::measure::t60_in_band(&ir, FS, 1_000.0).unwrap_or(0.0);

        // Countable repeats: how many clear peaks stand above the local floor
        // in the first second. A diffuse tank smears these into nothing.
        let e = band_env(&ir, 1_000.0, win);
        let take = (200).min(e.len());
        let floor = rms(&e[..take]) * 2.5;
        let mut peaks = 0;
        for k in 1..take.saturating_sub(1) {
            if e[k] > floor && e[k] > e[k - 1] && e[k] >= e[k + 1] {
                peaks += 1;
            }
        }

        // A pitch shifter is the one claim a spectrum answers outright: feed
        // one tone and look an octave either side of it.
        let f0 = 440.0f32;
        let tone = render(&s, &sine(n, f0, 0.5));
        let tail = &tone[(FS * 1.0) as usize..];
        let at = tone_energy(tail, f0).max(1e-12);
        let up = tone_energy(tail, f0 * 2.0) / at;
        let down = tone_energy(tail, f0 * 0.5) / at;

        // Chirp: how much later the bottom of the band *arrives* than the top.
        //
        // Onset, not peak. A band's envelope peak sits wherever the tank's
        // build-up puts it, so peak-to-peak measured Magneto and Far Hall as
        // chirping harder than the spring --- it was reading the build, not
        // the dispersion. First crossing of a tenth of that band's own peak is
        // when the energy actually got there.
        // At sample resolution and over the first quarter second, which is
        // where a coil's sweep lives. Five-millisecond windows put every mode
        // at zero, including the springs: the whole dispersion fits inside one
        // window. This is the method `a_spring_returns_the_top_of_the_band_first`
        // already uses on a bare coil.
        // How much of the answer is over in the first eighty milliseconds:
        // an ambience is nearly all early and a cloud nearly none of it.
        //
        // The chirp column that used to be here was dropped. A band-passed
        // peak in a dense tank sits where the modal build-up puts it, not
        // where the energy first arrives, so it read Chamber at 84 ms and the
        // springs at 5 --- a chamber chirping harder than a coil is the
        // measurement being wrong, not the chamber. Dispersion is a
        // first-arrival property and is measured on a bare coil, in
        // `a_spring_returns_the_top_of_the_band_first`.
        let head = rms(&ir[..(FS * 0.08) as usize]);
        let early = head / rms(&ir).max(1e-9);

        // Saturation: how much the wet level falls when the input is driven,
        // beyond the level change itself.
        let quiet = render(&s, &noise(n, 0.5));
        let loud: Vec<f32> = noise(n, 0.5).iter().map(|v| v * 6.0).collect();
        let driven = render(&s, &loud);
        let sat = 20.0
            * ((rms(&driven) / 6.0) / rms(&quiet).max(1e-9))
                .max(1e-9)
                .log10();

        println!(
            "{:<24} {:>6.0}m {:>6.2}s {:>7} {:>8.3} {:>8.3} {:>6.2} {:>6.1}dB",
            m.name, build, t60, peaks, up, down, early, sat
        );
    }

    // ---- the claims a single column cannot answer ----
    println!("\n-- named claims, each measured on its own terms --");

    let idx = |name: &str| MODES.iter().position(|m| m.name == name).expect(name);

    // Magneto and Tape Chamber say "multi-head tape echo". A tape echo puts
    // energy back at its head spacing, which autocorrelation finds and a
    // diffuse tank does not fake.
    for name in ["Magneto", "Tape Chamber", "Concert Hall"] {
        let s = settings(idx(name));
        let ir = render(&s, &impulse(n));
        let env = band_env(&ir, 1_000.0, win);
        // Look for a repeat between 60 and 600 ms.
        let (lo, hi) = (12usize, 120usize);
        let mean = env.iter().take(400).sum::<f32>() / 400.0;
        let dev: Vec<f32> = env.iter().take(400).map(|v| v - mean).collect();
        let norm: f32 = dev.iter().map(|v| v * v).sum::<f32>().max(1e-12);
        let mut best = (0.0f32, 0usize);
        for lag in lo..hi.min(dev.len() / 2) {
            let c: f32 = dev[..dev.len() - lag]
                .iter()
                .zip(&dev[lag..])
                .map(|(a, b)| a * b)
                .sum::<f32>()
                / norm;
            if c > best.0 {
                best = (c, lag);
            }
        }
        println!(
            "  {name:<16} strongest repeat {:.2} at {} ms",
            best.0,
            best.1 * 5
        );
    }

    // Chorale says "vowels on the tail, so it sings". A vowel is two
    // resonances; against the same tank with the choir off, the tail should
    // gain energy at the formants and lose it between them.
    for name in ["Chorale", "Choir Loft"] {
        let i = idx(name);
        let s = settings(i);
        let mut off = s;
        off.choir_amount = 0.0;
        let (a, b) = (render(&s, &noise(n, 0.5)), render(&off, &noise(n, 0.5)));
        let tail = (FS * 1.0) as usize;
        // **At this mode's own vowel.** Chorale sings "Ah" (730, 1090) and
        // Choir Loft sings "Ee" (270, 2290); measuring both at Ah's formants
        // said Choir Loft had none, which was the measurement looking in the
        // wrong place rather than a choir that does not sing.
        let (f1, f2, between) = if MODES[i].vowel < 1.0 {
            (730.0f32, 1090.0f32, 1900.0f32)
        } else {
            (270.0f32, 2290.0f32, 900.0f32)
        };
        let at = |x: &[f32], f: f32| tone_energy(&x[tail..], f).max(1e-12);
        let g = |f: f32| 20.0 * (at(&a, f) / at(&b, f)).log10();
        println!(
            "  {name:<16} vowel {:<3} formants {:+.1}/{:+.1} dB, between {:+.1} dB",
            noob_reverberator::dsp::formant::VOWEL_NAMES[MODES[i].vowel as usize],
            g(f1),
            g(f2),
            g(between)
        );
    }

    // Nineteen Seventy Nine says "narrow and grainy": the era colouring should
    // cost it the top of the band against the same mode with it turned down.
    for name in ["Nineteen Seventy Nine", "Nineteen Eighty Four"] {
        let i = idx(name);
        let s = settings(i);
        let mut off = s;
        off.era_amount = 0.0;
        let (a, b) = (render(&s, &noise(n, 0.5)), render(&off, &noise(n, 0.5)));
        let tail = (FS * 0.8) as usize;
        let hi = |x: &[f32]| tone_energy(&x[tail..], 12_000.0).max(1e-12);
        println!(
            "  {name:<24} 12 kHz with the era on: {:+.1} dB",
            20.0 * (hi(&a) / hi(&b)).log10()
        );
    }

    // The three chaotic modes say they "will not hold still". Two renders of
    // the same input should differ, and a steady mode's should not.
    for name in ["Chaotic Neutral", "Chaotic Hall", "Concert Hall"] {
        let s = settings(idx(name));
        let a = render(&s, &impulse(n));
        let b = render(&s, &impulse(n));
        let d: Vec<f32> = a.iter().zip(b.iter()).map(|(x, y)| x - y).collect();
        println!(
            "  {name:<16} two renders differ by {:.1e} of level",
            rms(&d) / rms(&a).max(1e-9)
        );
    }
}
