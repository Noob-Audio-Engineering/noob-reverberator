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
use super::loss::Loss;

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

    /// `tension` sets how strong the dispersion is --- a tight spring chirps
    /// less and sounds brighter, a slack one sweeps further. `sections` is how
    /// long the sweep lasts. `delay_ms` is the length of one trip down the
    /// coil.
    pub fn set(&mut self, tension: f32, sections: usize, delay_ms: f32, curve: &Curve) {
        // Below zero the cascade delays the *top* of the band instead, which
        // is a chirp sweeping the wrong way and not a spring.
        let a = tension.clamp(0.05, 0.95);
        self.used = sections.clamp(1, MAX_SECTIONS);
        for s in &mut self.chain[..self.used] {
            s.a = a;
        }
        let cap = self.line.capacity() as f32 - 8.0;
        self.len = (delay_ms.max(1.0) * self.fs / 1000.0).clamp(8.0, cap);
        // The cascade's own delay is part of the trip: each first-order
        // section delays by about `(1+a)/(1−a)` samples at the bottom of the
        // band, and a hundred of them is not a rounding error. Leaving it out
        // makes the loop shorter than the fit thinks and the decay too long,
        // which is the same fault the plate had.
        let cascade = self.used as f32 * (1.0 + a) / (1.0 - a);
        self.loss.fit(self.fs, (self.len + cascade) as usize, curve);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut v = x + self.fb;
        for s in &mut self.chain[..self.used] {
            v = s.process(v);
        }
        self.line.push(v);
        let out = self.line.read(self.len);
        self.fb = self.loss.process(out);
        out
    }
}
