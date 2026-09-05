//! Chorale: vowels on the tail, so the reverb sings rather than rings.
//!
//! Strymon's Chorale machine is described as a vocal choir accompaniment with
//! customisable vowel ranges and intensities. What makes something sound like
//! a voice rather than a filter sweep is **formants**: a vowel is two or three
//! resonances at fixed absolute frequencies, and the reason a sung note stays
//! recognisably "ah" across an octave is that those resonances do *not* move
//! with the pitch.
//!
//! That is the whole trick, and it is why this is a bank of fixed peaking
//! filters rather than anything tracking the input. The tail is already a
//! dense signal with energy everywhere; putting a vowel's resonances on it
//! makes the ear hear a voice holding that vowel.
//!
//! # Where the numbers come from
//!
//! The first two formants of each vowel, in the region every published table
//! agrees about --- these are the values a phonetics course gives for an adult
//! voice, and they are approximate on purpose. A vowel is a region rather than
//! a point: it varies by speaker, by language and by how hard the note is
//! sung, and a table claiming four significant figures would be claiming a
//! precision the thing itself does not have.

use super::svf::{Kind, Svf};

/// The vowels on offer. **Append only**: a saved state stores the index.
pub const VOWEL_NAMES: [&str; 6] = ["Ah", "Eh", "Ee", "Oh", "Oo", "Mm"];

/// First, second and third formants, in hertz.
const FORMANTS: [[f32; 3]; 6] = [
    [730.0, 1090.0, 2440.0], // Ah, as in "father"
    [530.0, 1840.0, 2480.0], // Eh, as in "bed"
    [270.0, 2290.0, 3010.0], // Ee, as in "beet"
    [570.0, 840.0, 2410.0],  // Oh, as in "bought"
    [300.0, 870.0, 2240.0],  // Oo, as in "boot"
    [280.0, 1000.0, 2200.0], // Mm, a closed hum
];

/// How loud each formant is relative to the first. A voice's upper formants
/// are quieter, and making them equal gives something that sounds like three
/// filters rather than one vowel.
const LEVELS: [f32; 3] = [1.0, 0.6, 0.32];

pub struct Choir {
    /// Three formants, per side.
    bank: [[Svf; 3]; 2],
    /// The vowel being crossfaded from and to, and how far through.
    from: usize,
    to: usize,
    blend: f32,
    amount: f32,
    fs: f32,
}

impl Choir {
    pub fn new(fs: f32) -> Self {
        Choir {
            bank: [[Svf::default(); 3]; 2],
            from: 0,
            to: 0,
            blend: 1.0,
            amount: 0.0,
            fs,
        }
    }

    pub fn clear(&mut self) {
        for side in &mut self.bank {
            for f in side.iter_mut() {
                f.reset();
            }
        }
    }

    /// `vowel` chooses which, `spread` moves the formants together or apart
    /// (a smaller throat against a larger one), and `amount` is how much of
    /// the vowel is applied at all.
    ///
    /// `vowel` is a **continuous** index, so sweeping it walks from one vowel
    /// to the next rather than switching. A choir that changes vowel on a
    /// sample boundary is a choir that clicks.
    pub fn set(&mut self, vowel: f32, spread: f32, amount: f32, resonance: f32) {
        let n = VOWEL_NAMES.len();
        let v = vowel.clamp(0.0, (n - 1) as f32);
        self.from = v.floor() as usize;
        self.to = (self.from + 1).min(n - 1);
        self.blend = v - self.from as f32;
        self.amount = amount.clamp(0.0, 1.0);

        let k = 2f32.powf(spread.clamp(-1.0, 1.0));
        // A formant's sharpness is what makes it a vowel rather than a tilt.
        let q = 3.0 + resonance.clamp(0.0, 1.0) * 12.0;
        let gain = 6.0 + resonance.clamp(0.0, 1.0) * 14.0;
        for i in 0..3 {
            let f = (FORMANTS[self.from][i] * (1.0 - self.blend)
                + FORMANTS[self.to][i] * self.blend)
                * k;
            let g = gain * LEVELS[i] * self.amount;
            for side in &mut self.bank {
                side[i].set(Kind::Bell, self.fs, f.clamp(60.0, self.fs * 0.45), q, g);
            }
        }
    }

    /// Colour one stereo pair.
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.amount <= 0.0 {
            return (l, r);
        }
        let mut a = l;
        let mut b = r;
        for i in 0..3 {
            a = self.bank[0][i].process(a);
            b = self.bank[1][i].process(b);
        }
        // The formants add a lot of gain where they sit, so the level is
        // pulled back by roughly what they put in. Without it, turning the
        // choir up is indistinguishable from turning the reverb up, and the
        // control stops being about vowels.
        let trim = 1.0 / (1.0 + self.amount * 1.6);
        (a * trim, b * trim)
    }
}
