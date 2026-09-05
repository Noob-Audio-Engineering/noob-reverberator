//! The modes: named places in the space the controls already reach.
//!
//! The survey's conclusion was that a mode list is a good way to *ship* tuned
//! points and a poor way to be the only way to reach them. Supermassive's
//! twenty-two descriptions are all written in the same three quantities ---
//! attack, decay length, echo density --- so those are controls here, and a
//! mode is a place to stand rather than a door to a hidden algorithm.
//!
//! Choosing a mode sets every control it names. Everything stays movable
//! afterwards, and the panel says so.
//!
//! # This table is append only
//!
//! A saved state stores the **index**. Inserting a mode in the middle would
//! reopen every project that used a later one as a different reverb. New modes
//! go on the end, for as long as there are modes to add.

use super::shape::Kind as ShapeKind;

/// The topologies. These are the real structural differences --- what a mode
/// cannot reach by turning a knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    /// A feedback delay network: halls, rooms, chambers, clouds, echoes.
    Network,
    /// Dattorro's figure of eight: plates.
    Plate,
    /// A dispersive cascade: springs.
    Spring,
    /// Taps only, no tail: ambiences and reflections.
    Early,
}

/// Everything a mode decides.
#[derive(Debug, Clone, Copy)]
pub struct Mode {
    pub name: &'static str,
    /// One line, in the panel, about what it is for.
    pub blurb: &'static str,
    pub arch: Arch,
    /// How many delay lines, where that means anything.
    pub lines: usize,
    /// The tank's size, as a multiplier on the architecture's own scale.
    pub size: f32,
    /// Seconds, before the decay curve's bands are applied.
    pub decay: f32,
    /// 0 sparse, 1 as dense as the diffusers go.
    pub density: f32,
    /// How long the reverb takes to arrive at full level, in milliseconds.
    pub attack: f32,
    /// Modulation rate in hertz, depth in samples, and how much of the depth
    /// is a random walk rather than a sweep.
    pub mod_rate: f32,
    pub mod_depth: f32,
    pub mod_random: f32,
    /// How loud the early reflections are against the tail.
    pub early: f32,
    /// Semitones the feedback path is transposed by, and how much of it is.
    pub shift: f32,
    pub shift_mix: f32,
    /// An envelope over the wet path.
    pub shape: ShapeKind,
    /// Which era's colouring, and how much of it.
    pub era: usize,
    pub era_amount: f32,
}

/// A mode with everything quiet, to be written over.
const fn base(name: &'static str, blurb: &'static str, arch: Arch) -> Mode {
    Mode {
        name,
        blurb,
        arch,
        lines: 12,
        size: 1.0,
        decay: 2.0,
        density: 0.6,
        attack: 0.0,
        mod_rate: 0.8,
        mod_depth: 6.0,
        mod_random: 0.5,
        early: 0.25,
        shift: 0.0,
        shift_mix: 0.0,
        shape: ShapeKind::Off,
        era: 0,
        era_amount: 0.0,
    }
}

/// The modes, in the order they will always be in.
pub static MODES: &[Mode] = &[
    Mode {
        lines: 16,
        size: 1.6,
        decay: 3.2,
        density: 0.55,
        attack: 24.0,
        mod_rate: 0.5,
        mod_depth: 9.0,
        mod_random: 0.35,
        early: 0.2,
        ..base(
            "Concert Hall",
            "A big room with a slow build and a long, even tail.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 1.5,
        decay: 3.0,
        density: 0.5,
        attack: 18.0,
        mod_rate: 0.9,
        mod_depth: 14.0,
        mod_random: 0.2,
        early: 0.25,
        ..base(
            "Bright Hall",
            "The same hall with more air in it and a deeper sweep.",
            Arch::Network,
        )
    },
    Mode {
        lines: 12,
        size: 0.7,
        decay: 1.4,
        density: 0.85,
        attack: 2.0,
        mod_rate: 1.1,
        mod_depth: 4.0,
        mod_random: 0.3,
        early: 0.45,
        ..base(
            "Room",
            "Close walls, an audible first reflection, and a short tail.",
            Arch::Network,
        )
    },
    Mode {
        lines: 14,
        size: 1.0,
        decay: 2.2,
        density: 0.95,
        attack: 4.0,
        mod_rate: 0.6,
        mod_depth: 5.0,
        mod_random: 0.6,
        early: 0.2,
        ..base(
            "Chamber",
            "Dense from the first instant, the way a hard-walled room is.",
            Arch::Network,
        )
    },
    Mode {
        size: 1.0,
        decay: 2.4,
        density: 0.9,
        mod_rate: 1.3,
        mod_depth: 8.0,
        mod_random: 0.15,
        early: 0.1,
        ..base(
            "Plate",
            "No room at all: full density immediately, bright, and metallic.",
            Arch::Plate,
        )
    },
    Mode {
        size: 0.55,
        decay: 1.2,
        density: 0.7,
        mod_rate: 2.0,
        mod_depth: 3.0,
        mod_random: 0.1,
        early: 0.1,
        ..base(
            "Small Plate",
            "A tighter sheet: shorter, brighter, and more obviously metal.",
            Arch::Plate,
        )
    },
    Mode {
        size: 1.0,
        decay: 1.8,
        density: 0.2,
        early: 0.0,
        ..base(
            "Spring",
            "A coil, which chirps rather than diffuses.",
            Arch::Spring,
        )
    },
    Mode {
        size: 0.5,
        decay: 0.35,
        density: 0.4,
        early: 1.0,
        ..base(
            "Reflections",
            "The walls without the tail: where the room is, not how long.",
            Arch::Early,
        )
    },
    Mode {
        lines: 16,
        size: 1.2,
        decay: 1.1,
        density: 0.75,
        attack: 60.0,
        mod_rate: 1.7,
        mod_depth: 10.0,
        mod_random: 0.8,
        early: 0.6,
        ..base(
            "Ambience",
            "Felt and not heard: early energy, a short tail, and air.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 3.0,
        decay: 12.0,
        density: 0.95,
        attack: 180.0,
        mod_rate: 0.3,
        mod_depth: 20.0,
        mod_random: 0.7,
        early: 0.05,
        ..base(
            "Cloud",
            "Slowest to arrive, longest to leave, and dense all through.",
            Arch::Network,
        )
    },
    Mode {
        lines: 8,
        size: 4.0,
        decay: 9.0,
        density: 0.15,
        mod_rate: 0.2,
        mod_depth: 16.0,
        mod_random: 0.9,
        early: 0.0,
        ..base(
            "Echoverb",
            "Sparse and long: the repeats stay countable for a while.",
            Arch::Network,
        )
    },
    Mode {
        lines: 14,
        size: 1.4,
        decay: 4.0,
        density: 0.8,
        attack: 30.0,
        mod_rate: 0.45,
        mod_depth: 11.0,
        mod_random: 0.4,
        early: 0.1,
        shift: 12.0,
        shift_mix: 0.5,
        ..base(
            "Shimmer",
            "An octave is added to the tail every time round the loop.",
            Arch::Network,
        )
    },
    Mode {
        lines: 14,
        size: 1.4,
        decay: 5.0,
        density: 0.85,
        attack: 40.0,
        mod_rate: 0.4,
        mod_depth: 12.0,
        mod_random: 0.45,
        early: 0.08,
        shift: -12.0,
        shift_mix: 0.45,
        ..base(
            "Undertow",
            "The same trick downwards, which sinks instead of climbing.",
            Arch::Network,
        )
    },
    Mode {
        lines: 12,
        size: 0.8,
        decay: 1.6,
        density: 0.9,
        mod_rate: 1.0,
        mod_depth: 4.0,
        mod_random: 0.3,
        early: 0.3,
        shape: ShapeKind::Gate,
        ..base(
            "Gated",
            "Flat, and then it stops. A room cannot do this.",
            Arch::Network,
        )
    },
    Mode {
        lines: 12,
        size: 1.0,
        decay: 2.0,
        density: 0.85,
        mod_rate: 0.9,
        mod_depth: 6.0,
        early: 0.15,
        shape: ShapeKind::Reverse,
        ..base(
            "Reverse",
            "Into the note rather than away from it.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 2.0,
        decay: 6.0,
        density: 0.9,
        attack: 90.0,
        mod_rate: 0.35,
        mod_depth: 14.0,
        mod_random: 0.6,
        early: 0.05,
        shape: ShapeKind::Swell,
        ..base(
            "Swell",
            "The reverb arrives behind the dry signal and stays.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 1.5,
        decay: 3.5,
        density: 0.6,
        attack: 20.0,
        mod_rate: 0.7,
        mod_depth: 12.0,
        mod_random: 0.25,
        early: 0.2,
        era: 1,
        era_amount: 1.0,
        ..base(
            "Nineteen Seventy Nine",
            "The same hall through the converters of the day: narrow and grainy.",
            Arch::Network,
        )
    },
    Mode {
        size: 1.1,
        decay: 2.6,
        density: 0.9,
        mod_rate: 1.2,
        mod_depth: 9.0,
        mod_random: 0.2,
        early: 0.1,
        era: 2,
        era_amount: 1.0,
        ..base(
            "Nineteen Eighty Four",
            "A plate with the word length of the machine that made it famous.",
            Arch::Plate,
        )
    },
];

/// The names, for the panel and for the host's parameter list.
pub fn names() -> Vec<&'static str> {
    MODES.iter().map(|m| m.name).collect()
}

pub fn get(i: usize) -> &'static Mode {
    &MODES[i.min(MODES.len() - 1)]
}
