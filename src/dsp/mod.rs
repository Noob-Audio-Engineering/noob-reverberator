//! Noob Reverberator's engine.
//!
//! The plug-in is about one thing: **the decay time is drawn against
//! frequency, and the network is fitted to it.** [`decay`] holds the curve a
//! person draws, [`loss`] turns it into the filter each delay line needs, and
//! everything else is the reverberator those lines are wired into.

pub mod bloom;
pub mod colour;
pub mod decay;
pub mod delay;
pub mod diffuse;
pub mod drive;
pub mod duck;
pub mod early;
pub mod engine;
pub mod fdn;
pub mod filters;
pub mod fitter;
pub mod formant;
pub mod loss;
pub mod measure;
pub mod mode;
pub mod modulate;
pub mod params;
pub mod pitch;
pub mod plate;
pub mod preset;
pub mod shape;
pub mod spring;
pub mod svf;
pub mod sync;
pub mod tape;
pub mod trace;

#[cfg(test)]
mod tests;

use noob_vst_webgui_framework::{NoobVstWebguiFramework, StreamKind, StreamSpec};
use serde_json::{Value, json};

/// How many points the panel's decay curve is drawn from.
pub const CURVE_POINTS: usize = 160;

/// Build the bridge the page talks to, and resolve the parameter indices.
pub fn build_bridge(
    name: &str,
    sr: f32,
    standalone: bool,
) -> (NoobVstWebguiFramework, engine::ParamIx) {
    let mut b = NoobVstWebguiFramework::builder(name)
        .meta(bridge_meta(sr, standalone))
        .params(params::param_specs());
    for s in streams() {
        b = b.stream(s);
    }
    let bridge = b.build();
    let ix = engine::ParamIx::new(&bridge);
    (bridge, ix)
}

/// What the page needs to know that is not a parameter.
pub fn bridge_meta(sr: f32, standalone: bool) -> Value {
    json!({
        "vendor": "Noob Audio Engineering",
        "version": env!("CARGO_PKG_VERSION"),
        "sample_rate": sr,
        "standalone": standalone,
        "max_bands": decay::MAX_BANDS,
        "curve_points": CURVE_POINTS,
        "fit_bottom": loss::FIT_BOTTOM,
        "fit_top": loss::FIT_TOP,
        "min_t60": decay::MIN_T60,
        "max_t60": decay::MAX_T60,
        "band_shapes": params::BAND_SHAPE_NAMES,
        "shapes": shape::SHAPE_NAMES,
        "sync": sync::SYNC_NAMES,
        // The vowels the choir sings between. The control is continuous on
        // purpose --- Chorale's whole idea is sweeping from one to the next
        // --- so these are names for the page to show and to jump to, not a
        // step count that would take the sweep away.
        "vowels": formant::VOWEL_NAMES,
        "eras": colour::ERA_NAMES,
        "presets": preset::factory_json(),
        // `set` is the mode. Without it the page could only tell the engine
        // *which* mode, and the engine only ever read that to pick an
        // architecture --- so every network mode was the same reverb. The
        // page applies these through the ordinary parameter path, exactly as
        // it applies a preset, so the host sees the knobs move.
        "modes": mode::MODES.iter().enumerate().map(|(i, m)| json!({
            "name": m.name,
            "blurb": m.blurb,
            "arch": match m.arch {
                mode::Arch::Network => "network",
                mode::Arch::Plate => "plate",
                mode::Arch::Spring => "spring",
                mode::Arch::Early => "early",
            },
            "set": mode::param_set(i),
        })).collect::<Vec<_>>(),
    })
}

/// The streams the engine publishes for the page to draw.
///
/// **The page computes none of this.** The decay curve it draws is the one the
/// engine is fitted to, and the realised curve beside it is read back out of
/// the filters that are running --- so what is on the panel is what the audio
/// is doing, not a second implementation of it that can drift.
pub fn streams() -> Vec<StreamSpec> {
    vec![
        StreamSpec::new("meter", 4)
            .name("Meter")
            .kind(StreamKind::Meter)
            .channels(2)
            .meta(json!({ "layout": "in_l,in_r,out_l,out_r" })),
        StreamSpec::new("asked", CURVE_POINTS)
            .name("Decay asked for")
            .kind(StreamKind::Curve)
            .meta(json!({
                "axis": "log",
                "from": loss::FIT_BOTTOM,
                "to": loss::FIT_TOP,
                "unit": "s",
            })),
        StreamSpec::new("realised", CURVE_POINTS)
            .name("Decay realised")
            .kind(StreamKind::Curve)
            .meta(json!({
                "axis": "log",
                "from": loss::FIT_BOTTOM,
                "to": loss::FIT_TOP,
                "unit": "s",
            })),
        // Appended, never inserted: streams are addressed by index, so a new
        // one in the middle points every existing reader at the wrong data.
        StreamSpec::new("spectrum", CURVE_POINTS)
            .name("The tail")
            .kind(StreamKind::Spectrum)
            .meta(json!({
                "axis": "log",
                "from": loss::FIT_BOTTOM,
                "to": loss::FIT_TOP,
                "unit": "dB",
            })),
    ]
}
