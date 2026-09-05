//! Era: what the converters and the arithmetic of a decade did to a reverb,
//! as an axis separate from the architecture.
//!
//! This is the idea I took from Valhalla's COLOR control, and it is a good one
//! because it factors. Their 1970s setting "replicates the reduced bandwidth
//! of the earliest digital reverberators (10 kHz maximum output frequency)"
//! and is "downsampled internally to reproduce the artifacts of running at a
//! lower sampling rate"; the 1980s setting keeps the noisy modulation at full
//! bandwidth; NOW is "clean and colorless". None of that is a change of
//! topology --- so a hall and a plate can both be run through it, and it is
//! one control rather than a duplicate of every algorithm.
//!
//! What is here is the same three ideas, built rather than copied: a bandwidth
//! limit, a word length, and whether either is applied at all. The numbers are
//! mine and are not measurements of anybody's hardware --- I have measured no
//! reverb but this one.

use super::svf::{Kind, Svf};

/// The eras. **Append only**: a saved state stores the index.
pub const ERA_NAMES: [&str; 4] = ["Now", "Nineteen Seventies", "Nineteen Eighties", "Broken"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Era {
    /// Full bandwidth, full word length. Nothing is done.
    Now,
    /// Narrow, coarse, and sampled down inside the tank.
    Seventies,
    /// Full bandwidth, still coarse.
    Eighties,
    /// Further than any hardware went, because a plug-in can.
    Broken,
}

impl Era {
    pub fn from_index(i: usize) -> Era {
        match i {
            1 => Era::Seventies,
            2 => Era::Eighties,
            3 => Era::Broken,
            _ => Era::Now,
        }
    }

    /// The bandwidth this era holds, in hertz, and how many bits it keeps.
    fn spec(self) -> (f32, f32) {
        match self {
            Era::Now => (f32::INFINITY, 0.0),
            Era::Seventies => (10_000.0, 12.0),
            Era::Eighties => (16_000.0, 14.0),
            Era::Broken => (6_000.0, 8.0),
        }
    }
}

/// Applied to what goes into the tank, and to what comes back round in it.
pub struct Colour {
    era: Era,
    band: [Svf; 2],
    /// The quantiser's step, precomputed.
    step: f32,
    inv_step: f32,
    /// Where the sample-rate reduction is in its cycle, and what it is holding.
    hold: [f32; 2],
    phase: f32,
    inc: f32,
    amount: f32,
}

impl Colour {
    pub fn new(fs: f32) -> Self {
        let mut c = Colour {
            era: Era::Now,
            band: [Svf::default(); 2],
            step: 0.0,
            inv_step: 0.0,
            hold: [0.0; 2],
            phase: 0.0,
            inc: 1.0,
            amount: 1.0,
        };
        c.set(Era::Now, 1.0, fs);
        c
    }

    /// `amount` fades the whole effect in, so an era is a place on a dial
    /// rather than a switch.
    pub fn set(&mut self, era: Era, amount: f32, fs: f32) {
        self.era = era;
        self.amount = amount.clamp(0.0, 1.0);
        let (bw, bits) = era.spec();
        let corner = if bw.is_finite() {
            bw.min(fs * 0.45)
        } else {
            fs * 0.45
        };
        for b in &mut self.band {
            // Two in series: one pole of roll-off is not a bandwidth, it is a
            // tilt, and the thing being reproduced is a converter's edge.
            b.set(Kind::HighShelf, fs, corner, 0.7, -36.0 * self.amount);
        }
        if bits > 0.0 {
            let levels = 2f32.powf(bits);
            self.step = 2.0 / levels;
            self.inv_step = levels / 2.0;
        } else {
            self.step = 0.0;
            self.inv_step = 0.0;
        }
        // Running the tank at a lower rate is what gives the seventies their
        // aliasing. Held rather than filtered, deliberately: the artefacts are
        // the point, and filtering them out would leave only the dullness.
        self.inc = match era {
            Era::Seventies => (20_000.0 / fs).min(1.0),
            Era::Broken => (12_000.0 / fs).min(1.0),
            _ => 1.0,
        };
    }

    pub fn reset(&mut self) {
        self.hold = [0.0; 2];
        self.phase = 0.0;
        for b in &mut self.band {
            b.reset();
        }
    }

    /// Colour one stereo pair.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.era == Era::Now || self.amount <= 0.0 {
            return (l, r);
        }
        let mut v = [self.band[0].process(l), self.band[1].process(r)];

        if self.inc < 1.0 {
            self.phase += self.inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                self.hold = v;
            }
            v = self.hold;
        }

        if self.step > 0.0 {
            for x in v.iter_mut() {
                let q = (*x * self.inv_step).round() * self.step;
                *x += (q - *x) * self.amount;
            }
        }
        (v[0], v[1])
    }
}
