//! What a control change costs the audio thread, against how many bands are
//! in use.
//!
//! Two numbers per row: what `configure` costs on the audio thread, which is
//! what a dropout would come from, and what the fit costs on its own thread,
//! which is only how long the new curve takes to arrive.
use noob_reverberator::dsp::colour::Era;
use noob_reverberator::dsp::decay::{Band, Curve, Shape};
use noob_reverberator::dsp::engine::{Reverb, Settings};
use noob_reverberator::dsp::fdn::{Fdn, prime_lengths};
use noob_reverberator::dsp::shape::Kind as ShapeKind;
use std::time::Instant;

const FS: f32 = 48_000.0;

fn settings(curve: Curve) -> Settings {
    Settings {
        bypass: false,
        mix: 1.0,
        output: 1.0,
        freeze: false,
        mode: 0,
        decay: curve.base,
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
        lines: 16,
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
        curve,
    }
}

fn curve_with(active: usize) -> Curve {
    let mut c = Curve {
        base: 2.4,
        ..Default::default()
    };
    for i in 0..active {
        c.bands[i] = Band {
            on: true,
            shape: match i % 3 {
                0 => Shape::Bell,
                1 => Shape::LowShelf,
                _ => Shape::HighShelf,
            },
            freq: 40.0 * 1.25f32.powi(i as i32),
            mult: if i % 2 == 0 { 1.8 } else { 0.6 },
            width: 1.0,
        };
    }
    c
}

fn main() {
    println!("A block at {FS} Hz and 256 samples is 5.33 ms.\n");
    println!("| bands in use | `configure` on the audio thread | the fit, on its own thread |");
    println!("|---|---|---|");
    for active in [0usize, 1, 2, 4, 8, 16, 32] {
        let curve = curve_with(active);
        let s = settings(curve);

        // What the audio thread pays: a changed curve, then configure.
        let mut rev = Reverb::new(FS);
        rev.configure(&s);
        let runs = 200;
        let t0 = Instant::now();
        for k in 0..runs {
            // Move something every time, so `configure` never takes its
            // "nothing changed" exit.
            let mut s2 = s;
            s2.width = 1.0 + k as f32 * 1e-6;
            rev.configure(&s2);
        }
        let cfg = t0.elapsed().as_secs_f64() / runs as f64;

        // What the fit itself costs, measured the old way for comparison.
        let mut fdn = Fdn::new(16, (FS * 0.6) as usize, FS);
        let mut lens = Vec::new();
        prime_lengths(16, FS * 0.013, FS * 0.075, &mut lens);
        fdn.set_lengths(&lens, &curve);
        let t1 = Instant::now();
        for _ in 0..5 {
            fdn.refit(&curve);
        }
        let fit = t1.elapsed().as_secs_f64() / 5.0;

        println!(
            "| {active} | {:.3} ms | {:.1} ms |",
            cfg * 1000.0,
            fit * 1000.0
        );
    }
}
