//! Moving the delays about, which is what stops a tank ringing on one note.
//!
//! The survey found two kinds of modulation named again and again, and they
//! are not the same thing:
//!
//! - **Chorused** --- the read position is swept by a smooth oscillator. It
//!   detunes the tail, the way a chorus does, and at depth it is audible as
//!   pitch movement. Valhalla's mode notes call it "lush".
//! - **Randomised** --- the read position wanders on a slow random walk.
//!   VintageVerb's Random Space is described as reducing metallic artefacts
//!   "without the pitch change that can occur in the algorithms with chorused
//!   modulation", which is exactly the trade: less obvious, and it does not
//!   sing.
//!
//! - **Chaotic** --- the transport is not steady. Slow drift and fast jitter
//!   together, which is what VintageVerb's Chaotic modes mean by "modulation
//!   and saturation artifacts inspired by classic tape echoes... subtle wow
//!   and flutter chorusing that adds depth without making your reverb
//!   seasick".
//!
//! The first two are the ends of one axis and sit on one control between them.
//! Chaos is **not** on that axis: it is neither a steadier sweep nor a rougher
//! walk, it is two rates at once --- a drift far below the sweep's and a
//! flutter far above it --- and blending it into either would just make that
//! one noisier. It has its own amount.

/// One line's modulator: a sine and a random walk, crossfaded.
#[derive(Debug, Clone, Copy)]
pub struct Modulator {
    phase: f32,
    inc: f32,
    /// Where the random walk currently is, and where it is heading.
    from: f32,
    to: f32,
    t: f32,
    step: f32,
    rng: u32,
    /// The chaotic pair: a slow drift and a fast flutter, each with its own
    /// phase, because a tape's speed error is not one oscillation.
    wow: f32,
    wow_inc: f32,
    flutter: f32,
    flutter_inc: f32,
}

impl Modulator {
    /// `seed` must differ per line: two lines wandering in step is one line.
    pub fn new(seed: u32) -> Self {
        let mut m = Modulator {
            phase: 0.0,
            inc: 0.0,
            from: 0.0,
            to: 0.0,
            t: 1.0,
            step: 1.0 / 480.0,
            rng: seed | 1,
            wow: 0.0,
            wow_inc: 0.0,
            flutter: 0.0,
            flutter_inc: 0.0,
        };
        // Spread the starting phases so the lines do not all begin together,
        // which would be one modulator with the depth multiplied by the line
        // count for the first cycle.
        m.phase = (seed as f32 * 0.618_034) % 1.0;
        m
    }

    /// `rate` in hertz, at sample rate `fs`.
    pub fn set_rate(&mut self, fs: f32, rate: f32) {
        self.inc = rate.max(0.0) / fs;
        // The random walk moves at the same rate, so one control governs both
        // characters rather than the sound changing speed as it crossfades.
        self.step = (rate.max(0.01) / fs).min(0.5);
        // Wow and flutter keep their own rates, near the ones a tape machine
        // has: a drift under a hertz and a flutter around ten. Tying them to
        // the Rate control would make chaos a third flavour of the sweep,
        // which is what it is not.
        self.wow_inc = 0.6 / fs;
        self.flutter_inc = 9.3 / fs;
    }

    #[inline]
    fn next_rand(&mut self) -> f32 {
        // xorshift32: cheap, and its period is far past anything a reverb
        // tail will hear.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / 8_388_608.0 - 1.0
    }

    /// The next offset, in samples, for a depth of `depth` samples, a
    /// `random` blend from 0 (all sine) to 1 (all walk), and `chaos` of
    /// unsteady transport added on top.
    #[inline]
    pub fn next(&mut self, depth: f32, random: f32, chaos: f32) -> f32 {
        self.phase += self.inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let sine = (std::f32::consts::TAU * self.phase).sin();

        self.t += self.step;
        if self.t >= 1.0 {
            self.t -= 1.0;
            self.from = self.to;
            self.to = self.next_rand();
        }
        // Smoothstep between the walk's points: a straight line between them
        // has a corner at every point, and a corner in a delay length is a
        // click.
        let s = self.t * self.t * (3.0 - 2.0 * self.t);
        let walk = self.from + (self.to - self.from) * s;

        let steady = sine * (1.0 - random) + walk * random;
        if chaos <= 0.0 {
            return depth * steady;
        }

        self.wow += self.wow_inc;
        if self.wow >= 1.0 {
            self.wow -= 1.0;
        }
        self.flutter += self.flutter_inc;
        if self.flutter >= 1.0 {
            self.flutter -= 1.0;
        }
        // Deep and slow, plus shallow and fast. The proportions are a tape's:
        // most of the error is drift, and the flutter is what makes it sound
        // mechanical rather than merely detuned.
        let unsteady = 0.8 * (std::f32::consts::TAU * self.wow).sin()
            + 0.2 * (std::f32::consts::TAU * self.flutter).sin();
        depth * (steady + chaos * 2.5 * unsteady)
    }
}
