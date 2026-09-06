//! Where each mode's character actually comes from, stage by stage.
//!
//!     cargo run --release --features plugin --example trace_modes
//!
//! `every_mode_is_a_different_sound` proves no two modes produce the same
//! audio and cannot say why, and "different" is a weak thing to know: two
//! modes that differ only because one tank is larger are two sizes of one
//! reverb, and two that differ because one runs a tape and a shifter and the
//! other does not are two instruments.
//!
//! So this reads the engine's own taps. Each stage reports what it handed on,
//! and the profile shows the journey: how much the bloom added, whether the
//! tape did anything at all, what the tank returned, what the choir took away.
//! Then every pair is compared stage by stage, and the report names the stage
//! that separates them most --- which is the thing worth knowing when you want
//! to be sure a mode is genuinely its own and not a filter over its neighbour.
use noob_reverberator::dsp::engine::{Reverb, Settings, read_settings};
use noob_reverberator::dsp::mode::MODES;
use noob_reverberator::dsp::trace::{Stage, Trace};

const FS: f32 = 48_000.0;

fn profile(i: usize) -> Trace {
    let (bridge, ix) = noob_reverberator::dsp::build_bridge("trace", FS, true);
    // Driven the way the mode strip drives it, so this is the mode as the
    // plug-in makes it.
    for (id, v) in noob_reverberator::dsp::mode::param_set(i) {
        if let Some(at) = bridge.index_of(&id) {
            bridge.set_param(at, v);
        }
    }
    let audio = bridge.take_audio().expect("the audio handle");
    let mut s: Settings = read_settings(&audio, &ix);
    s.mode = i;
    s.mix = 1.0;

    let n = (FS * 4.0) as usize;
    let mut seed = 7u32;
    // A burst with an ending: the dynamics stages have nothing to show
    // against a signal that never stops, and the saturating ones have nothing
    // to show against one that is quiet.
    let input: Vec<f32> = (0..n)
        .map(|k| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if k < (FS * 0.5) as usize {
                ((seed >> 9) as f32 / 4_194_304.0 - 1.0) * 0.5
            } else {
                0.0
            }
        })
        .collect();

    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    let mut l = input.clone();
    let mut r = input;
    let mut t = Trace::new();
    rev.process_with(&s, &mut l, &mut r, &mut t);
    t
}

/// One mode as a vector of numbers a listener would notice: what each stage
/// did to the level, and how wide and how peaky the wet came out.
fn vector(t: &Trace) -> Vec<f32> {
    let mut v: Vec<f32> = Stage::ALL.iter().skip(1).map(|s| t.gain_db(*s)).collect();
    v.push(t.level(Stage::Wet).width() * 20.0);
    v.push(t.level(Stage::Wet).crest_db());
    v.push(t.level(Stage::TankOut).crest_db());
    v
}

fn main() {
    let traces: Vec<Trace> = (0..MODES.len()).map(profile).collect();

    println!("Each stage's gain in decibels, against the stage before it.\n");
    print!("{:<24}", "mode");
    for s in Stage::ALL.iter().skip(1) {
        print!("{:>10}", s.name());
    }
    println!("{:>8}{:>8}", "width", "crest");
    for (i, m) in MODES.iter().enumerate() {
        let t = &traces[i];
        print!("{:<24}", m.name);
        for s in Stage::ALL.iter().skip(1) {
            let g = t.gain_db(*s);
            // A stage that did nothing at all is worth seeing as nothing
            // rather than as a very small number.
            if g.abs() < 0.05 {
                print!("{:>10}", "-");
            } else {
                print!("{g:>10.1}");
            }
        }
        println!(
            "{:>8.2}{:>8.1}",
            t.level(Stage::Wet).width(),
            t.level(Stage::Wet).crest_db()
        );
    }

    // Which stage separates each pair, and by how much.
    let vecs: Vec<Vec<f32>> = traces.iter().map(vector).collect();
    let labels: Vec<String> = Stage::ALL
        .iter()
        .skip(1)
        .map(|s| s.name().to_string())
        .chain(["wet width".into(), "wet crest".into(), "tank crest".into()])
        .collect();

    let mut closest: Vec<(f32, usize, usize, usize)> = Vec::new();
    for i in 0..vecs.len() {
        for j in (i + 1)..vecs.len() {
            // The stage they disagree on most, and the distance overall.
            let mut worst = (0.0f32, 0usize);
            let mut sum = 0.0f32;
            for (k, (a, b)) in vecs[i].iter().zip(vecs[j].iter()).enumerate() {
                let d = (a - b).abs();
                sum += d * d;
                if d > worst.0 {
                    worst = (d, k);
                }
            }
            closest.push((sum.sqrt(), i, j, worst.1));
        }
    }
    closest.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    println!("\nThe ten pairs whose journeys are most alike, and what still parts them:");
    for (d, i, j, k) in closest.iter().take(10) {
        println!(
            "  {d:>6.1}  {:<22} / {:<22} parts on {}",
            MODES[*i].name, MODES[*j].name, labels[*k]
        );
    }

    // A stage nothing uses is a stage that is not earning its place; a stage
    // only one mode uses is that mode's own.
    println!(
        "
Which modes put each stage to work (pre-delay cannot be seen by a level):"
    );
    for (k, s) in Stage::ALL.iter().skip(1).enumerate() {
        // **A level tap cannot see a delay.** A hundred milliseconds of
        // pre-delay hands its input on at exactly the level it arrived, so
        // this column would read nought modes for ever, however much of it
        // was in use. Left out rather than reported as a stage nobody wants.
        if *s == Stage::PreDelay {
            continue;
        }
        let users: Vec<&str> = (0..MODES.len())
            .filter(|&i| vecs[i][k].abs() > 0.5)
            .map(|i| MODES[i].name)
            .collect();
        println!(
            "  {:<10} {:>2} modes{}",
            s.name(),
            users.len(),
            if users.len() <= 4 && !users.is_empty() {
                format!("  ({})", users.join(", "))
            } else {
                String::new()
            }
        );
    }
}
