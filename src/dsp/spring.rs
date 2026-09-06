//! The spring: a reverb whose character is dispersion, not density.
//!
//! A spring reverb is a coil with a transducer at each end. What travels along
//! it is a torsional wave, and in a coil that wave is **dispersive**: high
//! frequencies arrive before low ones. Strike it and the delayed copy is not a
//! copy at all, it is a chirp sweeping upwards --- the "boing" everybody knows
//! from a guitar amplifier, and what Strymon's Spring machine and Valhalla's
//! mode notes are both reaching for.
//!
//! Density is not the point here. A spring has very few paths and they are
//! long, so its echoes stay countable; a plate has too many to count from the
//! first instant. Building a spring by diffusing harder gets a small plate.
//!
//! # How the chirp is made
//!
//! A long cascade of identical first-order allpasses. Each one delays the
//! bottom of the band more than the top, and a hundred of them in series turn
//! an impulse into a sweep. The cascade's coefficient sets how strong the
//! dispersion is, and the number of sections sets how long the sweep lasts.
//!
//! That is not a model of a coil --- a real one has a cutoff, several
//! propagating modes and a transducer at each end with its own resonance. It
//! is a model of the *thing the ear uses to recognise a spring*, which is the
//! chirp, and the survey's own framing supports it: Valhalla describe
//! designing "from a psychoacoustic perspective. Instead of creating a
//! simplified physical model of a simplified physical space" --- generating
//! the cues rather than the room.

use super::decay::Curve;
use super::delay::Delay;
use super::loss::{Fitted, Loss, Scratch};

/// The most allpass sections one spring may have.
pub const MAX_SECTIONS: usize = 160;

/// A first-order allpass, `(a + z⁻¹)/(1 + a·z⁻¹)`.
#[derive(Debug, Clone, Copy, Default)]
struct Ap1 {
    a: f32,
    x1: f32,
    y1: f32,
}

impl Ap1 {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.a * (x - self.y1) + self.x1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

pub struct Spring {
    chain: Vec<Ap1>,
    used: usize,
    line: Delay,
    len: f32,
    loss: Loss,
    scratch: Scratch,
    /// How hard the spring's amplifier is driven. See [`super::drive`].
    drive: f32,
    fs: f32,
    fb: f32,
}

impl Spring {
    pub fn new(fs: f32, max_ms: f32) -> Self {
        let cap = ((fs * max_ms / 1000.0) as usize + 64).next_power_of_two();
        Spring {
            chain: vec![Ap1::default(); MAX_SECTIONS],
            used: 100,
            line: Delay::with_capacity(cap),
            len: fs * 0.03,
            loss: Loss::default(),
            scratch: Scratch::default(),
            drive: 0.0,
            fs,
            fb: 0.0,
        }
    }

    pub fn clear(&mut self) {
        for a in &mut self.chain {
            *a = Ap1::default();
        }
        self.line.clear();
        self.loss.reset();
        self.fb = 0.0;
    }

    /// Apply a fit worked out elsewhere.
    pub fn apply(&mut self, curve: &Curve, f: &Fitted) {
        self.loss.apply(self.fs, curve, f);
    }

    /// The decay this coil is actually producing at `f`, in seconds. As
    /// approximate as the fit is --- the cascade's delay depends on frequency
    /// and this uses the single number the fit was given.
    pub fn realised_t60(&self, f: f32) -> f32 {
        self.loss.t60(self.fs, self.trip() as usize, f)
    }

    /// One trip, which is what a fit for this coil is against.
    pub fn trip(&self) -> f32 {
        self.len + self.used as f32
    }

    /// `tension` sets how strong the dispersion is --- a tight spring chirps
    /// less and sounds brighter, a slack one sweeps further. `sections` is how
    /// long the sweep lasts. `delay_ms` is the length of one trip down the
    /// coil.
    pub fn set(&mut self, tension: f32, sections: usize, delay_ms: f32, curve: &Curve) {
        self.set_shape(tension, sections, delay_ms);
        let trip = self.trip();
        let mut sc = std::mem::take(&mut self.scratch);
        self.loss.fit(self.fs, trip as usize, curve, &mut sc);
        self.scratch = sc;
    }

    /// The coil only, without fitting anything.
    ///
    /// # The coefficient is negative, and that is the whole thing
    ///
    /// A first-order allpass `(a + z⁻¹)/(1 + a·z⁻¹)` has a group delay of
    /// `(1−a)/(1+a)` at the bottom of the band and `(1+a)/(1−a)` at the top.
    /// With a **positive** `a` the top is delayed more, and a hundred of them
    /// in cascade delay the top of the audible band by about as much as the
    /// bottom --- the dispersion all happens up near Nyquist where nothing
    /// is.
    ///
    /// I had it positive. Measured, a slack coil put 4 kHz at 30.2 ms and
    /// 300 Hz at 30.3 ms: a tenth of a millisecond of chirp, which is no
    /// chirp. It decayed correctly the whole time, which is why nothing else
    /// caught it --- a spring that does not chirp is a small dark plate, and
    /// it measures like one.
    ///
    /// Negative, the bottom of the band is delayed more and the top comes
    /// back first, which is the descending "boing" a spring is recognised by.
    /// 0 leaves the cascade linear; 1 is as hard as it is driven.
    pub fn set_drive(&mut self, amount: f32) {
        self.drive = amount.clamp(0.0, 1.0);
    }

    pub fn set_shape(&mut self, tension: f32, sections: usize, delay_ms: f32) {
        let a = -tension.clamp(0.05, 0.95);
        self.used = sections.clamp(1, MAX_SECTIONS);
        for s in &mut self.chain[..self.used] {
            s.a = a;
        }
        let cap = self.line.capacity() as f32 - 8.0;
        self.len = (delay_ms.max(1.0) * self.fs / 1000.0).clamp(8.0, cap);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut v = x + self.fb;
        for s in &mut self.chain[..self.used] {
            v = s.process(v);
        }
        self.line.push(v);
        let out = self.line.read(self.len);
        self.fb = super::drive::saturate(
            self.loss.process(out),
            self.drive,
            super::drive::SPRING_LEVEL,
        );
        out
    }
}
