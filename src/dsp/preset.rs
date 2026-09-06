//! The presets that ship with the plug-in.
//!
//! A preset here is **a mode plus the controls it moves afterwards**, and it
//! is stored that way rather than as a full parameter dump. Two reasons, and
//! the second is the one that matters:
//!
//! - A dump of every parameter is unreadable, so nobody checks it, so it rots.
//!   These say what they change, which is also what they are *about*.
//! - A dump has to be re-issued whenever a parameter is added, and a preset
//!   one version behind silently restores a default over something a person
//!   set. A preset that names five controls keeps naming five controls
//!   forever.
//!
//! Applying one goes through the ordinary parameter edit path, so the host
//! records it as it would automation and undo works on it. The page owns the
//! button; the engine owns nothing here but the list.

use super::mode::MODES;

/// One preset: which mode it starts from, and what it moves.
pub struct Preset {
    pub name: &'static str,
    /// What it is for, in the panel.
    pub blurb: &'static str,
    /// The mode it starts from, **by name** --- resolved to an index when the
    /// page asks, so reordering the mode table cannot silently repoint it.
    pub mode: &'static str,
    /// Parameter id and plain value, applied over the mode.
    pub set: &'static [(&'static str, f32)],
}

/// The factory presets. **Append only**, like everything else a saved state
/// can refer to by position.
pub static PRESETS: &[Preset] = &[
    Preset {
        name: "Vocal Plate",
        blurb: "A bright sheet, kept out of the way of the words.",
        mode: "Plate",
        set: &[
            ("mix", 22.0),
            ("decay", 1.9),
            ("predelay", 28.0),
            ("band1_on", 1.0),
            ("band1_shape", 1.0),
            ("band1_freq", 220.0),
            ("band1_mult", 0.55),
            ("band2_on", 1.0),
            ("band2_shape", 2.0),
            ("band2_freq", 7000.0),
            ("band2_mult", 0.7),
        ],
    },
    Preset {
        name: "Drum Room",
        blurb: "Close walls and an audible first reflection.",
        mode: "Room",
        set: &[
            ("mix", 28.0),
            ("decay", 0.9),
            ("predelay", 6.0),
            ("early_level", 55.0),
            ("early_size", 5.0),
            ("density", 65.0),
        ],
    },
    Preset {
        name: "Gated Snare",
        blurb: "Flat and then gone, which no room does.",
        mode: "Gated",
        set: &[
            ("mix", 45.0),
            ("decay", 1.6),
            ("shape_hold", 220.0),
            ("density", 95.0),
            ("width", 130.0),
        ],
    },
    Preset {
        name: "Long Bloom",
        blurb: "Arrives late, stays a while, and keeps its bottom end.",
        mode: "Swell",
        set: &[
            ("mix", 40.0),
            ("decay", 7.0),
            ("attack", 220.0),
            ("band1_on", 1.0),
            ("band1_shape", 1.0),
            ("band1_freq", 300.0),
            ("band1_mult", 1.8),
            ("band2_on", 1.0),
            ("band2_shape", 2.0),
            ("band2_freq", 4000.0),
            ("band2_mult", 0.45),
        ],
    },
    Preset {
        name: "Air Only",
        blurb: "Felt and not heard: the top rings and nothing else does.",
        mode: "Ambience",
        set: &[
            ("mix", 30.0),
            ("decay", 1.4),
            ("band1_on", 1.0),
            ("band1_shape", 1.0),
            ("band1_freq", 700.0),
            ("band1_mult", 0.25),
            ("band2_on", 1.0),
            ("band2_shape", 2.0),
            ("band2_freq", 6000.0),
            ("band2_mult", 2.4),
        ],
    },
    Preset {
        name: "Cathedral",
        blurb: "Stone: very long underneath, and the top absorbed away.",
        mode: "Concert Hall",
        set: &[
            ("mix", 38.0),
            ("decay", 8.0),
            ("size", 220.0),
            ("predelay", 45.0),
            ("band1_on", 1.0),
            ("band1_shape", 1.0),
            ("band1_freq", 400.0),
            ("band1_mult", 2.0),
            ("band2_on", 1.0),
            ("band2_shape", 2.0),
            ("band2_freq", 3000.0),
            ("band2_mult", 0.35),
        ],
    },
    Preset {
        name: "Shimmering Pad",
        blurb: "An octave added every time round, over a very long tail.",
        mode: "Shimmer",
        set: &[
            ("mix", 45.0),
            ("decay", 6.0),
            ("shift_mix", 55.0),
            ("attack", 90.0),
            ("mod_depth", 45.0),
        ],
    },
    Preset {
        name: "Surf Spring",
        blurb: "A slack coil and nothing else: all chirp.",
        mode: "Spring",
        set: &[
            ("mix", 35.0),
            ("decay", 1.6),
            ("tension", 35.0),
            ("sections", 130.0),
        ],
    },
    Preset {
        name: "Tape Cave",
        blurb: "The echo is what the room hears, so each repeat has its own tail.",
        mode: "Magneto",
        set: &[
            ("mix", 42.0),
            ("decay", 3.4),
            ("tape_mix", 80.0),
            ("tape_time", 420.0),
            ("tape_feedback", 52.0),
            ("tape_wobble", 45.0),
            ("tape_drive", 55.0),
        ],
    },
    Preset {
        name: "Distant Choir",
        blurb: "Vowels on the tail, a long way off.",
        mode: "Chorale",
        set: &[
            ("mix", 50.0),
            ("decay", 9.0),
            ("choir_amount", 80.0),
            ("choir_vowel", 3.0),
            ("choir_resonance", 65.0),
        ],
    },
    Preset {
        name: "Reverse Swell",
        blurb: "Into the note rather than away from it.",
        mode: "Reverse",
        set: &[
            ("mix", 45.0),
            ("decay", 2.6),
            ("shape_hold", 700.0),
            ("density", 90.0),
        ],
    },
    Preset {
        name: "Nineteen Eighty Three",
        blurb: "A hall through the converters of the day, with the grain left in.",
        mode: "Nineteen Seventy Nine",
        set: &[
            ("mix", 34.0),
            ("decay", 3.2),
            ("era", 2.0),
            ("era_amount", 100.0),
            ("mod_depth", 40.0),
        ],
    },
    Preset {
        name: "Restless Hall",
        blurb: "A long tail that never settles into a ring.",
        mode: "Chaotic Hall",
        set: &[
            ("mix", 38.0),
            ("decay", 6.5),
            ("mod_chaos", 85.0),
            ("mod_depth", 45.0),
        ],
    },
    Preset {
        name: "Driven Plate",
        blurb: "Hit hard enough that the transducer gives way.",
        mode: "Plate",
        set: &[
            ("mix", 42.0),
            ("decay", 2.8),
            ("thickness", 80.0),
            ("density", 85.0),
        ],
    },
    Preset {
        name: "Out Of The Way",
        blurb: "Ducks under the take and comes back between the words.",
        mode: "Concert Hall",
        set: &[
            ("mix", 46.0),
            ("decay", 3.4),
            ("duck", 70.0),
            ("duck_release", 320.0),
            ("predelay", 35.0),
        ],
    },
    Preset {
        name: "On The Eighth",
        blurb: "A pre-delay that follows the session rather than the millisecond.",
        mode: "Chamber",
        set: &[
            ("mix", 40.0),
            ("decay", 2.2),
            ("predelay_sync", 4.0),
            ("early_level", 45.0),
        ],
    },
    Preset {
        name: "Cut It Short",
        blurb: "A big room with the tail taken off it.",
        mode: "Supermassive",
        set: &[
            ("mix", 44.0),
            ("decay", 8.0),
            ("gate", 85.0),
            ("gate_hold", 180.0),
        ],
    },
];

/// The presets as JSON for the page, with each mode resolved to its index.
///
/// A preset naming a mode that does not exist is a bug in this file, so it is
/// dropped rather than shipped as a button that does nothing --- and
/// [`every_preset_resolves`](super::tests) fails when one is.
pub fn factory_json() -> serde_json::Value {
    let list: Vec<serde_json::Value> = PRESETS
        .iter()
        .filter_map(|p| {
            let mode = MODES.iter().position(|m| m.name == p.mode)?;
            let set: Vec<serde_json::Value> = p
                .set
                .iter()
                .map(|(id, v)| serde_json::json!([id, v]))
                .collect();
            Some(serde_json::json!({
                "name": p.name,
                "blurb": p.blurb,
                "mode": mode,
                "set": set,
            }))
        })
        .collect();
    serde_json::Value::Array(list)
}
