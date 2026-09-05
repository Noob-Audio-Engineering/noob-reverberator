//! Diffusion: turning a few echoes into something with no pitch.
//!
//! A chain of allpasses in series. Each one passes every frequency at the same
//! level and scrambles the phases, so a single impulse becomes a burst whose
//! density multiplies at every stage --- which is what "echo density" means in
//! every mode description in the survey, and what separates an ambience from a
//! plate at the same decay time.
//!
//! # Why the lengths are what they are
//!
//! Each stage is roughly a third of the one before it. Equal stages put their
//! repeats on top of each other and the chain rings; a steep taper wastes the
//! later stages, which fire before the earlier ones have said anything. A
//! third is the usual compromise and it is what the ratio here is.
//!
//! The **amount** of diffusion is the allpass gain, not the number of stages.
//! At zero the chain is a bare delay and the early sound is a handful of
//! discrete repeats --- which is what a sparse room does, and what Valhalla's
//! notes mean by "the decay is a bit sparse, but it quickly builds". At its
//! top the chain is dense from the first millisecond, which is a plate.

use super::delay::Allpass;
use super::modulate::Modulator;

/// The most stages one chain can have.
pub const MAX_STAGES: usize = 8;

pub struct Diffuser {
    stages: Vec<Allpass>,
    lens: Vec<f32>,
    mods: Vec<Modulator>,
    depth: f32,
    random: f32,
}

impl Diffuser {
    /// A chain of `n` stages whose longest is `longest` samples.
    pub fn new(n: usize, longest: usize, seed: u32) -> Self {
        let n = n.clamp(1, MAX_STAGES);
        let mut stages = Vec::with_capacity(n);
        let mut lens = Vec::with_capacity(n);
        let mut mods = Vec::with_capacity(n);
        let mut len = longest as f32;
        for i in 0..n {
            // Room for the modulation to swing without the read clamping,
            // which would flatten the top of every sweep.
            stages.push(Allpass::new((len as usize + 64).next_power_of_two()));
            lens.push(len);
            mods.push(Modulator::new(
                seed.wrapping_mul(2_654_435_761).wrapping_add(i as u32),
            ));
            len *= 0.37;
            len = len.max(8.0);
        }
        Diffuser {
            stages,
            lens,
            mods,
            depth: 0.0,
            random: 0.0,
        }
    }

    pub fn clear(&mut self) {
        for s in &mut self.stages {
            s.clear();
        }
    }

    /// `amount` from 0 (bare delay) to 1 (as dense as the chain goes).
    pub fn set_amount(&mut self, amount: f32) {
        // Held short of one: at one an allpass is a resonator that never lets
        // go, and eight of those in series inside a tank is an oscillator.
        let g = amount.clamp(0.0, 1.0) * 0.78;
        for (s, &len) in self.stages.iter_mut().zip(self.lens.iter()) {
            s.set(len, g);
        }
    }

    /// Scale every stage, for a size control.
    pub fn set_scale(&mut self, scale: f32, longest: f32) {
        let mut len = longest.max(8.0) * scale.clamp(0.05, 4.0);
        for i in 0..self.stages.len() {
            let cap = self.stages[i].capacity() as f32 - 8.0;
            self.lens[i] = len.clamp(8.0, cap);
            len *= 0.37;
        }
    }

    pub fn set_modulation(&mut self, fs: f32, rate: f32, depth: f32, random: f32) {
        for m in &mut self.mods {
            m.set_rate(fs, rate);
        }
        self.depth = depth;
        self.random = random.clamp(0.0, 1.0);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = x;
        for i in 0..self.stages.len() {
            let len = if self.depth > 0.0 {
                // The shorter stages are swept less, in proportion, so a depth
                // that is gentle on the longest stage does not swing the
                // shortest one through its whole length.
                let scale = self.lens[i] / self.lens[0].max(1.0);
                self.lens[i] + self.mods[i].next(self.depth * scale, self.random)
            } else {
                self.lens[i]
            };
            y = self.stages[i].process_at(y, len);
        }
        y
    }
}
