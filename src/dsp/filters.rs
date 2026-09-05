//! The biquads the rest of the engine is built from.
//!
//! Direct form I with the coefficients normalised by `a0` at design time, so
//! the sample loop is four multiply-adds and no division. The design formulae
//! are the Audio EQ Cookbook ones (Robert Bristow-Johnson); they are written
//! out rather than reached for by name because the loss-filter fit depends on
//! the peaking filter's gain being *exactly* `gain_db` at its centre, and that
//! is a property of these particular formulae.

use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// A filter that passes everything through unchanged.
    pub fn unity() -> Self {
        Biquad {
            b0: 1.0,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Set the coefficients but **keep the state**, so a control can move
    /// while the tail is ringing without a click.
    fn set(&mut self, b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) {
        let inv = 1.0 / a0;
        self.b0 = b0 * inv;
        self.b1 = b1 * inv;
        self.b2 = b2 * inv;
        self.a1 = a1 * inv;
        self.a2 = a2 * inv;
    }

    /// A peaking filter of `gain_db` at `f0`, `q` wide.
    pub fn peaking(&mut self, fs: f32, f0: f32, q: f32, gain_db: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * (f0 / fs).clamp(1e-6, 0.49);
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * q.max(0.05));
        self.set(
            1.0 + alpha * a,
            -2.0 * cs,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cs,
            1.0 - alpha / a,
        );
    }

    /// A low shelf of `gain_db` below `f0`.
    pub fn low_shelf(&mut self, fs: f32, f0: f32, q: f32, gain_db: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * (f0 / fs).clamp(1e-6, 0.49);
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * q.max(0.05));
        let sqa = 2.0 * a.sqrt() * alpha;
        self.set(
            a * ((a + 1.0) - (a - 1.0) * cs + sqa),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
            a * ((a + 1.0) - (a - 1.0) * cs - sqa),
            (a + 1.0) + (a - 1.0) * cs + sqa,
            -2.0 * ((a - 1.0) + (a + 1.0) * cs),
            (a + 1.0) + (a - 1.0) * cs - sqa,
        );
    }

    /// A high shelf of `gain_db` above `f0`.
    pub fn high_shelf(&mut self, fs: f32, f0: f32, q: f32, gain_db: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * (f0 / fs).clamp(1e-6, 0.49);
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * q.max(0.05));
        let sqa = 2.0 * a.sqrt() * alpha;
        self.set(
            a * ((a + 1.0) + (a - 1.0) * cs + sqa),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
            a * ((a + 1.0) + (a - 1.0) * cs - sqa),
            (a + 1.0) - (a - 1.0) * cs + sqa,
            2.0 * ((a - 1.0) - (a + 1.0) * cs),
            (a + 1.0) - (a - 1.0) * cs - sqa,
        );
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    /// The magnitude response at `f`, for the fit's own measurement of itself.
    ///
    /// **In double precision, and that is not fussiness.** The denominator is
    /// `1 + a1·cos ω + a2·cos 2ω`, and for a gentle filter `a1 ≈ −2`, `a2 ≈ 1`,
    /// so at a low frequency it is a difference of numbers near 2 that lands
    /// near 1e-5. In `f32` that cancellation leaves about a per cent of the
    /// answer, which is invisible in a response plot and ruinous here: the
    /// decay time is `60 / |loss|`, so a per cent of a hundredth of a decibel
    /// is tens of per cent of a second. I measured this as a 47% error in the
    /// decay at 30 Hz at 96 kHz and spent a while improving a fit that was
    /// already right, because the ruler was wrong.
    ///
    /// The coefficients stay `f32` --- their own rounding is real and belongs
    /// in the answer. It is only the evaluation that is widened.
    pub fn magnitude(&self, fs: f32, f: f32) -> f32 {
        let w = 2.0 * std::f64::consts::PI * f as f64 / fs as f64;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let (b0, b1, b2) = (self.b0 as f64, self.b1 as f64, self.b2 as f64);
        let (a1, a2) = (self.a1 as f64, self.a2 as f64);
        let nr = b0 + b1 * c1 + b2 * c2;
        let ni = -(b1 * s1 + b2 * s2);
        let dr = 1.0 + a1 * c1 + a2 * c2;
        let di = -(a1 * s1 + a2 * s2);
        (((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-300)).sqrt()) as f32
    }
}

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
