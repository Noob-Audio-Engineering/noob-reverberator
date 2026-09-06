//! Does selecting a mode change anything the plug-in actually does?
//!
//! `cargo run --release --features plugin --example mode_identity`
//!
//! This drives the engine exactly the way the plug-in drives it: write the
//! mode's published parameter set through the bridge, which is what the mode
//! strip on the page does.
//!
//! It used to set the `mode` parameter and nothing else, because that is all
//! the plug-in did --- and it reported four distinct sounds from thirty-two
//! names, every network mode bit-for-bit identical to every other. The engine
//! read the mode only to choose an architecture and nothing applied the rest
//! of the table.
use noob_reverberator::dsp::engine::{Reverb, read_settings};
use noob_reverberator::dsp::mode::MODES;

const FS: f32 = 48_000.0;

fn render_as_the_plugin_does(mode: usize) -> Vec<f32> {
    let (bridge, ix) = noob_reverberator::dsp::build_bridge("probe", FS, true);
    // Exactly what the mode strip writes when it is clicked.
    for (id, v) in noob_reverberator::dsp::mode::param_set(mode) {
        if let Some(at) = bridge.index_of(&id) {
            bridge.set_param(at, v);
        }
    }
    let audio = bridge.take_audio().expect("the audio handle");
    let mut s = read_settings(&audio, &ix);
    s.mode = mode;
    s.mix = 1.0;
    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    let n = (FS * 2.0) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    l[0] = 1.0;
    r[0] = 1.0;
    rev.process(&s, &mut l, &mut r);
    l
}

fn main() {
    let mut groups: std::collections::BTreeMap<String, Vec<&str>> = Default::default();
    let refs: Vec<(usize, Vec<f32>)> = (0..MODES.len())
        .map(|i| (i, render_as_the_plugin_does(i)))
        .collect();

    // Which modes are bit-for-bit the same sound?
    let mut identical = 0;
    for (i, a) in refs.iter() {
        let mut same_as = None;
        for (j, b) in refs.iter() {
            if j >= i {
                break;
            }
            if a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x == y) {
                same_as = Some(*j);
                break;
            }
        }
        if let Some(j) = same_as {
            identical += 1;
            groups
                .entry(MODES[j].name.to_string())
                .or_default()
                .push(MODES[*i].name);
        } else {
            groups.entry(MODES[*i].name.to_string()).or_default();
        }
    }

    println!("Driving the engine the way the plug-in drives it (set `mode`, nothing else):\n");
    for (head, rest) in groups.iter() {
        if rest.is_empty() {
            println!("  {head}  --- on its own");
        } else {
            println!("  {head}  is bit-for-bit identical to:");
            for r in rest {
                println!("      {r}");
            }
        }
    }
    println!(
        "\n{identical} of {} modes produce a sound already produced by an earlier mode.",
        MODES.len()
    );
    println!(
        "{} distinct sounds from {} names.",
        MODES.len() - identical,
        MODES.len()
    );
}
