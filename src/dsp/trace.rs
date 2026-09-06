//! Where the signal is at every stage, for telling two reverbs apart.
//!
//! Knowing that two modes produce different audio is a weak thing to know.
//! `every_mode_is_a_different_sound` proves it and cannot say *why*: two modes
//! could differ because one has a tape in front of it, or because the tank is
//! twice the size, or because a shifter is running --- and the difference
//! between "these are two rooms" and "these are the same room with a filter
//! on it" is exactly that.
//!
//! So the engine taps itself. Each stage reports what it handed on, and the
//! result is a profile of the whole journey: what the pre-delay passed, what
//! the bloom added, what the tank returned, what the choir did to it. Two
//! modes are then comparable stage by stage, and a mode that is only nominally
//! different shows up as one whose profile matches another's everywhere.
//!
//! # It costs the plug-in nothing
//!
//! The taps go through a trait with a do-nothing implementation, and
//! [`Reverb::process`](super::engine::Reverb::process) uses that one. Every
//! call is an empty function on a zero-sized type, which the optimiser removes
//! along with the arithmetic that fed it. Diagnostics that slow the audio
//! thread get switched off and then rot; these cannot, because there is
//! nothing to switch off.
//!
//! `a_traced_render_is_the_same_audio_as_an_untraced_one` holds the two paths
//! to the sample, which is the property that makes a measurement taken here
//! worth anything.

/// A point in the signal's journey through the engine.
///
/// **Append only.** A report is written in this order and read by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// What the host handed over, after being made finite.
    Input,
    /// After the pre-delay, which is the gap before the room answers.
    PreDelay,
    /// After the bloom generator, which swells in front of the tank.
    Bloom,
    /// After the tape, whose repeats are what the room hears.
    Tape,
    /// The early reflections, on their own branch.
    Early,
    /// What actually goes into the tank, after freeze and the input drive.
    TankIn,
    /// What the tank returned, before anything is done to it.
    TankOut,
    /// After the pitch shifter has been mixed into the tail.
    Shift,
    /// After the choir's formants.
    Choir,
    /// After the era colouring.
    Colour,
    /// The wet signal as it leaves, after the envelope, ducker and gate.
    Wet,
}

impl Stage {
    /// Every stage, in the order the signal meets them.
    pub const ALL: [Stage; 11] = [
        Stage::Input,
        Stage::PreDelay,
        Stage::Bloom,
        Stage::Tape,
        Stage::Early,
        Stage::TankIn,
        Stage::TankOut,
        Stage::Shift,
        Stage::Choir,
        Stage::Colour,
        Stage::Wet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Input => "input",
            Stage::PreDelay => "pre-delay",
            Stage::Bloom => "bloom",
            Stage::Tape => "tape",
            Stage::Early => "early",
            Stage::TankIn => "tank in",
            Stage::TankOut => "tank out",
            Stage::Shift => "shift",
            Stage::Choir => "choir",
            Stage::Colour => "colour",
            Stage::Wet => "wet",
        }
    }
}

/// Somewhere for the engine to report what a stage handed on.
pub trait Tap {
    fn at(&mut self, stage: Stage, l: f32, r: f32);
}

/// The implementation the plug-in uses: nothing at all.
///
/// Zero-sized, and every call is an empty body, so the optimiser deletes both
/// the call and whatever was computed only to feed it.
pub struct NoTap;

impl Tap for NoTap {
    #[inline(always)]
    fn at(&mut self, _stage: Stage, _l: f32, _r: f32) {}
}

/// What one stage did, over however many samples were traced.
#[derive(Debug, Default, Clone, Copy)]
pub struct Level {
    pub peak: f32,
    sum_sq: f64,
    /// How far the two sides drifted apart, summed.
    sum_diff_sq: f64,
    n: u64,
}

impl Level {
    pub fn rms(&self) -> f32 {
        if self.n == 0 {
            return 0.0;
        }
        (self.sum_sq / self.n as f64).sqrt() as f32
    }

    /// How different the two channels are, against the level itself: 0 is
    /// mono and 1 is fully apart. A tank that is not really stereo shows here.
    pub fn width(&self) -> f32 {
        if self.n == 0 {
            return 0.0;
        }
        let d = (self.sum_diff_sq / self.n as f64).sqrt() as f32;
        d / self.rms().max(1e-9)
    }

    /// Peak against RMS, in decibels. A tank full of discrete echoes has a
    /// high crest; a dense one does not.
    pub fn crest_db(&self) -> f32 {
        20.0 * (self.peak / self.rms().max(1e-12)).max(1e-9).log10()
    }
}

/// A running profile of the whole journey.
#[derive(Debug, Default, Clone)]
pub struct Trace {
    levels: [Level; Stage::ALL.len()],
}

impl Trace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn level(&self, stage: Stage) -> &Level {
        &self.levels[stage as usize]
    }

    /// What each stage added or took away, in decibels, against the one
    /// before it. This is the column that says where a mode's character comes
    /// from: a mode with a tape shows the tape doing something and a mode
    /// without shows it doing nothing at all.
    pub fn gain_db(&self, stage: Stage) -> f32 {
        let i = stage as usize;
        if i == 0 {
            return 0.0;
        }
        let before = self.levels[i - 1].rms();
        let after = self.levels[i].rms();
        if before <= 1e-12 {
            return 0.0;
        }
        20.0 * (after / before).max(1e-9).log10()
    }
}

impl Tap for Trace {
    #[inline]
    fn at(&mut self, stage: Stage, l: f32, r: f32) {
        let s = &mut self.levels[stage as usize];
        let m = 0.5 * (l + r);
        s.peak = s.peak.max(l.abs()).max(r.abs());
        s.sum_sq += (m * m) as f64;
        let d = 0.5 * (l - r);
        s.sum_diff_sq += (d * d) as f64;
        s.n += 1;
    }
}
