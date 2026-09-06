//! Bloom: a section that builds before the tank hears it.
//!
//! Strymon's Bloom machine is described as a "bloom generating" section with
//! its own feedback feeding the reverb tank. What that is for is a sound that
//! **grows** rather than one that starts loud and falls: the note arrives, the
//! generator regenerates it a few times with each pass a little louder, and
//! only then does the reverb get its full input.
//!
//! # Why this is not the tape, and not the attack control
//!
//! [`Tape`](super::tape::Tape) has fixed feedback, so its repeats always fall
//! away. A bloom's feedback **rises after an onset** and then lets go, which
//! is why it swells.
//!
//! The attack control is a fade on the wet path: the same tail, arriving
//! gradually. A bloom is a different tail --- the tank is fed more, later, so
//! the reverb it makes is denser and longer than the one the dry signal would
//! have produced on its own. That is audible as growth rather than as a fade,
//! and it is why both are worth having.

use super::delay::Delay;
use super::filters::OnePole;

pub struct Bloom {
    line: Delay,
    len: f32,
    /// The input follower, which is only used to spot an onset.
    env: f32,
    env_k: f32,
    prev: f32,
    /// How far through the swell we are, in samples since the last onset.
    phase: f32,
    swell: f32,
    /// How far the feedback rises at the top of a swell, and where it rests.
    depth: f32,
    floor: f32,
    /// One pole in the loop, so a bloom darkens as it grows rather than
    /// turning into a whistle.
    tone: OnePole,
    fb: f32,
    fs: f32,
}

impl Bloom {
    pub fn new(fs: f32, max_ms: f32) -> Self {
        let cap = ((fs * max_ms / 1000.0) as usize + 64).next_power_of_two();
        let mut b = Bloom {
            line: Delay::with_capacity(cap),
            len: fs * 0.12,
            env: 0.0,
            env_k: (-1.0f32 / (0.003 * fs)).exp(),
            prev: 0.0,
            phase: f32::MAX,
            swell: fs * 0.6,
            depth: 0.0,
            floor: 0.0,
            tone: OnePole::default(),
            fb: 0.0,
            fs,
        };
        b.set(0.0, 120.0, 600.0);
        b
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.tone.reset();
        self.env = 0.0;
        self.prev = 0.0;
        self.phase = f32::MAX;
        self.fb = 0.0;
    }

    /// `amount` is how much the feedback swells, `time_ms` one pass of the
    /// generator, and `swell_ms` how long it takes to reach full.
    pub fn set(&mut self, amount: f32, time_ms: f32, swell_ms: f32) {
        let cap = self.line.capacity() as f32 - 8.0;
        self.len = (time_ms.max(1.0) * self.fs / 1000.0).clamp(8.0, cap);
        let a = amount.clamp(0.0, 1.0);
        // Held below one even at the top of a swell: a loop that reaches
        // unity does not bloom, it sits there.
        self.depth = a * 0.85;
        self.floor = a * 0.10;
        self.swell = (swell_ms.max(1.0) * 0.001 * self.fs).max(16.0);
    }

    /// One sample in, what the tank should hear out.
    ///
    /// # The swell is triggered, not followed
    ///
    /// The first version drove the feedback straight from an envelope of the
    /// input, and it did not bloom at all: the moment the note stopped the
    /// envelope fell, the feedback fell with it and the loop died. Measured,
    /// a burst was **0.80 just after and 0.002 half a second later** --- the
    /// opposite of the machine's whole point.
    ///
    /// A bloom is a *triggered* shape. An onset starts it, and from there it
    /// grows on its own for `swell_ms` whether or not anything else is
    /// played, then lets go. That is what makes the sound arrive after the
    /// note rather than with it.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        if self.depth <= 0.0 {
            return x;
        }
        // An onset: the follower has risen sharply from a low level.
        let a = x.abs();
        self.env = a.max(self.env * self.env_k);
        if self.env > self.prev * 1.5 && self.env > 1e-3 {
            self.phase = 0.0;
        }
        self.prev = self.prev * 0.9995 + self.env * 0.0005;

        let u = self.phase / self.swell;
        self.phase += 1.0;
        // Up over one swell, then away over the next three. The peak is where
        // the loop is closest to sustaining itself, which is where a bloom is
        // loudest --- after the note, not on it.
        let shape = if u < 1.0 {
            u * u * (3.0 - 2.0 * u)
        } else {
            (-(u - 1.0) / 3.0).exp()
        };

        self.line.push(x + self.fb);
        let out = self.line.read(self.len);
        let g = (self.floor + self.depth * shape).min(0.97);
        self.fb = self.tone.process(out) * g;
        // The generator's output *and* the dry input, or the attack
        // disappears into the swell.
        x + out
    }
}
