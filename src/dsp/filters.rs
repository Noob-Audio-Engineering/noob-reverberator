//! The one-pole this engine smooths and damps with.
//!
//! There was a `Biquad` here, and it is gone on purpose. The loss filters were
//! direct-form biquads until they were measured: their coefficients are the
//! difference of numbers near two, and rounding that to `f32` moved a
//! hundredth-of-a-decibel loss at 20 Hz by more than the loss itself. They are
//! [`Svf`](super::svf::Svf)s now.
//!
//! Keeping the biquad around "in case" would have left a second filter
//! implementation in the tree with nothing using it and nothing testing it,
//! for the next person to reach for without knowing why it was abandoned.

use std::f32::consts::PI;

/// A one-pole low pass, for damping and for smoothing controls.
#[derive(Debug, Clone, Copy, Default)]
pub struct OnePole {
    a: f32,
    z: f32,
}

impl OnePole {
    pub fn set_cutoff(&mut self, fs: f32, f: f32) {
        let w = 2.0 * PI * (f.clamp(1.0, fs * 0.49)) / fs;
        self.a = (-w).exp();
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.z = x * (1.0 - self.a) + self.z * self.a;
        self.z
    }
    pub fn reset(&mut self) {
        self.z = 0.0;
    }
}
