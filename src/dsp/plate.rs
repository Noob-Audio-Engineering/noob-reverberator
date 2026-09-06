//! The plate: a figure-of-eight tank rather than a network of lines.
//!
//! A plate reverb has no room in it. A steel sheet is a two-dimensional
//! resonator whose modes are dense from the first instant, and what that
//! sounds like is a reverb with **no build-up at all** --- full density
//! immediately, bright, and with the character the survey's mode notes keep
//! calling "metallic sheen".
//!
//! The topology is Jon Dattorro's: an input chain of allpasses to smear the
//! signal, then two halves of a loop, each a modulated allpass, a delay, a
//! loss and a second allpass, with each half feeding the other. Energy goes
//! round the figure of eight rather than round `N` separate lines, which is
//! why a plate sounds like one thing and a network like a sum of things.
//!
//! # What is different here
//!
//! The published topology has a damping filter per half and a fixed decay
//! gain. This one carries a [`Loss`](super::loss::Loss) instead, fitted to the
//! same decay curve everything else in this plug-in is fitted to, so a plate's
//! decay can be drawn against frequency exactly like a hall's. What the fit is
//! given as the trip is one lap of the figure of eight, because that is what
//! it is.

use super::decay::Curve;
use super::delay::{Allpass, Delay};
use super::loss::{Fitted, Loss, Scratch};
use super::modulate::Modulator;

/// The lengths, in samples at 29.761 kHz, from Dattorro's paper. They are
/// scaled to the running sample rate and to the size control, and they are
/// kept rather than rounded because their ratios are what make the tank
/// diffuse rather than resonant.
const IN_LENS: [f32; 4] = [142.0, 107.0, 379.0, 277.0];
const AP1_LENS: [f32; 2] = [672.0, 908.0];
const AP2_LENS: [f32; 2] = [1800.0, 2656.0];
const D1_LENS: [f32; 2] = [4453.0, 4217.0];
const D2_LENS: [f32; 2] = [3720.0, 3163.0];
/// The rate those lengths were written at.
const REF_FS: f32 = 29_761.0;

/// How long a Schroeder allpass of line length `len` and gain `g` actually
/// delays a signal, at the bottom of the band.
fn group_delay(len: f32, g: f32) -> f32 {
    (len * (1.0 + g) / (1.0 - g)).max(1.0)
}

pub struct Plate {
    input: [Allpass; 4],
    ap1: [Allpass; 2],
    ap2: [Allpass; 2],
    d1: [Delay; 2],
    d2: [Delay; 2],
    len1: [f32; 2],
    len2: [f32; 2],
    ap1_len: [f32; 2],
    ap2_len: [f32; 2],
    loss: [Loss; 2],
    scratch: Scratch,
    modu: [Modulator; 2],
    depth: f32,
    random: f32,
    chaos: f32,
    feed: [f32; 2],
    fs: f32,
    scale: f32,
}

impl Plate {
    pub fn new(fs: f32) -> Self {
        let k = fs / REF_FS;
        let cap = |n: f32| ((n * k * 4.0) as usize + 128).next_power_of_two();
        Plate {
            input: [
                Allpass::new(cap(IN_LENS[0])),
                Allpass::new(cap(IN_LENS[1])),
                Allpass::new(cap(IN_LENS[2])),
                Allpass::new(cap(IN_LENS[3])),
            ],
            ap1: [
                Allpass::new(cap(AP1_LENS[0])),
                Allpass::new(cap(AP1_LENS[1])),
            ],
            ap2: [
                Allpass::new(cap(AP2_LENS[0])),
                Allpass::new(cap(AP2_LENS[1])),
            ],
            d1: [
                Delay::with_capacity(cap(D1_LENS[0])),
                Delay::with_capacity(cap(D1_LENS[1])),
            ],
            d2: [
                Delay::with_capacity(cap(D2_LENS[0])),
                Delay::with_capacity(cap(D2_LENS[1])),
            ],
            len1: D1_LENS,
            len2: D2_LENS,
            ap1_len: AP1_LENS,
            ap2_len: AP2_LENS,
            loss: [Loss::default(), Loss::default()],
            scratch: Scratch::default(),
            modu: [Modulator::new(0x9E37_79B9), Modulator::new(0x85EB_CA6B)],
            depth: 0.0,
            random: 0.0,
            chaos: 0.0,
            feed: [0.0, 0.0],
            fs,
            scale: 1.0,
        }
    }

    pub fn clear(&mut self) {
        for a in &mut self.input {
            a.clear();
        }
        for i in 0..2 {
            self.ap1[i].clear();
            self.ap2[i].clear();
            self.d1[i].clear();
            self.d2[i].clear();
            self.loss[i].reset();
            self.feed[i] = 0.0;
        }
    }

    /// Apply a fit worked out elsewhere.
    pub fn apply(&mut self, curve: &Curve, fits: &[Fitted; 2]) {
        for (l, f) in self.loss.iter_mut().zip(fits.iter()) {
            l.apply(self.fs, curve, f);
        }
    }

    /// The decay this tank is actually producing at `f`, in seconds.
    ///
    /// Read back out of the loss filters that are running, the same way the
    /// network's is --- and **as approximate as the fit is**, for the same
    /// reason: the lap runs through allpasses whose delay depends on
    /// frequency, and this uses the one number the fit was given. The
    /// benchmark measures the tail itself and reports the difference, which
    /// is around 0.3 octaves.
    ///
    /// Drawing it anyway is the point. A line that is honestly approximate
    /// tells somebody where the tank is not doing what they asked; no line at
    /// all tells them nothing.
    pub fn realised_t60(&self, f: f32) -> f32 {
        let laps = self.laps();
        0.5 * (self.loss[0].t60(self.fs, laps[0] as usize, f)
            + self.loss[1].t60(self.fs, laps[1] as usize, f))
    }

    /// One lap of each half, which is what a fit for this tank is against.
    pub fn laps(&self) -> [f32; 2] {
        let mut out = [0.0f32; 2];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.len1[i]
                + self.len2[i]
                + group_delay(self.ap1_len[i], self.ap1[i].gain())
                + group_delay(self.ap2_len[i], self.ap2[i].gain());
        }
        out
    }

    /// Set the tank's size and diffusion, and refit its loss to the curve.
    ///
    /// `size` scales every length; at 1.0 it is the plate the numbers
    /// describe.
    pub fn set(&mut self, size: f32, diffusion: f32, curve: &Curve) {
        self.set_shape(size, diffusion);
        let laps = self.laps();
        let mut sc = std::mem::take(&mut self.scratch);
        for (l, lap) in self.loss.iter_mut().zip(laps.iter()) {
            l.fit(self.fs, *lap as usize, curve, &mut sc);
        }
        self.scratch = sc;
    }

    /// The geometry only, without fitting anything --- for the caller having
    /// the fit done off the audio thread.
    pub fn set_shape(&mut self, size: f32, diffusion: f32) {
        let k = self.fs / REF_FS * size.clamp(0.05, 4.0);
        self.scale = k;
        let d = diffusion.clamp(0.0, 1.0);
        for (i, a) in self.input.iter_mut().enumerate() {
            a.set(IN_LENS[i] * k, 0.25 + d * 0.5);
        }
        for i in 0..2 {
            self.ap1_len[i] = AP1_LENS[i] * k;
            self.ap2_len[i] = AP2_LENS[i] * k;
            self.len1[i] = D1_LENS[i] * k;
            self.len2[i] = D2_LENS[i] * k;
            // The first allpass of each half is the modulated one, and its
            // gain is negative in the published topology: that sign is what
            // makes the decay smooth rather than fluttering.
            self.ap1[i].set(self.ap1_len[i], -0.7 * (0.3 + d * 0.7));
            self.ap2[i].set(self.ap2_len[i], 0.5 * (0.3 + d * 0.7));
        }
    }

    pub fn set_modulation(&mut self, rate: f32, depth: f32, random: f32, chaos: f32) {
        for m in &mut self.modu {
            m.set_rate(self.fs, rate);
        }
        self.depth = depth * self.scale;
        self.random = random.clamp(0.0, 1.0);
        self.chaos = chaos.clamp(0.0, 1.0);
    }

    /// One lap of the figure of eight, in samples.
    pub fn lap(&self) -> f32 {
        self.len1[0] + self.len2[0] + self.ap1_len[0] + self.ap2_len[0]
    }

    /// One sample in, a stereo pair out.
    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let mut v = x;
        for a in &mut self.input {
            v = a.process(v);
        }

        let mut out = [0.0f32; 2];
        #[allow(clippy::needless_range_loop)] // `i` also indexes the far half
        for i in 0..2 {
            // Each half is fed by the *other* half's last output: that
            // crossing is the figure of eight.
            let y = v + self.feed[1 - i];
            let len = self.ap1_len[i] + self.modu[i].next(self.depth, self.random, self.chaos);
            let y = self.ap1[i].process_at(y, len);
            self.d1[i].push(y);
            let a = self.d1[i].read(self.len1[i]);
            let b = self.loss[i].process(a);
            let c = self.ap2[i].process(b);
            self.d2[i].push(c);
            out[i] = self.d2[i].read(self.len2[i]);
        }
        // Written only after both halves are read, so neither sees the other's
        // new value inside the same sample --- which would be a loop with no
        // delay in it.
        self.feed = out;

        // Tapped from inside the loop as well as from its output: that is what
        // makes the two sides different without making them two reverbs.
        let l = out[0] + self.d1[1].read(self.len1[1] * 0.61) * 0.6;
        let r = out[1] + self.d1[0].read(self.len1[0] * 0.43) * 0.6;
        (l * 0.5, r * 0.5)
    }
}
