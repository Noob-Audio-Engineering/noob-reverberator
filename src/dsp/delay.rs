//! Delay lines, read at fractional positions.
//!
//! Every line here is read with a **third-order Lagrange** interpolator rather
//! than linearly, and the reason is the decay claim rather than taste.
//!
//! Linear interpolation between two samples is a one-zero low pass whose loss
//! depends on where between them you land. Inside a feedback loop that loss is
//! applied on **every trip**, so it adds to whatever the loss filter was
//! fitted to give --- and it moves as the modulation moves the read position.
//! A network built that way has a high-frequency decay that is shorter than
//! the curve says and that wobbles with the modulation. Since this plug-in's
//! whole claim is that the drawn decay is the realised decay, an interpolator
//! that quietly shortens the top of the band is not a detail.
//!
//! Third-order Lagrange is not free of that either --- nothing causal is ---
//! but its loss at the top of the audible band is far smaller, and what
//! remains is inside the loop when the fit measures it, because the fit reads
//! the filters and the interpolator sits outside them. That last part is a
//! known gap, and `docs/BENCHMARK.md` measures the tail rather than the
//! filters so that it cannot hide there.

/// A delay line whose length may be changed while it runs.
#[derive(Debug, Clone)]
pub struct Delay {
    buf: Vec<f32>,
    mask: usize,
    write: usize,
}

impl Delay {
    /// A line able to hold at least `max` samples.
    ///
    /// Rounded up to a power of two so the wrap is a mask rather than a
    /// branch: the read happens four times per sample per line, and with
    /// sixteen lines that is sixty-four wraps a sample.
    pub fn with_capacity(max: usize) -> Self {
        let n = max.next_power_of_two().max(8);
        Delay {
            buf: vec![0.0; n],
            mask: n - 1,
            write: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.write = 0;
    }

    #[inline]
    pub fn push(&mut self, x: f32) {
        self.write = (self.write + 1) & self.mask;
        self.buf[self.write] = x;
    }

    /// Read `d` samples back, `d` fractional.
    ///
    /// Clamped to leave room for the interpolator's four points at both ends;
    /// a read that walked off the end would be a click at exactly the moment
    /// a modulation reached its extreme, which is the hardest kind of fault to
    /// catch by listening.
    #[inline]
    pub fn read(&self, d: f32) -> f32 {
        let d = d.clamp(1.0, (self.buf.len() - 3) as f32);
        let i = d as usize;
        let frac = d - i as f32;
        let base = self.write.wrapping_sub(i);
        // In order of increasing delay: one sample nearer than `i`, then `i`,
        // then the two beyond it. The read lands between `s1` and `s2`.
        let s0 = self.buf[base.wrapping_add(1) & self.mask];
        let s1 = self.buf[base & self.mask];
        let s2 = self.buf[base.wrapping_sub(1) & self.mask];
        let s3 = self.buf[base.wrapping_sub(2) & self.mask];
        lagrange3(s0, s1, s2, s3, frac)
    }
}

/// Third-order Lagrange between `s1` and `s2`, with `s0` before and `s3`
/// after, at `f` of the way from `s1` to `s2`.
///
/// The four points sit at `−1, 0, 1, 2`, and the basis is written for those
/// positions. Writing it for `0, 1, 2, 3` and passing `f` in `0..1` evaluates
/// it next to `s0` instead of between `s1` and `s2` --- a whole sample early,
/// which sounds like nothing at all and detunes every air column in the
/// network. That is what the ramp test catches.
#[inline]
fn lagrange3(s0: f32, s1: f32, s2: f32, s3: f32, f: f32) -> f32 {
    let fm1 = f - 1.0;
    let fm2 = f - 2.0;
    let fp1 = f + 1.0;
    let c0 = -f * fm1 * fm2 / 6.0;
    let c1 = fp1 * fm1 * fm2 * 0.5;
    let c2 = -fp1 * f * fm2 * 0.5;
    let c3 = fp1 * f * fm1 / 6.0;
    s0 * c0 + s1 * c1 + s2 * c2 + s3 * c3
}

/// A Schroeder allpass, the building block every diffuser here is made of.
///
/// It passes every frequency at the same level and scrambles their phases,
/// which is what turns a handful of echoes into something without a pitch. The
/// delay may be modulated, which is what stops a chain of them from ringing on
/// one note.
#[derive(Debug, Clone)]
pub struct Allpass {
    line: Delay,
    len: f32,
    gain: f32,
}

impl Allpass {
    pub fn new(max: usize) -> Self {
        Allpass {
            line: Delay::with_capacity(max),
            len: (max / 2) as f32,
            gain: 0.5,
        }
    }

    pub fn set(&mut self, len: f32, gain: f32) {
        self.len = len.clamp(1.0, (self.line.capacity() - 3) as f32);
        // Held below one: at one it is a resonator that never lets go, and a
        // chain of those inside a reverb tank is an oscillator.
        self.gain = gain.clamp(-0.95, 0.95);
    }

    pub fn clear(&mut self) {
        self.line.clear();
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.process_at(x, self.len)
    }

    /// Run it with the delay somewhere other than its set length, for chains
    /// whose lengths are being modulated.
    #[inline]
    pub fn process_at(&mut self, x: f32, len: f32) -> f32 {
        let d = self.line.read(len);
        let v = x - self.gain * d;
        self.line.push(v);
        self.gain * v + d
    }

    pub fn len(&self) -> f32 {
        self.len
    }

    /// This stage's gain, for a caller working out how long a lap really is.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// How long a delay this stage can hold.
    pub fn capacity(&self) -> usize {
        self.line.capacity()
    }
}
