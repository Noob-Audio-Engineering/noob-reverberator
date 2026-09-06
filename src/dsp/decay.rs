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

/// The shapes a decay band can take, and the maths behind them, come from
/// [`noob_band_shapes`] --- the same crate Noob-Q's filters are described by.
///
/// They are the same vocabulary because they are the same question asked
/// twice: what does a band of this kind, at this frequency, of this width,
/// look like across the spectrum? What differs is what the answer is used
/// for, and that difference stays here rather than in the crate.
pub use noob_band_shapes::{KIND_NAMES as SHAPE_NAMES, Kind as Shape, SHELF_Q};

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
                inv += b.loss_amount() * b.shape_at(f);
            }
        }
        // Below zero the loss would have changed sign --- the line would gain
        // energy every trip. The floor is what makes "this cannot run away" a
        // property of the curve rather than of the fit.
        (self.base / inv.max(1.0 / (MAX_T60 / MIN_T60))).clamp(MIN_T60, MAX_T60)
    }

    /// The decay this curve asks for **averaged over an octave** around `f`.
    ///
    /// A band-passed measurement cannot see a point on the curve; it sees a
    /// band. Comparing what it reports against `t60(f)` therefore compares two
    /// different quantities, and it does so worst exactly where the curve is
    /// steepest --- at a shelf's corner, which is where somebody looking for a
    /// fault would look first. Measured at a 250 Hz shelf, that mismatch alone
    /// accounted for 0.18 octaves.
    ///
    /// Averaged in the logarithm of the time, over the logarithm of the
    /// frequency, because both are how the quantities are heard and how they
    /// are drawn.
    pub fn t60_over_octave(&self, f: f32) -> f32 {
        const STEPS: usize = 17;
        let lo = f / std::f32::consts::SQRT_2;
        let hi = f * std::f32::consts::SQRT_2;
        let mut acc = 0.0f32;
        for i in 0..STEPS {
            let t = i as f32 / (STEPS - 1) as f32;
            acc += self.t60(lo * (hi / lo).powf(t)).ln();
        }
        (acc / STEPS as f32).exp()
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
    /// The Q this band is drawn with --- and the Q the filter that realises it
    /// is built with. One number, used in both places, because the two being
    /// allowed to differ is what made the first version of this curve
    /// undrawable.
    ///
    /// Width is in octaves here and the crate speaks Q, so the conversion
    /// happens once, at the boundary.
    pub fn q(&self) -> f32 {
        match self.shape {
            Shape::Bell => noob_band_shapes::q_from_octaves(self.width),
            // A notch is narrower than a bell of the same stated width: it is
            // there to take one ring out of a tail, not to shape a region.
            Shape::Notch => noob_band_shapes::q_from_octaves(self.width) * 2.5,
            Shape::LowShelf | Shape::HighShelf | Shape::Tilt => SHELF_Q,
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

    /// How much this band moves the **loss**, which is the quantity the bands
    /// actually add up in. See [`Curve::t60`].
    ///
    /// # A bipolar band needs different arithmetic, and getting it wrong is
    /// not obvious
    ///
    /// For a shape with one side, `1/mult − 1` is exactly right: at the centre
    /// the shape is one, the loss is divided by `mult`, and the decay there is
    /// `mult` times the base. Read the arithmetic and it is done.
    ///
    /// A tilt has no centre --- its shape runs from `−½` to `+½` --- so that
    /// formula makes `mult` mean neither end. At four it gave lows of `0.73×`
    /// and highs of `1.6×`: an end-to-end ratio of 2.2 from a control saying
    /// four. Worse, a strong tilt the other way drove the loss **negative** at
    /// one end and hit the clamp, so half the control's travel did the same
    /// thing.
    ///
    /// Asking for the ratio between the *ends* to be `mult` and solving gives
    /// `2(1 − mult) / (1 + mult)`, which lands in `(−2, 2)` for every positive
    /// multiplier --- so it can never take the loss through zero, and the
    /// number on the control is the ratio a person can hear between the two
    /// ends of the band.
    pub fn loss_amount(&self) -> f32 {
        let m = self.centre_mult().max(0.01);
        if self.shape.is_bipolar() {
            2.0 * (1.0 - m) / (1.0 + m)
        } else {
            1.0 / m - 1.0
        }
    }

    /// The band's shape at `f`, with the amount divided out.
    ///
    /// Straight from [`noob_band_shapes::prototype`]: this is the same shape
    /// an equaliser's filter of the same kind would have, in the limit where
    /// the gain stops changing it. That limit is exactly what a reverb needs,
    /// because one drawn curve becomes a different filter on every delay line
    /// and those filters have different gains.
    pub fn shape_at(&self, f: f32) -> f32 {
        noob_band_shapes::prototype(self.shape, f, self.freq, self.q())
    }
}
