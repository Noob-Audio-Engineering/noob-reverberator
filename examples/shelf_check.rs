//! Can the fit actually realise the steep high shelves the modes now ask for?
use noob_reverberator::dsp::engine::{Reverb, read_settings};
use noob_reverberator::dsp::measure::t60_in_band;
use noob_reverberator::dsp::mode::MODES;

const FS: f32 = 48_000.0;

fn run(i: usize, strip: bool) -> Vec<(f32, f32, f32)> {
    let (bridge, ix) = noob_reverberator::dsp::build_bridge("probe", FS, true);
    let audio = bridge.take_audio().expect("handle");
    let mut s = read_settings(&audio, &ix);
    Reverb::apply_mode(&mut s, i);
    s.mode = i;
    s.mix = 1.0;
    s.mod_depth = 0.0;
    s.early_level = 0.0;
    if strip {
        // Everything that colours the measurement rather than the tank.
        s.era_amount = 0.0;
        s.tape_mix = 0.0;
        s.choir_amount = 0.0;
        s.shift_mix = 0.0;
        s.thickness = 0.0;
    }
    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    let n = (FS * 12.0) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    l[0] = 1.0;
    r[0] = 1.0;
    rev.process(&s, &mut l, &mut r);
    let mono: Vec<f32> = l.iter().zip(r.iter()).map(|(a, b)| 0.5 * (a + b)).collect();
    [125.0f32, 1000.0, 4000.0]
        .iter()
        .map(|&f| {
            let want = s.curve.t60_over_octave(f);
            let got = t60_in_band(&mono, FS, f).unwrap_or(f32::NAN);
            (f, want, got)
        })
        .collect()
}

fn main() {
    for name in [
        "Tape Chamber",
        "Nineteen Seventy Nine",
        "Magneto",
        "Far Hall",
    ] {
        let i = MODES.iter().position(|m| m.name == name).unwrap();
        println!("{name}:");
        for (label, strip) in [("as the mode ships", false), ("colouring stripped", true)] {
            let rows = run(i, strip);
            let worst = rows
                .iter()
                .map(|(_, w, g)| (g / w).log2().abs())
                .fold(0.0f32, f32::max);
            let cells: Vec<String> = rows
                .iter()
                .map(|(f, w, g)| format!("{f:.0}Hz want {w:.2} got {g:.2}"))
                .collect();
            println!("   {label:<20} worst {worst:.2} oct   {}", cells.join("  "));
        }
    }
}
