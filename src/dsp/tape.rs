//! Magneto: a multi-head tape echo in front of the tank.
//!
//! Strymon's Magneto machine is described as a multi-head echo setup whose
//! pre-delay has its own feedback and diffusion, blending a delay into a
//! reverb. What makes it a *tape* echo rather than a digital one is three
//! things, and all three are here:
//!
//! - **Several heads on one loop.** The repeats are not evenly spaced copies;
//!   they are taps at fixed fractions of a single circulating delay, so
//!   turning heads on and off changes the rhythm without changing the time.
//! - **Wow and flutter.** The tape does not move at a constant speed. Slow
//!   drift and fast jitter move the read position, which detunes each repeat
//!   slightly and differently --- and it compounds, because a repeat that has
//!   been round four times has been detuned four times.
//! - **Loss on every pass.** Each trip through the loop is a trip through
//!   saturation and a bandwidth limit, so a repeat gets darker and squashier
//!   as it goes, rather than simply quieter.
//!
//! It is not a model of any machine. Nobody has measured a tape echo for this
//! project, and the numbers below are chosen to sound like what the survey
//! describes rather than to reproduce a device.

use super::delay::Delay;
use super::filters::OnePole;
use super::modulate::Modulator;

/// The most heads one loop can have.
pub const MAX_HEADS: usize = 4;

/// Where the heads sit, as fractions of the loop. Not evenly spaced: even
/// heads make a flam rather than a rhythm.
const HEAD_AT: [f32; MAX_HEADS] = [0.25, 0.5, 0.75, 1.0];

pub struct Tape {
    line: Delay,
    len: f32,
    heads: usize,
    feedback: f32,
    /// Wow is slow and deep, flutter fast and shallow. Two modulators, because
    /// one of them cannot be both.
    wow: Modulator,
    flutter: Modulator,
    wow_depth: f32,
    flutter_depth: f32,
    /// The loop's own bandwidth and the drive into its saturation.
    tone: OnePole,
    drive: f32,
    fs: f32,
    fb: f32,
}

impl Tape {
    pub fn new(fs: f32, max_ms: f32) -> Self {
        let cap = ((fs * max_ms / 1000.0) as usize + 64).next_power_of_two();
        let mut t = Tape {
            line: Delay::with_capacity(cap),
            len: fs * 0.25,
            heads: 3,
            feedback: 0.35,
            wow: Modulator::new(0x51ED_2701),
            flutter: Modulator::new(0x9E37_79B1),
            wow_depth: 0.0,
            flutter_depth: 0.0,
            tone: OnePole::default(),
            drive: 1.0,
            fs,
            fb: 0.0,
        };
        t.tone.set_cutoff(fs, 6_000.0);
        t
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.tone.reset();
        self.fb = 0.0;
    }

    /// `time_ms` is one lap of the loop --- the last head's delay. `heads` is
    /// how many taps are live, `feedback` how much comes back, `wobble` how
    /// unsteady the transport is, and `drive` how hard the loop is pushed.
    pub fn set(&mut self, time_ms: f32, heads: usize, feedback: f32, wobble: f32, drive: f32) {
        let cap = self.line.capacity() as f32 - 8.0;
        self.len = (time_ms.max(1.0) * self.fs / 1000.0).clamp(16.0, cap);
        self.heads = heads.clamp(1, MAX_HEADS);
        // Held below one. At one a tape echo is a machine that never forgets,
        // and with saturation in the loop that is not a drone, it is a growing
        // square wave.
        self.feedback = feedback.clamp(0.0, 0.98);
        let w = wobble.clamp(0.0, 1.0);
        self.wow.set_rate(self.fs, 0.4 + w * 0.6);
        self.flutter.set_rate(self.fs, 7.0 + w * 6.0);
        // Wow is a fraction of the loop, so a longer setting drifts further in
        // absolute terms --- which is what a tape does, because the error is
        // in the speed and not in the length.
        self.wow_depth = w * self.len * 0.004;
        self.flutter_depth = w * self.len * 0.0006;
        self.drive = 1.0 + drive.clamp(0.0, 1.0) * 6.0;
        self.tone
            .set_cutoff(self.fs, 8_000.0 - drive.clamp(0.0, 1.0) * 4_000.0);
    }

    /// One sample in, the summed heads out.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        // The wobble is on the read, not the write: a tape's speed error moves
        // where the head is over the tape, and applying it to both would
        // cancel it.
        let drift = self.wow.next(self.wow_depth, 0.35, 0.0)
            + self.flutter.next(self.flutter_depth, 0.8, 0.0);

        self.line.push(x + self.fb);

        let mut out = 0.0;
        for &at in HEAD_AT[..self.heads].iter() {
            out += self.line.read((self.len * at + drift).max(2.0));
        }
        let k = 1.0 / self.heads as f32;
        out *= k;

        // What comes back has been through the tape once more: darker, and
        // squashed. `tanh` rather than a clip, because a tape does not have a
        // ceiling it hits, it has a slope that gives way.
        let last = self.line.read((self.len + drift).max(2.0));
        self.fb = (self.tone.process(last) * self.drive).tanh() / self.drive * self.feedback;

        out
    }
}
