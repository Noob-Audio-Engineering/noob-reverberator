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

use super::decay::Shape;
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

/// One band of a mode's decay curve.
///
/// A mode is a *place*, and the thing that most separates one place from
/// another is not how long it rings but **how long it rings at each
/// frequency**. Air absorbs the top of the band, soft rooms absorb more of it,
/// steel plates barely absorb it at all and lose the bottom instead, and a
/// spring is a resonance in the middle with nothing either side. Until these
/// existed every mode here decayed at the same rate at 125 Hz as at 4 kHz ---
/// the tilt across the whole table spanned a quarter of an octave --- and the
/// thirty-two names were tonally one reverb.
///
/// `mult` is a multiplier on the decay time in that band: 0.5 is half as long,
/// 2.0 twice. **`mult` of exactly 1.0 means the band is off**, which is what
/// leaves a mode deliberately flat.
#[derive(Debug, Clone, Copy)]
pub struct Tone {
    pub shape: Shape,
    pub freq: f32,
    pub mult: f32,
    pub width: f32,
}

/// A band that does nothing.
pub const NO_TONE: Tone = Tone {
    shape: Shape::Bell,
    freq: 1_000.0,
    mult: 1.0,
    width: 1.0,
};

/// How many curve bands a mode may set. The rest of the thirty-two stay the
/// listener's own: a mode owns the bottom three slots and nothing above them,
/// so a curve drawn by hand survives changing mode.
pub const MODE_TONES: usize = 3;

const fn hs(freq: f32, mult: f32) -> Tone {
    Tone {
        shape: Shape::HighShelf,
        freq,
        mult,
        width: 1.0,
    }
}
const fn ls(freq: f32, mult: f32) -> Tone {
    Tone {
        shape: Shape::LowShelf,
        freq,
        mult,
        width: 1.0,
    }
}
const fn bell(freq: f32, mult: f32, width: f32) -> Tone {
    Tone {
        shape: Shape::Bell,
        freq,
        mult,
        width,
    }
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
    /// How much of the input goes through the tape before the tank.
    pub tape: f32,
    /// How much vowel is put on the tail, and which one.
    pub choir: f32,
    pub vowel: f32,
    /// How far away the source is, and how thick the tank runs.
    pub distance: f32,
    pub thickness: f32,
    /// How much the generator in front of the tank swells.
    pub bloom: f32,
    /// The decay curve this mode stands for. See [`Tone`].
    pub tone: [Tone; MODE_TONES],
    /// How unsteady the modulation runs. Neither a sweep nor a walk: a drift
    /// well below the rate with a flutter well above it, which is what stops
    /// a long tail settling into a steady ring.
    pub chaos: f32,
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
        tape: 0.0,
        choir: 0.0,
        vowel: 0.0,
        distance: 0.0,
        thickness: 0.0,
        bloom: 0.0,
        tone: [NO_TONE; MODE_TONES],
        chaos: 0.0,
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
        tone: [hs(2000.0, 0.45), ls(120.0, 1.25), NO_TONE],
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
        tone: [hs(3000.0, 0.80), ls(140.0, 1.10), NO_TONE],
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
        tone: [hs(2500.0, 0.50), ls(150.0, 1.15), NO_TONE],
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
        tone: [hs(2000.0, 0.60), ls(100.0, 1.30), NO_TONE],
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
        tone: [hs(4000.0, 0.95), ls(200.0, 0.55), NO_TONE],
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
        tone: [hs(5000.0, 1.00), ls(300.0, 0.45), NO_TONE],
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
        tone: [bell(1500.0, 1.45, 1.5), hs(3000.0, 0.40), ls(150.0, 0.50)],
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
        tone: [NO_TONE, NO_TONE, NO_TONE],
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
        tone: [hs(4000.0, 0.70), NO_TONE, NO_TONE],
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
        tone: [hs(1500.0, 0.50), ls(80.0, 1.40), NO_TONE],
        ..base(
            "Cloud",
            "Slow to arrive, long to leave, and dense all through.",
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
        tone: [hs(2500.0, 0.60), NO_TONE, NO_TONE],
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
        tone: [hs(3000.0, 1.50), ls(200.0, 0.70), NO_TONE],
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
        tone: [ls(150.0, 1.80), hs(1200.0, 0.35), NO_TONE],
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
        tone: [hs(5000.0, 0.80), NO_TONE, NO_TONE],
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
        tone: [hs(4000.0, 0.85), NO_TONE, NO_TONE],
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
        tone: [hs(2000.0, 0.55), NO_TONE, NO_TONE],
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
        tone: [hs(1800.0, 0.40), ls(120.0, 1.20), NO_TONE],
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
        tone: [hs(3000.0, 0.75), NO_TONE, NO_TONE],
        ..base(
            "Nineteen Eighty Four",
            "A plate with the word length of the machine that made it famous.",
            Arch::Plate,
        )
    },
    Mode {
        lines: 12,
        size: 1.1,
        decay: 2.2,
        density: 0.55,
        attack: 6.0,
        mod_rate: 0.6,
        mod_depth: 7.0,
        mod_random: 0.4,
        early: 0.15,
        tape: 0.75,
        tone: [hs(2000.0, 0.45), ls(100.0, 1.30), NO_TONE],
        ..base(
            "Magneto",
            "A multi-head tape echo in front of the tank: every repeat gets its own tail.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 2.2,
        decay: 7.0,
        density: 0.9,
        attack: 120.0,
        mod_rate: 0.25,
        mod_depth: 15.0,
        mod_random: 0.6,
        early: 0.05,
        choir: 0.7,
        vowel: 0.0,
        tone: [bell(800.0, 1.50, 1.8), hs(3000.0, 0.50), NO_TONE],
        ..base(
            "Chorale",
            "Vowels on the tail, so it sings rather than rings. Sweep Vowel.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 2.4,
        decay: 9.0,
        density: 0.95,
        attack: 150.0,
        mod_rate: 0.3,
        mod_depth: 16.0,
        mod_random: 0.5,
        early: 0.04,
        shift: 12.0,
        shift_mix: 0.45,
        choir: 0.55,
        vowel: 2.0,
        tone: [ls(150.0, 1.60), hs(2500.0, 0.50), NO_TONE],
        ..base(
            "Choir Loft",
            "The chorale an octave up as well: a room full of people who are not there.",
            Arch::Network,
        )
    },
    Mode {
        lines: 10,
        size: 0.9,
        decay: 2.0,
        density: 0.5,
        attack: 4.0,
        mod_rate: 0.8,
        mod_depth: 6.0,
        mod_random: 0.35,
        early: 0.3,
        tape: 0.6,
        era: 1,
        era_amount: 0.8,
        tone: [hs(1800.0, 0.40), ls(110.0, 1.25), NO_TONE],
        ..base(
            "Tape Chamber",
            "The tape echo through a chamber, with the converters of the day on it.",
            Arch::Network,
        )
    },
    Mode {
        size: 1.4,
        decay: 3.0,
        density: 0.35,
        early: 0.0,
        tone: [bell(1200.0, 1.50, 1.5), hs(2500.0, 0.35), ls(120.0, 0.45)],
        ..base(
            "Long Spring",
            "A slacker, longer coil: more chirp, and further to travel.",
            Arch::Spring,
        )
    },
    Mode {
        lines: 16,
        size: 3.6,
        decay: 30.0,
        density: 0.97,
        attack: 260.0,
        mod_rate: 0.18,
        mod_depth: 24.0,
        mod_random: 0.75,
        early: 0.02,
        tone: [hs(1200.0, 0.60), ls(60.0, 1.50), NO_TONE],
        ..base(
            "Supermassive",
            "Slowest to arrive and half a minute to leave. Freeze holds it.",
            Arch::Network,
        )
    },
    Mode {
        size: 0.75,
        decay: 0.9,
        density: 0.25,
        early: 0.6,
        // The low shelf is gentle on purpose. A plate is the loosest of the
        // four architectures at holding a drawn curve, and this is the
        // shortest plate here --- asking it for 0.6 at 250 Hz measured 0.67
        // octaves out even with every colouring stripped, which is a mode
        // asking its architecture for something it cannot do.
        tone: [hs(4000.0, 1.10), ls(250.0, 0.85), NO_TONE],
        ..base(
            "Nonlinear Plate",
            "A short bright sheet with the gate on: eighties drums, unashamedly.",
            Arch::Plate,
        )
    },
    Mode {
        lines: 8,
        size: 0.35,
        decay: 0.45,
        density: 0.95,
        attack: 0.0,
        mod_rate: 1.5,
        mod_depth: 2.0,
        mod_random: 0.2,
        early: 0.7,
        tone: [hs(6000.0, 0.90), NO_TONE, NO_TONE],
        ..base(
            "Tight Ambience",
            "Barely a reverb: a size and a little air, for things that must stay dry.",
            Arch::Network,
        )
    },
    Mode {
        lines: 14,
        size: 1.3,
        decay: 3.6,
        density: 0.7,
        attack: 20.0,
        mod_rate: 0.5,
        mod_depth: 9.0,
        mod_random: 0.45,
        early: 0.1,
        bloom: 0.75,
        tone: [hs(2500.0, 0.65), ls(120.0, 1.20), NO_TONE],
        ..base(
            "Bloom",
            "A generator swells in front of the tank, so the reverb arrives after the note.",
            Arch::Network,
        )
    },
    Mode {
        lines: 16,
        size: 2.0,
        decay: 5.0,
        density: 0.6,
        attack: 10.0,
        mod_rate: 0.4,
        mod_depth: 10.0,
        mod_random: 0.55,
        early: 0.35,
        distance: 0.8,
        tone: [hs(1500.0, 0.30), ls(100.0, 1.30), NO_TONE],
        ..base(
            "Far Hall",
            "The same hall from the back of it: less pattern, longer build, more diffuse.",
            Arch::Network,
        )
    },
    Mode {
        lines: 12,
        size: 1.0,
        decay: 2.4,
        density: 0.8,
        attack: 4.0,
        mod_rate: 0.9,
        mod_depth: 7.0,
        mod_random: 0.3,
        early: 0.2,
        thickness: 0.8,
        tone: [hs(1800.0, 0.50), ls(90.0, 1.40), NO_TONE],
        ..base(
            "Thick Chamber",
            "Driven into its own saturation: denser, warmer, and less polite.",
            Arch::Network,
        )
    },
    // The last three are one idea: modulation that never repeats. A steady
    // sweep at a long decay is audible as a sweep, and a random walk loosens
    // the ring without moving the pitch, so neither of them is *unsteady* ---
    // they are both regular, one in pitch and one in level. These run the
    // tank off a drift and a flutter at once, which is the thing a long tail
    // needs if it is not to settle.
    Mode {
        lines: 16,
        size: 1.6,
        decay: 3.6,
        density: 0.75,
        mod_rate: 0.7,
        mod_depth: 9.0,
        mod_random: 0.4,
        chaos: 0.7,
        early: 0.28,
        tone: [hs(2200.0, 0.50), ls(130.0, 1.20), NO_TONE],
        ..base(
            "Chaotic Hall",
            "A hall that will not hold still: the tail wanders instead of ringing.",
            Arch::Network,
        )
    },
    Mode {
        lines: 12,
        size: 0.9,
        decay: 1.9,
        density: 0.85,
        mod_rate: 1.1,
        mod_depth: 6.0,
        mod_random: 0.35,
        chaos: 0.8,
        early: 0.3,
        tone: [hs(2000.0, 0.55), ls(110.0, 1.25), NO_TONE],
        ..base(
            "Chaotic Chamber",
            "The same restlessness in a small room, where it reads as life rather than drift.",
            Arch::Network,
        )
    },
    Mode {
        lines: 14,
        size: 1.2,
        decay: 2.8,
        density: 0.7,
        mod_rate: 0.8,
        mod_depth: 7.0,
        mod_random: 0.5,
        chaos: 0.55,
        early: 0.0,
        tone: [NO_TONE, NO_TONE, NO_TONE],
        ..base(
            "Chaotic Neutral",
            "No room in particular, no reflections, and never twice the same.",
            Arch::Network,
        )
    },
];

/// Every parameter a mode stands for, in the plain units the controls use.
///
/// **This is what makes a mode a mode.** Selecting one used to set the `mode`
/// parameter and nothing else, and the only thing the engine read it for was
/// which of the four architectures to run --- so all twenty-five network modes
/// produced bit-for-bit identical audio, and thirty-two names were four
/// sounds. `apply_mode` existed and was correct, and nothing outside the
/// benchmark and the tests ever called it.
///
/// The list is applied by the page, through the ordinary parameter path, for
/// the same reason a preset is: the host then records it exactly as it would a
/// person turning the knobs, so undo works, automation sees it, and the
/// session saves what you can hear. An engine that quietly used a mode's
/// values instead would leave every knob on the panel disagreeing with the
/// sound.
///
/// Everything a mode sets stays movable afterwards, which the README has
/// always claimed and this is what finally makes true.
pub fn param_set(i: usize) -> Vec<(String, f32)> {
    let m = &MODES[i.min(MODES.len() - 1)];
    let mut v: Vec<(String, f32)> = vec![
        ("decay".into(), m.decay),
        ("size".into(), m.size * 100.0),
        ("density".into(), m.density * 100.0),
        ("attack".into(), m.attack),
        ("mod_rate".into(), m.mod_rate),
        // The mode table writes modulation depth in samples and the control is
        // a percentage of twenty of them.
        ("mod_depth".into(), m.mod_depth * 5.0),
        ("mod_random".into(), m.mod_random * 100.0),
        ("mod_chaos".into(), m.chaos * 100.0),
        ("early_level".into(), m.early * 100.0),
        ("shift".into(), m.shift),
        ("shift_mix".into(), m.shift_mix * 100.0),
        ("shape".into(), m.shape as usize as f32),
        ("era".into(), m.era as f32),
        ("era_amount".into(), m.era_amount * 100.0),
        ("lines".into(), m.lines as f32),
        ("tape_mix".into(), m.tape * 100.0),
        ("choir_amount".into(), m.choir * 100.0),
        ("choir_vowel".into(), m.vowel),
        ("distance".into(), m.distance * 100.0),
        ("thickness".into(), m.thickness * 100.0),
        ("bloom".into(), m.bloom * 100.0),
    ];
    // The mode owns the bottom three curve slots and nothing above them, so a
    // band drawn by hand survives changing mode.
    for (k, t) in m.tone.iter().enumerate() {
        let n = k + 1;
        let on = (t.mult - 1.0).abs() > 1e-6;
        v.push((format!("band{n}_on"), if on { 1.0 } else { 0.0 }));
        if on {
            v.push((format!("band{n}_shape"), t.shape as usize as f32));
            v.push((format!("band{n}_freq"), t.freq));
            v.push((format!("band{n}_mult"), t.mult));
            v.push((format!("band{n}_width"), t.width));
        }
    }
    v
}

/// The names, for the panel and for the host's parameter list.
pub fn names() -> Vec<&'static str> {
    MODES.iter().map(|m| m.name).collect()
}
