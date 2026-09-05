//! The feedback delay network: the tank most of these modes are built on.
//!
//! `N` delay lines are read, mixed through an orthogonal matrix, attenuated by
//! the loss filter each line was fitted with, and written back. The matrix
//! being orthogonal is what makes the decay belong to the loss filters and to
//! nothing else --- an energy-preserving mix means the only thing removing
//! energy is the thing that was fitted to remove it, which is the whole reason
//! the drawn curve can be believed.
//!
//! # Why the lengths are what they are
//!
//! Two lines whose lengths share a factor put their echoes on top of each
//! other, and the network's early sound gets a pitch. Mutually prime lengths
//! spread the coincidences out as far as they go. The lengths here are primes
//! scaled by the size control and then re-primed, so a size sweep does not
//! walk through arrangements that momentarily rhyme.

use super::decay::Curve;
use super::delay::Delay;
use super::loss::Loss;

/// The most lines a tank can have. Sixteen: past that the echo density gained
/// per line falls off while the cost does not, and the modes that want more
/// density get it from the diffusers instead.
pub const MAX_LINES: usize = 16;

pub struct Fdn {
    lines: Vec<Delay>,
    loss: Vec<Loss>,
    /// The nominal length of each line, in samples.
    len: Vec<f32>,
    /// Where each line is being read this instant, once modulation is applied.
    read_at: Vec<f32>,
    /// Scratch, so the mix does not allocate.
    buf: Vec<f32>,
    n: usize,
    fs: f32,
}

impl Fdn {
    /// A network of `n` lines, each able to hold `max_len` samples.
    pub fn new(n: usize, max_len: usize, fs: f32) -> Self {
        let n = n.clamp(4, MAX_LINES);
        Fdn {
            lines: (0..n).map(|_| Delay::with_capacity(max_len)).collect(),
            loss: vec![Loss::default(); n],
            len: vec![max_len as f32 * 0.5; n],
            read_at: vec![max_len as f32 * 0.5; n],
            buf: vec![0.0; n],
            n,
            fs,
        }
    }

    pub fn lines(&self) -> usize {
        self.n
    }

    pub fn clear(&mut self) {
        for l in &mut self.lines {
            l.clear();
        }
        for l in &mut self.loss {
            l.reset();
        }
    }

    /// Set the line lengths, in samples, and refit every loss filter to the
    /// curve.
    ///
    /// The fit is per line and it has to be: a short line goes round more
    /// often and must lose less each time to reach the same decay. This is the
    /// expensive call in the whole engine and it is not called per sample ---
    /// it runs when a control moves.
    pub fn set_lengths(&mut self, lengths: &[f32], curve: &Curve) {
        for i in 0..self.n {
            let cap = self.lines[i].capacity() as f32 - 4.0;
            self.len[i] = lengths[i.min(lengths.len() - 1)].clamp(4.0, cap);
            self.read_at[i] = self.len[i];
            self.loss[i].fit(self.fs, self.len[i] as usize, curve);
        }
    }

    /// Refit the loss filters without moving the lines.
    pub fn refit(&mut self, curve: &Curve) {
        for i in 0..self.n {
            self.loss[i].fit(self.fs, self.len[i] as usize, curve);
        }
    }

    /// The nominal length of a line, in samples.
    pub fn len_of(&self, i: usize) -> f32 {
        self.len[i]
    }

    /// Offset one line's read position, for modulation.
    pub fn modulate(&mut self, i: usize, offset: f32) {
        let cap = self.lines[i].capacity() as f32 - 4.0;
        self.read_at[i] = (self.len[i] + offset).clamp(2.0, cap);
    }

    /// The decay this network will actually produce at `f`, in seconds.
    ///
    /// Averaged over the lines, weighted by nothing: every line contributes
    /// its own decay, and with an orthogonal mix the tail is their sum, so the
    /// slowest line is what is left at the end. The mean is the honest summary
    /// of a network whose lines are within a factor of two of each other,
    /// which is what the length rules keep them to --- and the benchmark
    /// measures the actual tail rather than trusting this.
    pub fn t60_at(&self, f: f32) -> f32 {
        let mut s = 0.0;
        for i in 0..self.n {
            s += self.loss[i].t60(self.fs, self.len[i] as usize, f);
        }
        s / self.n as f32
    }

    /// One sample. `input` is what is being injected into each line; the
    /// values read back out are left in `out`.
    #[inline]
    pub fn process(&mut self, input: &[f32], out: &mut [f32]) {
        for i in 0..self.n {
            self.buf[i] = self.lines[i].read(self.read_at[i]);
        }
        out[..self.n].copy_from_slice(&self.buf[..self.n]);

        householder(&mut self.buf[..self.n]);

        for (i, (&x, inject)) in self.buf[..self.n].iter().zip(input).enumerate() {
            let v = self.loss[i].process(x) + inject;
            self.lines[i].push(v);
        }
    }
}

/// The Householder reflection `I − (2/N)·11ᵀ`, in place.
///
/// Orthogonal, so it moves energy between lines without creating or destroying
/// any, and it costs one sum and one multiply-add per line rather than the
/// `N²` a general matrix would. Every line reaches every other in one hop,
/// which is what builds echo density quickly.
///
/// It is its own inverse, which is worth knowing when reading the tank: mixing
/// twice is not mixing more.
#[inline]
fn householder(v: &mut [f32]) {
    let n = v.len();
    if n == 0 {
        return;
    }
    let mut sum = 0.0;
    for x in v.iter() {
        sum += *x;
    }
    let k = 2.0 * sum / n as f32;
    for x in v.iter_mut() {
        *x -= k;
    }
}

/// Line lengths for a tank: `n` mutually prime numbers spread over a range.
///
/// The spread is geometric rather than linear, so the ratio between
/// neighbouring lines is constant and no two are close at any size. `shortest`
/// and `longest` are in samples.
pub fn prime_lengths(n: usize, shortest: f32, longest: f32, out: &mut Vec<f32>) {
    out.clear();
    let ratio = (longest / shortest.max(1.0)).max(1.0);
    for i in 0..n {
        let t = if n > 1 {
            i as f32 / (n - 1) as f32
        } else {
            0.0
        };
        let want = shortest * ratio.powf(t);
        out.push(next_prime(want.max(4.0) as u32) as f32);
    }
}

/// The first prime at or above `n`.
///
/// Trial division, and it is fine: this runs when a size control moves, over
/// numbers of a few thousand, sixteen times.
fn next_prime(n: u32) -> u32 {
    let mut c = n.max(2);
    'outer: loop {
        if c == 2 {
            return 2;
        }
        if c.is_multiple_of(2) {
            c += 1;
            continue;
        }
        let mut d = 3;
        while d * d <= c {
            if c.is_multiple_of(d) {
                c += 2;
                continue 'outer;
            }
            d += 2;
        }
        return c;
    }
}
