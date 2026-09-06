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
    /// The allpass coefficient every section is set to, kept so [`Spring::trip`]
    /// can work out how long a trip actually is.
    a: f32,
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
            a: -0.6,
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

    /// The frequency the trip length is measured at.
    ///
    /// A cascade of allpasses has no single delay --- that is the point of it
    /// --- so a fit against "one trip" has to say which trip. 1 kHz is the
    /// geometric middle of the band the benchmark reports over (125 Hz to
    /// 8 kHz), so the error is spread rather than piled on one end.
    const TRIP_AT: f32 = 1_000.0;

    /// One trip, in samples, which is what a fit for this coil is against.
    ///
    /// # Why this is not one sample per section
    ///
    /// It was, and a first-order allpass does average exactly one sample of
    /// group delay over the whole band, so that looked right. But it is an
    /// average over frequency, and the fit does not care about the average
    /// --- it cares about the trip at the frequencies somebody listens at.
    /// The group delay is
    ///
    /// ```text
    /// tau(w) = (1 - a^2) / (1 + 2 a cos w + a^2)
    /// ```
    ///
    /// which for a slack coil is four samples per section near DC and a
    /// quarter of one near Nyquist. **A fixed frequency is a different point
    /// on that curve at every sample rate**: at 44.1 kHz, 1 kHz sits well up
    /// the band, and at 192 kHz the same 1 kHz is four times closer to DC,
    /// where each section delays it four times as much. The trip was
    /// therefore under-estimated by more and more as the rate rose, the fit
    /// solved for more trips than actually happen, and the coil decayed
    /// short: measured, 2.98 s at 44.1 kHz falling to 2.63 s at 192 kHz for
    /// the same 2.50 s asked for --- a fifth of an octave of drift that
    /// nothing at one sample rate could see.
    pub fn trip(&self) -> f32 {
        let a = self.a;
        let w = std::f32::consts::TAU * Self::TRIP_AT / self.fs;
        let tau = (1.0 - a * a) / (1.0 + 2.0 * a * w.cos() + a * a);
        self.len + self.used as f32 * tau
    }

    /// 0 leaves the cascade linear; 1 is as hard as its amplifier is driven.
    pub fn set_drive(&mut self, amount: f32) {
        self.drive = amount.clamp(0.0, 1.0);
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
    pub fn set_shape(&mut self, tension: f32, sections: usize, delay_ms: f32) {
        let a = -tension.clamp(0.05, 0.95);
        self.a = a;
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
