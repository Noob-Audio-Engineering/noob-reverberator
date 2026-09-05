//! Pitch shifting, for the modes that put a transposed copy back into the tank.
//!
//! Shimmer is a reverb whose feedback path is transposed, so each pass up the
//! loop is an octave higher than the last and the tail climbs instead of just
//! fading. Strymon's Shimmer is described as "two tunable voices"; Valhalla
//! ship a whole plug-in for it. Both want the same primitive: a shifter cheap
//! enough to sit inside a feedback loop and smooth enough that a sustained
//! note does not gargle.
//!
//! # Two taps, crossfaded, and why
//!
//! Transposing with a delay line means reading it at a rate other than one
//! sample per sample. The read position then drifts, and eventually it has to
//! jump --- which is a click, on every jump, forever.
//!
//! The fix is two read positions half a buffer apart, each crossfaded in while
//! the other is near its jump, so the jump always happens under a fade that is
//! already at zero. What that costs is a periodic comb: the two taps are the
//! same signal at different delays, and where they overlap they interfere.
//! With a raised-cosine fade the interference is confined to the fade and the
//! artefact is a gentle warble at the tap rate, which inside a reverb is
//! indistinguishable from the modulation already there.
//!
//! It is not a phase vocoder and does not pretend to be. Inside a reverb tail,
//! after the diffusers have already scrambled the phases, the difference does
//! not survive to the output --- and a phase vocoder's latency would.

use super::delay::Delay;

pub struct Shifter {
    line: Delay,
    /// Where each tap is reading, in samples behind the write head.
    pos: [f32; 2],
    /// How far the read position moves per sample, relative to writing.
    rate: f32,
    /// The window the taps travel over, in samples.
    window: f32,
}

impl Shifter {
    pub fn new(window: usize) -> Self {
        let cap = (window * 2 + 64).next_power_of_two();
        Shifter {
            line: Delay::with_capacity(cap),
            pos: [0.0, window as f32 * 0.5],
            rate: 0.0,
            window: window as f32,
        }
    }

    pub fn clear(&mut self) {
        self.line.clear();
        self.pos = [0.0, self.window * 0.5];
    }

    /// Transpose by `semitones`. Zero is a plain delay of half the window.
    pub fn set_semitones(&mut self, semitones: f32) {
        let ratio = 2f32.powf(semitones.clamp(-24.0, 24.0) / 12.0);
        // The tap's position is a *delay*, so raising the pitch means the
        // delay must shrink: the read head has to catch up with the write
        // head. `rate` is therefore `1 − ratio`, and the resulting pitch
        // factor is `1 − rate`.
        //
        // With the sign the other way round the factor comes out `2 − ratio`,
        // which is a plausible interval that is not the one asked for --- an
        // octave down arrives as a fifth up. It sounds like a pitch shifter
        // working, which is why this is measured rather than reasoned about.
        self.rate = 1.0 - ratio;
    }

    /// The window length in samples. Longer is smoother and more smeared in
    /// time; shorter is tighter and warbles more.
    pub fn set_window(&mut self, window: f32) {
        let cap = self.line.capacity() as f32 * 0.5 - 8.0;
        self.window = window.clamp(64.0, cap);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.line.push(x);
        let w = self.window;
        let mut out = 0.0;
        for p in self.pos.iter_mut() {
            *p += self.rate;
            // Wrapped rather than clamped: the tap runs off one end of the
            // window and comes back on the other, which is the jump the fade
            // exists to hide.
            while *p < 0.0 {
                *p += w;
            }
            while *p >= w {
                *p -= w;
            }
            // A raised cosine over the window: zero at both ends, so the jump
            // happens where the tap is contributing nothing.
            let phase = *p / w;
            let g = 0.5 - 0.5 * (std::f32::consts::TAU * phase).cos();
            out += self.line.read(*p + 2.0) * g;
        }
        out
    }
}
