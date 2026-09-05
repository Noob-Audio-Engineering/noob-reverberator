//! The decay curve: how long the tail lasts at each frequency, and the filters
//! that make the network do it.
//!
//! Most reverbs give you a high cut and a bass multiplier --- two crossovers,
//! three regions. FabFilter's Pro-R gives a curve instead, and that is the
//! right shape for the control, because the thing being controlled is already
//! a function of frequency: `T60(f)`, the time the tail takes to fall 60 dB.
//!
//! # Why a curve is not just a nicer interface
//!
//! In a feedback delay network, a line of length `m` samples that carries a
//! loss filter `H(z)` comes back round every `m / fs` seconds having lost
//! `-20·log10|H|` decibels. So the decay at a frequency is fixed by the
//! **gain per trip**, and the gain a line needs is
//!
//! ```text
//! |H(f)| = 10^( -3·m / (fs · T60(f)) )
//! ```
//!
//! --- three decibels of the sixty per `m/fs` seconds of the `T60`. That is an
//! identity, not a fit: given the curve and the line length, the magnitude the
//! filter must have at every frequency is known exactly. What is *not* exact
//! is realising that magnitude with a filter of a few poles, and that is the
//! part this module measures rather than assumes.
//!
//! Every line therefore gets its **own** filter, because every line has its
//! own length: a short line goes round more often and must lose less each
//! time. Fitting one filter and sharing it is the mistake that makes a
//! network's real decay drift away from its nominal one as the lines spread.

/// A band of the decay curve. The shapes are the ones a decay curve needs:
/// two shelves for the ends, bells for the middle, and a notch for taking one
/// narrow region out of the tail without touching its neighbours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    LowShelf,
    Bell,
    HighShelf,
    Notch,
}

/// One band: it multiplies the decay time over a region of the spectrum.
///
/// `mult` is a **multiplier on time**, not a gain. 2.0 means the tail lasts
/// twice as long there. Keeping it in time rather than decibels is the whole
/// point of the control: the number on the band is the number a person is
/// thinking in.
#[derive(Debug, Clone, Copy)]
pub struct Band {
    pub on: bool,
    pub shape: Shape,
    pub freq: f32,
    /// Multiplier on the decay time in this band.
    pub mult: f32,
    /// Bandwidth in octaves for the bell and the notch; the slope width for
    /// the shelves.
    pub width: f32,
}

impl Default for Band {
    fn default() -> Self {
        Band {
            on: false,
            shape: Shape::Bell,
            freq: 1_000.0,
            mult: 1.0,
            width: 1.0,
        }
    }
}

/// The most bands the curve can carry. Six, which is what Pro-R offers and
/// which is already more regions than a decay curve usually needs --- the
/// limit exists so the fit runs in bounded time in the audio thread's
/// preparation, not because a seventh would be wrong.
pub const MAX_BANDS: usize = 6;

/// The whole curve: a base time, and bands that multiply it.
#[derive(Debug, Clone, Copy)]
pub struct Curve {
    /// The decay time with every band flat, in seconds.
    pub base: f32,
    pub bands: [Band; MAX_BANDS],
}

impl Default for Curve {
    fn default() -> Self {
        Curve {
            base: 2.0,
            bands: [Band::default(); MAX_BANDS],
        }
    }
}

impl Curve {
    /// The decay time this curve asks for at `f`, in seconds.
    ///
    /// # Why the bands combine in the reciprocal and not by multiplying
    ///
    /// The obvious definition is `T60 = base × Π multᵢ^{sᵢ(f)}` --- each band
    /// multiplies the time over its region. I wrote that one first, and it is
    /// **not realisable**, for a reason worth writing down.
    ///
    /// A delay line's loss per trip goes as one over the decay time, and a
    /// cascade of filters *adds* in decibels. So the quantity the machine can
    /// build out of band shapes is the **loss**, linearly:
    ///
    /// ```text
    /// C(f) = C₀ · ( 1 + Σᵢ (1/multᵢ − 1) · sᵢ(f) )
    /// ```
    ///
    /// Defining the curve that way and reading the time off it,
    ///
    /// ```text
    /// T60(f) = base / ( 1 + Σᵢ (1/multᵢ − 1) · sᵢ(f) )
    /// ```
    ///
    /// gives a target that is a sum of filter shapes rather than a product of
    /// powers of them --- linear in exactly the parameters a filter has. At a
    /// band's own centre `s = 1` and the time is `base × mult` exactly, which
    /// is the promise the control makes; away from it the curve bends the way
    /// the filters bend, so the drawn curve is one the machine can make.
    ///
    /// The multiplying definition asked for an *exponential* in each band's
    /// shape, and a filter cascade cannot make one. Measured, that cost 0.21
    /// octaves of decay time at a shelf's corner on a hard bend, and no amount
    /// of fitting removed it, because it was not a fitting error.
    ///
    /// # Two bands can ask for silence to last forever
    ///
    /// Two bands over the same region each asking for twice the decay each
    /// take half the loss away, and together they take all of it. That is the
    /// arithmetic being honest: the clamp at [`MAX_T60`] is what stops it, and
    /// the curve drawn on the panel shows the clamp rather than hiding it.
    pub fn t60(&self, f: f32) -> f32 {
        let mut inv = 1.0f32;
        for b in &self.bands {
            if b.on {
                let m = match b.shape {
                    Shape::Notch => b.mult.min(1.0),
                    _ => b.mult,
                };
                inv += (1.0 / m.max(0.01) - 1.0) * b.shape_at(f);
            }
        }
        // Below zero the loss would have changed sign --- the line would gain
        // energy every trip. The floor is what makes "this cannot run away" a
        // property of the curve rather than of the fit.
        (self.base / inv.max(1.0 / (MAX_T60 / MIN_T60))).clamp(MIN_T60, MAX_T60)
    }
}

/// The shortest decay the network will be asked for. Below this the loss per
/// trip on a long line stops being representable --- the filter would need
/// more attenuation than a one-pole shape can hold --- and the tail turns into
/// a click rather than a short reverb.
pub const MIN_T60: f32 = 0.05;
/// The longest. Sixty seconds is past every published figure in the survey and
/// well short of the point where the network's own rounding sustains it.
pub const MAX_T60: f32 = 60.0;

impl Band {
    /// The Q this band's shape is drawn with --- and the Q the filter that
    /// realises it is built with. One number, used in both places, because
    /// the two being allowed to differ is what made the first version of this
    /// curve undrawable.
    pub fn q(&self) -> f32 {
        match self.shape {
            Shape::Bell => 1.0 / (2.0 * self.width.max(0.05).sinh().max(1e-3)),
            Shape::Notch => 2.5 / (2.0 * self.width.max(0.05).sinh().max(1e-3)),
            Shape::LowShelf | Shape::HighShelf => SHELF_Q,
        }
    }

    /// How much this band multiplies the decay at its own centre.
    ///
    /// The multiplier is exact there by construction --- see [`Curve::t60`]
    /// for how the bands combine away from it, which is not by multiplying.
    pub fn centre_mult(&self) -> f32 {
        match self.shape {
            Shape::Notch => self.mult.min(1.0),
            _ => self.mult,
        }
    }

    /// The band's normalised shape at `f`: 1 at its centre, 0 far from it.
    ///
    /// These are the analogue prototypes of the Audio EQ Cookbook filters in
    /// the limit of a small gain, where the shape stops depending on the gain.
    /// They carry no sample rate, which is why the drawn curve does not move
    /// when the session's rate does; the digital filter's warping near Nyquist
    /// is left for the fit to absorb, and for the benchmark to report.
    pub fn shape_at(&self, f: f32) -> f32 {
        let q = self.q().max(0.05);
        match self.shape {
            // A peaking filter's normalised response is a Lorentzian in the
            // bandpass variable `1/x − x`.
            Shape::Bell | Shape::Notch => {
                let x = (f / self.freq).max(1e-9);
                let u = 1.0 / x - x;
                1.0 / (1.0 + (q * u) * (q * u))
            }
            Shape::LowShelf => shelf_shape(f / self.freq, q),
            // The high shelf is the low shelf read backwards.
            Shape::HighShelf => shelf_shape(self.freq / f.max(1e-9), q),
        }
    }
}

/// The Q both shelves are drawn and built with. 0.7 is the usual choice: the
/// steepest a single biquad shelf goes without overshooting past its corner.
pub const SHELF_Q: f32 = 0.7;

/// A low shelf's normalised shape: 1 well below the corner, a half at it, 0
/// well above.
fn shelf_shape(x: f32, q: f32) -> f32 {
    let x = x.max(1e-9);
    let x2 = x * x;
    let d = (1.0 - x2) * (1.0 - x2) + x2 / (q * q);
    (0.5 * (1.0 + (1.0 - x2 * x2) / d.max(1e-12))).clamp(0.0, 1.0)
}
