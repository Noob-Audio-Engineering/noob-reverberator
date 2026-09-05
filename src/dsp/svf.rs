//! The shelving and peaking filters the decay curve is realised with.
//!
//! A topology-preserving state variable filter rather than a direct-form
//! biquad, and the reason is measured rather than stylistic.
//!
//! # Why not a biquad
//!
//! The loss a delay line needs is `60·(m/fs)/T60` decibels per trip. On a
//! short line at a high sample rate that is a *hundredth of a decibel*, and a
//! direct-form biquad cannot hold it: its coefficients are `a1 ≈ −2`,
//! `a2 ≈ 1`, and the response at a low frequency is the difference between
//! them, so rounding the coefficients to `f32` moves the answer by more than
//! the answer is.
//!
//! Measured, on a 250 Hz low shelf: at 48 kHz a direct-form biquad's magnitude
//! at 20 Hz is within 0.002 dB of its analogue prototype, which is fine. At
//! 96 kHz it is out by **0.011 dB** --- and the loss being asked for there was
//! 0.02 dB, so half the quantity was coefficient rounding. That is what turned
//! a 12-second decay into a 17-second one at the bottom of the band.
//!
//! The state variable form has no such difference of near-equal coefficients:
//! its frequency coefficient is `tan(π f₀/fs)`, which at a low frequency is
//! simply a small number, and the response at a low frequency is a ratio of
//! small numbers rather than a cancellation of large ones.
//!
//! The forms below are Andrew Simper's, and the magnitude is the analogue
//! prototype evaluated at the warped frequency --- which is not an
//! approximation of what this filter does, it is exactly what it does, because
//! the transform preserves it.

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Bell,
    LowShelf,
    HighShelf,
}

#[derive(Debug, Clone, Copy)]
pub struct Svf {
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    m0: f32,
    m1: f32,
    m2: f32,
    ic1: f32,
    ic2: f32,
}

impl Default for Svf {
    fn default() -> Self {
        let mut s = Svf {
            g: 0.0,
            k: 0.0,
            a1: 1.0,
            a2: 0.0,
            a3: 0.0,
            m0: 1.0,
            m1: 0.0,
            m2: 0.0,
            ic1: 0.0,
            ic2: 0.0,
        };
        s.unity();
        s
    }
}

impl Svf {
    /// Pass everything through untouched.
    pub fn unity(&mut self) {
        self.g = 0.0;
        self.k = 0.0;
        self.a1 = 1.0;
        self.a2 = 0.0;
        self.a3 = 0.0;
        self.m0 = 1.0;
        self.m1 = 0.0;
        self.m2 = 0.0;
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    /// Design one filter. The state is left alone, so a control may move while
    /// the tail is still ringing.
    pub fn set(&mut self, kind: Kind, fs: f32, f0: f32, q: f32, gain_db: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = PI * (f0 / fs).clamp(1e-7, 0.49);
        let t = w.tan();
        let q = q.max(0.05);
        match kind {
            Kind::Bell => {
                self.g = t;
                self.k = 1.0 / (q * a);
                self.m0 = 1.0;
                self.m1 = self.k * (a * a - 1.0);
                self.m2 = 0.0;
            }
            Kind::LowShelf => {
                self.g = t / a.sqrt();
                self.k = 1.0 / q;
                self.m0 = 1.0;
                self.m1 = self.k * (a - 1.0);
                self.m2 = a * a - 1.0;
            }
            Kind::HighShelf => {
                self.g = t * a.sqrt();
                self.k = 1.0 / q;
                self.m0 = a * a;
                self.m1 = self.k * (1.0 - a) * a;
                self.m2 = 1.0 - a * a;
            }
        }
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    #[inline]
    pub fn process(&mut self, v0: f32) -> f32 {
        let v3 = v0 - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        self.m0 * v0 + self.m1 * v1 + self.m2 * v2
    }

    /// This filter's magnitude at `f`.
    ///
    /// Exact rather than approximate: the transform maps the digital response
    /// onto the analogue prototype at the warped frequency
    /// `x = tan(πf/fs) / g`, so evaluating the prototype there *is* evaluating
    /// the filter. In double precision, because the result is divided into
    /// sixty to get a decay time.
    pub fn magnitude(&self, fs: f32, f: f32) -> f32 {
        if self.g == 0.0 {
            return 1.0;
        }
        let x = (std::f64::consts::PI * f as f64 / fs as f64)
            .clamp(1e-12, std::f64::consts::FRAC_PI_2 - 1e-9)
            .tan()
            / self.g as f64;
        let (m0, m1, m2) = (self.m0 as f64, self.m1 as f64, self.m2 as f64);
        let k = self.k as f64;
        let one_minus = 1.0 - x * x;
        let nr = m0 * one_minus + m2;
        let ni = (m0 * k + m1) * x;
        let dr = one_minus;
        let di = k * x;
        (((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-300)).sqrt()) as f32
    }
}
