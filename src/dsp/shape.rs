//! The envelopes that make a reverb do something a room cannot.
//!
//! Three of the survey's machines are not spaces at all --- they are a reverb
//! with a shape imposed on its level over time:
//!
//! - **Gate** --- the tail runs flat and then stops. A room's decay is
//!   exponential; this one is not, which is why it sounds like the 1980s.
//! - **Reverse** --- the tail swells into the note instead of away from it.
//! - **Ramp** and **swoosh** --- in between, and the "physics-defying shapes"
//!   Strymon's Nonlinear machine names.
//! - **Swell** --- the whole reverb fades in behind the dry signal, which is
//!   Strymon's Swell machine and what Valhalla's Attack control reaches when
//!   it is turned up.
//!
//! All of them are one thing: a gain applied to the wet path, driven by how
//! long ago the input arrived. What differs is the curve.
//!
//! # Why this is driven by an envelope follower and not by a note
//!
//! A plug-in has no notes. It has audio, and the only thing it can know is
//! that the input got louder --- so the shape is retriggered by an onset, and
//! runs from there. That is what makes a gated reverb work on a snare and
//! misbehave on a sustained pad, and it is a property of the effect rather
//! than a shortcoming of this one.

/// The shapes on offer. **Append only**: a saved state stores the index.
pub const SHAPE_NAMES: [&str; 6] = ["Off", "Gate", "Reverse", "Ramp", "Swoosh", "Swell"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Off,
    Gate,
    Reverse,
    Ramp,
    Swoosh,
    Swell,
}

impl Kind {
    pub fn from_index(i: usize) -> Kind {
        match i {
            1 => Kind::Gate,
            2 => Kind::Reverse,
            3 => Kind::Ramp,
            4 => Kind::Swoosh,
            5 => Kind::Swell,
            _ => Kind::Off,
        }
    }
}

pub struct Shaper {
    kind: Kind,
    /// How long the shape lasts, in samples.
    hold: f32,
    /// How far through it we are, in samples. Past `hold` the gain is settled.
    t: f32,
    /// The envelope follower that decides when a new note has arrived.
    env: f32,
    prev: f32,
    attack: f32,
    release: f32,
    fs: f32,
    /// Smoothing on the output gain, so no shape has a step in it.
    gain: f32,
}

impl Shaper {
    pub fn new(fs: f32) -> Self {
        Shaper {
            kind: Kind::Off,
            hold: fs * 0.4,
            t: f32::MAX,
            env: 0.0,
            prev: 0.0,
            attack: (-1.0f32 / (0.002 * fs)).exp(),
            release: (-1.0f32 / (0.150 * fs)).exp(),
            fs,
            gain: 1.0,
        }
    }

    pub fn set(&mut self, kind: Kind, hold_ms: f32) {
        if kind != self.kind {
            self.kind = kind;
            // A shape that has just been chosen starts settled rather than
            // mid-sweep, so switching does not fire a swell nobody asked for.
            self.t = f32::MAX;
        }
        self.hold = (hold_ms.max(1.0) * self.fs / 1000.0).max(16.0);
    }

    pub fn reset(&mut self) {
        self.t = f32::MAX;
        self.env = 0.0;
        self.prev = 0.0;
        self.gain = 1.0;
    }

    /// The gain the wet path should have this sample, given the dry input.
    #[inline]
    pub fn next(&mut self, x: f32) -> f32 {
        if self.kind == Kind::Off {
            return 1.0;
        }
        let a = x.abs();
        let k = if a > self.env {
            self.attack
        } else {
            self.release
        };
        self.env = a + (self.env - a) * k;

        // An onset: the follower has risen sharply from a low level. Both
        // conditions matter --- rising alone retriggers on every swell of a
        // pad, and "loud" alone never retriggers at all.
        if self.env > self.prev * 1.6 && self.env > 1e-3 {
            self.t = 0.0;
        }
        self.prev = self
            .prev
            .max(self.env * 0.999)
            .min(self.env.max(self.prev * 0.999));

        let p = (self.t / self.hold).min(1.0);
        self.t += 1.0;

        let want = match self.kind {
            Kind::Off => 1.0,
            // Flat, then off. The edge is smoothed by the follower below
            // rather than being a step, because a step is a click.
            Kind::Gate => {
                if p < 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
            // Into the note rather than away from it.
            Kind::Reverse => p * p,
            Kind::Ramp => 1.0 - p,
            // Up and then away: the shape Strymon call a swoosh.
            Kind::Swoosh => (std::f32::consts::PI * p).sin(),
            // In behind the dry signal, and stays.
            Kind::Swell => p,
        };

        // One-pole towards the target: every shape above is discontinuous
        // somewhere, and this is what stops that being audible as a click.
        let s = (-1.0f32 / (0.005 * self.fs)).exp();
        self.gain = want + (self.gain - want) * s;
        self.gain
    }
}
