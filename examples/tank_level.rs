//! Where to put the saturator's knee, measured rather than guessed.
//!
//! `cargo run --release --features plugin --example tank_level`
//!
//! The three architectures run at very different internal levels, so one
//! absolute knee cannot fit them: placed for the network it is a knee the
//! plate never reaches. This prints, for each of them, the level of the
//! signal actually handed to `dsp::drive::saturate` and what Thickness does
//! to the output, which is where the constants in that module come from.
//!
//! It also shows why the plate is driven at its transducer as well as in its
//! loop. Read off the loop alone, the plate moved by three parts in a
//! hundred: its recirculation is a small part of what comes out under a
//! sustained signal, and a saturator there is a control nobody can hear.
use noob_reverberator::dsp::engine::{Reverb, read_settings};
use noob_reverberator::dsp::mode::{Arch, MODES};

fn main() {
    const FS: f32 = 48_000.0;
    println!(
        "{:<10} {:>9} {:>9} {:>9}",
        "tank", "thickness", "peak", "rms"
    );
    for arch in [Arch::Network, Arch::Plate, Arch::Spring] {
        let i = MODES.iter().position(|m| m.arch == arch).expect("an arch");
        let (bridge, ix) = noob_reverberator::dsp::build_bridge("probe", FS, true);
        let audio = bridge.take_audio().expect("the audio handle");
        let mut s = read_settings(&audio, &ix);
        Reverb::apply_mode(&mut s, i);
        s.mode = i;
        s.mix = 1.0;
        for thickness in [0.0f32, 1.0] {
            s.thickness = thickness;
            let mut rev = Reverb::new(FS);
            rev.configure(&s);
            let n = (FS * 2.0) as usize;
            let mut l = vec![0.0f32; n];
            let mut r = vec![0.0f32; n];
            // Two seconds of noise near full scale: a busy mix, which is the
            // signal a driven tank is meant to be driven by.
            let mut seed = 1u32;
            for k in 0..n {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let v = ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.7;
                l[k] = v;
                r[k] = v;
            }
            rev.process(&s, &mut l, &mut r);
            let peak = l.iter().fold(0.0f32, |a, b| a.max(b.abs()));
            let rms = (l.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
            println!(
                "{:<10} {thickness:>9} {peak:>9.4} {rms:>9.4}",
                format!("{arch:?}")
            );
        }
    }
}
