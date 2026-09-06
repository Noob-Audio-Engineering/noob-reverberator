//! Every control the plug-in has, in the order the host will always see them.
//!
//! # This list is append only
//!
//! A host stores automation against the **index**, and a saved project asks
//! for parameter seven rather than for "Decay". Inserting one in the middle
//! moves every later parameter's automation onto the wrong control. New ones
//! go on the end.

use noob_vst_webgui_framework::ParamSpec;

use super::colour::ERA_NAMES;
use super::decay::MAX_BANDS;
use super::formant::VOWEL_NAMES;
use super::mode;
use super::shape::SHAPE_NAMES;

/// The shapes a decay band can have, in index order --- from the shared
/// crate, so the panel, the host and Noob-Q all name them the same way.
pub use super::decay::SHAPE_NAMES as BAND_SHAPE_NAMES;

/// Every parameter, in order.
pub fn param_specs() -> Vec<ParamSpec> {
    let mut v = vec![
        ParamSpec::new("bypass", "Bypass").toggle().group("global"),
        ParamSpec::new("mix", "Mix")
            .range(0.0, 100.0)
            .default(35.0)
            .unit("%")
            .group("global"),
        ParamSpec::new("output", "Output")
            .range(-24.0, 24.0)
            .default(0.0)
            .unit("dB")
            .group("global"),
        ParamSpec::new("freeze", "Freeze").toggle().group("global"),
        ParamSpec::new("mode", "Mode")
            .labels(mode::names())
            .group("space"),
        ParamSpec::new("decay", "Decay")
            .range(0.05, 60.0)
            .default(2.4)
            .unit("s")
            .log()
            .decimals(2)
            .group("space"),
        ParamSpec::new("size", "Size")
            .range(5.0, 400.0)
            .default(100.0)
            .unit("%")
            .group("space"),
        ParamSpec::new("density", "Density")
            .range(0.0, 100.0)
            .default(80.0)
            .unit("%")
            .group("space"),
        ParamSpec::new("attack", "Attack")
            .range(0.0, 500.0)
            .default(10.0)
            .unit("ms")
            .skew(0.5)
            .group("space"),
        ParamSpec::new("predelay", "Pre-delay")
            .range(0.0, 500.0)
            .default(0.0)
            .unit("ms")
            .skew(0.5)
            .group("space"),
        ParamSpec::new("width", "Width")
            .range(0.0, 200.0)
            .default(100.0)
            .unit("%")
            .group("space"),
        ParamSpec::new("mod_rate", "Mod Rate")
            .range(0.01, 10.0)
            .default(0.7)
            .unit("Hz")
            .log()
            .decimals(2)
            .group("modulation"),
        ParamSpec::new("mod_depth", "Mod Depth")
            .range(0.0, 100.0)
            .default(30.0)
            .unit("%")
            .group("modulation"),
        ParamSpec::new("mod_random", "Mod Character")
            .range(0.0, 100.0)
            .default(40.0)
            .unit("%")
            .group("modulation"),
        ParamSpec::new("early_level", "Early Level")
            .range(0.0, 100.0)
            .default(25.0)
            .unit("%")
            .group("early"),
        ParamSpec::new("early_size", "Early Size")
            .range(1.0, 40.0)
            .default(8.0)
            .unit("m")
            .decimals(1)
            .group("early"),
        ParamSpec::new("early_absorb", "Early Absorption")
            .range(0.0, 95.0)
            .default(35.0)
            .unit("%")
            .group("early"),
        ParamSpec::new("early_taps", "Early Taps")
            .range(1.0, 64.0)
            .default(24.0)
            .integer()
            .group("early"),
        ParamSpec::new("shift", "Shift")
            .range(-24.0, 24.0)
            .default(12.0)
            .unit("st")
            .integer()
            .group("shift"),
        ParamSpec::new("shift_mix", "Shift Amount")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("shift"),
        ParamSpec::new("shift_window", "Shift Window")
            .range(1024.0, 32768.0)
            .default(16384.0)
            .unit("sa")
            .log()
            .integer()
            .group("shift"),
        ParamSpec::new("shape", "Shape")
            .labels(SHAPE_NAMES.to_vec())
            .group("shape"),
        ParamSpec::new("shape_hold", "Shape Time")
            .range(10.0, 4000.0)
            .default(400.0)
            .unit("ms")
            .log()
            .group("shape"),
        ParamSpec::new("era", "Era")
            .labels(ERA_NAMES.to_vec())
            .group("era"),
        ParamSpec::new("era_amount", "Era Amount")
            .range(0.0, 100.0)
            .default(100.0)
            .unit("%")
            .group("era"),
        ParamSpec::new("tension", "Spring Tension")
            .range(5.0, 95.0)
            .default(60.0)
            .unit("%")
            .group("spring"),
        ParamSpec::new("sections", "Spring Sections")
            .range(1.0, 160.0)
            .default(100.0)
            .integer()
            .group("spring"),
        ParamSpec::new("tape_mix", "Tape Amount")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("tape"),
        ParamSpec::new("tape_time", "Tape Time")
            .range(20.0, 2000.0)
            .default(320.0)
            .unit("ms")
            .log()
            .group("tape"),
        ParamSpec::new("tape_heads", "Tape Heads")
            .range(1.0, 4.0)
            .default(3.0)
            .integer()
            .group("tape"),
        ParamSpec::new("tape_feedback", "Tape Feedback")
            .range(0.0, 98.0)
            .default(35.0)
            .unit("%")
            .group("tape"),
        ParamSpec::new("tape_wobble", "Tape Wobble")
            .range(0.0, 100.0)
            .default(30.0)
            .unit("%")
            .group("tape"),
        ParamSpec::new("tape_drive", "Tape Drive")
            .range(0.0, 100.0)
            .default(30.0)
            .unit("%")
            .group("tape"),
        ParamSpec::new("choir_amount", "Choir Amount")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("choir"),
        ParamSpec::new("choir_vowel", "Vowel")
            .range(0.0, (VOWEL_NAMES.len() - 1) as f32)
            .default(0.0)
            .decimals(2)
            .group("choir"),
        ParamSpec::new("choir_spread", "Choir Size")
            .range(-100.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("choir"),
        ParamSpec::new("choir_resonance", "Choir Resonance")
            .range(0.0, 100.0)
            .default(50.0)
            .unit("%")
            .group("choir"),
        ParamSpec::new("distance", "Distance")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("space"),
        ParamSpec::new("thickness", "Thickness")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("space"),
        ParamSpec::new("duck", "Ducking")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("dynamics"),
        ParamSpec::new("duck_release", "Duck Release")
            .range(20.0, 2000.0)
            .default(250.0)
            .unit("ms")
            .log()
            .group("dynamics"),
        ParamSpec::new("gate", "Auto Gate")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("dynamics"),
        ParamSpec::new("gate_hold", "Gate Hold")
            .range(20.0, 3000.0)
            .default(300.0)
            .unit("ms")
            .log()
            .group("dynamics"),
        ParamSpec::new("bloom", "Bloom")
            .range(0.0, 100.0)
            .default(0.0)
            .unit("%")
            .group("bloom"),
        ParamSpec::new("bloom_time", "Bloom Time")
            .range(10.0, 600.0)
            .default(120.0)
            .unit("ms")
            .log()
            .group("bloom"),
        ParamSpec::new("bloom_swell", "Bloom Swell")
            .range(50.0, 4000.0)
            .default(600.0)
            .unit("ms")
            .log()
            .group("bloom"),
        ParamSpec::new("lines", "Lines")
            .range(4.0, 16.0)
            .default(12.0)
            .integer()
            .group("space"),
    ];

    // The decay curve's bands. Five parameters each, and they are generated
    // rather than written out so a band cannot be given the wrong group or a
    // range that differs from its neighbours by a typo.
    for i in 0..MAX_BANDS {
        let n = i + 1;
        let g = "decay curve";
        v.push(
            ParamSpec::new(format!("band{n}_on"), format!("Band {n} On"))
                .toggle()
                .group(g),
        );
        v.push(
            ParamSpec::new(format!("band{n}_shape"), format!("Band {n} Shape"))
                .labels(BAND_SHAPE_NAMES.to_vec())
                .group(g),
        );
        v.push(
            ParamSpec::new(format!("band{n}_freq"), format!("Band {n} Frequency"))
                .range(20.0, 20_000.0)
                .default(default_band_freq(i))
                .unit("Hz")
                .log()
                .group(g),
        );
        v.push(
            ParamSpec::new(format!("band{n}_mult"), format!("Band {n} Decay"))
                .range(0.1, 10.0)
                .default(1.0)
                .unit("x")
                .log()
                .decimals(2)
                .group(g),
        );
        v.push(
            ParamSpec::new(format!("band{n}_width"), format!("Band {n} Width"))
                .range(0.1, 4.0)
                .default(1.0)
                .unit("oct")
                .decimals(2)
                .group(g),
        );
    }
    v
}

/// Where each band sits before anybody moves it.
///
/// Spread geometrically over the audible band, so bands opened one after
/// another do not land on top of each other --- and written as a formula
/// rather than a table, because a table of thirty-two frequencies is
/// thirty-two chances to fat-finger one and no way to notice.
fn default_band_freq(i: usize) -> f32 {
    const LO: f32 = 60.0;
    const HI: f32 = 12_000.0;
    let t = i as f32 / (MAX_BANDS - 1).max(1) as f32;
    LO * (HI / LO).powf(t)
}
