//! Getting the tail out of the way: ducking, and a gate on the tail.
//!
//! Two of Pro-R's controls, and they solve the same problem from opposite
//! ends. A reverb long enough to be worth having is long enough to sit on top
//! of the thing that caused it, and the usual answer is to reach for a
//! compressor on the return with the dry signal in its sidechain. Both of
//! these are that, built in and knowing what the dry signal is without being
//! routed it.
//!
//! - **Ducking** pulls the wet down while the dry is loud and lets it back as
//!   the dry falls away. A vocal keeps its own space and the space arrives
//!   between the words.
//! - **The gate** cuts the tail once it falls below a threshold, which is
//!   what stops a long reverb from filling every gap in a mix. Its threshold
//!   follows the signal rather than being a number to set, because the number
//!   somebody wants depends on how loud they printed the track.
//!
//! # Why the detector is the dry signal and not the wet
//!
//! Ducking from the wet is a compressor on the return: the tail pulls itself
//! down and then releases into its own decay, which pumps. Ducking from the
//! dry gives what the control is actually for --- the reverb stays out of the
//! way of the source and not out of the way of itself.

use super::filters::OnePole;

/// Pull the wet down while the dry is loud.
pub struct Ducker {
    env: f32,
    attack: f32,
    release: f32,
    amount: f32,
    /// Smoothing on the gain itself, so a fast envelope cannot step.
    smooth: OnePole,
    fs: f32,
}

impl Ducker {
    pub fn new(fs: f32) -> Self {
        let mut d = Ducker {
            env: 0.0,
            attack: 0.0,
            release: 0.0,
            amount: 0.0,
            smooth: OnePole::default(),
            fs,
        };
        d.set(0.0, 5.0, 250.0);
        d
    }

    /// `amount` is how far down it pulls at full scale, 0 to 1. `attack_ms`
    /// and `release_ms` are the detector's.
    pub fn set(&mut self, amount: f32, attack_ms: f32, release_ms: f32) {
        self.amount = amount.clamp(0.0, 1.0);
        self.attack = (-1.0f32 / (attack_ms.max(0.1) * 0.001 * self.fs)).exp();
        self.release = (-1.0f32 / (release_ms.max(1.0) * 0.001 * self.fs)).exp();
        self.smooth.set_cutoff(self.fs, 60.0);
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
        self.smooth.reset();
    }

    /// The gain the wet path should have, given the dry input.
    #[inline]
    pub fn next(&mut self, dry: f32) -> f32 {
        if self.amount <= 0.0 {
            return 1.0;
        }
        let a = dry.abs();
        let k = if a > self.env {
            self.attack
        } else {
            self.release
        };
        self.env = a + (self.env - a) * k;
        // Straight proportional ducking rather than a threshold and a ratio:
        // what this control is asked for is "keep out of the way", and a
        // threshold turns that into two more numbers to get right.
        let g = 1.0 - self.amount * self.env.clamp(0.0, 1.0);
        self.smooth.process(g)
    }
}

/// Cut the tail once it falls below a threshold that follows the signal.
pub struct AutoGate {
    /// The loudest the dry has been lately, which is what the threshold is
    /// relative to.
    peak: f32,
    decay: f32,
    env: f32,
    env_k: f32,
    open_for: f32,
    hold: f32,
    gain: f32,
    depth: f32,
    smooth: f32,
    fs: f32,
}

impl AutoGate {
    pub fn new(fs: f32) -> Self {
        let mut g = AutoGate {
            peak: 0.0,
            decay: 0.0,
            env: 0.0,
            env_k: 0.0,
            open_for: 0.0,
            hold: 0.0,
            gain: 1.0,
            depth: 0.0,
            smooth: 0.0,
            fs,
        };
        g.set(0.0, 300.0);
        g
    }

    /// `depth` is how far it closes, 0 to 1. `hold_ms` is how long it stays
    /// open after the signal has gone.
    pub fn set(&mut self, depth: f32, hold_ms: f32) {
        self.depth = depth.clamp(0.0, 1.0);
        self.hold = hold_ms.max(1.0) * 0.001 * self.fs;
        self.env_k = (-1.0f32 / (0.010 * self.fs)).exp();
        // The reference falls slowly, so a threshold set by a loud passage
        // does not stay set through a quiet one.
        self.decay = (-1.0f32 / (2.0 * self.fs)).exp();
        self.smooth = (-1.0f32 / (0.030 * self.fs)).exp();
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
        self.env = 0.0;
        self.open_for = 0.0;
        self.gain = 1.0;
    }

    /// The gain the wet path should have, given the dry input.
    #[inline]
    pub fn next(&mut self, dry: f32) -> f32 {
        if self.depth <= 0.0 {
            return 1.0;
        }
        let a = dry.abs();
        self.env = a.max(self.env * self.env_k);
        self.peak = a.max(self.peak * self.decay);

        // A fortieth of the loudest the signal has been lately. Relative
        // rather than absolute, because the number somebody wants depends on
        // how loud they printed the track, and asking them for it is asking
        // them to redo it when they change the fader.
        let threshold = self.peak * 0.025;
        if self.env > threshold {
            self.open_for = self.hold;
        } else if self.open_for > 0.0 {
            self.open_for -= 1.0;
        }

        let want = if self.open_for > 0.0 {
            1.0
        } else {
            1.0 - self.depth
        };
        self.gain = want + (self.gain - want) * self.smooth;
        self.gain
    }
}
