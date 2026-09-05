//! The loss filter on one delay line, fitted to the decay curve.
//!
//! # The arithmetic, because the signs are where this goes wrong
//!
//! A line of `m` samples at `fs` comes round every `m/fs` seconds. To fall
//! 60 dB in `T60` seconds it must lose
//!
//! ```text
//! A(f) = -60 · (m / fs) / T60(f)      decibels per trip
//! ```
//!
//! `A` is negative: a loss. With the curve written as a base time multiplied
//! by band weights, `T60(f) = base · W(f)`, so
//!
//! ```text
//! A(f) = A0 / W(f),      A0 = -60 · (m / fs) / base
//! ```
//!
//! and that is **division**, where a cascade of filters gives addition in
//! decibels. Splitting it,
//!
//! ```text
//! A(f) = A0 + A0 · (1/W(f) − 1)
//! ```
//!
//! and the tempting next step --- one filter per band, each with the gain that
//! makes its own centre right --- is only correct where the bands do not
//! overlap. I wrote that version first. It is exact on a flat curve and at an
//! isolated band's centre, and it was **0.44 octaves of decay time** out at
//! 7.5 kHz between a low shelf and a high shelf, because a shelf's skirt is
//! not the shape the weight function draws and two skirts do not add.
//!
//! So the gains are **fitted** rather than derived: the target `A(f)` is
//! sampled on a log-frequency grid, each band contributes the shape its own
//! biquad actually has, and the gains are the least-squares solution. Then it
//! is done again against what the filters measured --- three passes, because
//! a peaking filter's response is not quite linear in its gain, and one pass
//! leaves that non-linearity in the answer.
//!
//! The point of doing it this way is that the fit's error is a *measured*
//! quantity. [`Loss::t60`] reads the coefficients that are running, so the
//! benchmark's row is the reverb's own answer and not the design's intent.
//!
//! # Why this is fitted per line and not once
//!
//! `A0` carries `m`. A short line goes round more often and must lose less
//! each time to reach the same `T60`. Fitting one filter and sharing it across
//! lines gives every line the loss of the average line, and the network's real
//! decay then drifts from its nominal one as the lengths spread --- worst
//! exactly where the lengths are most spread, which is where a network is
//! trying hardest not to sound like a comb filter.

use super::decay::{Curve, MAX_BANDS, Shape};
use super::svf::{Kind, Svf};

/// The per-trip loss for one line.
#[derive(Debug, Clone)]
pub struct Loss {
    /// The flat part, as a linear gain.
    gain: f32,
    /// Two slots per drawn band. Most shapes use one; a **tilt** is two
    /// shelves back to back and uses both, because there is no single biquad
    /// that lifts one end and drops the other. The second slot of a band that
    /// does not need it passes everything through.
    bands: [Svf; MAX_BANDS * 2],
    /// Which drawn bands are tilts, so `process` knows to run the second slot.
    tilt: [bool; MAX_BANDS],
    active: usize,
}

impl Default for Loss {
    fn default() -> Self {
        Loss {
            gain: 0.0,
            bands: [Svf::default(); MAX_BANDS * 2],
            tilt: [false; MAX_BANDS],
            active: 0,
        }
    }
}

/// The most a single band may bend the loss, in decibels per trip.
///
/// Without a cap a band asking for a very long decay on a very long line wants
/// a positive gain, and a filter that adds energy inside a feedback loop is a
/// filter that will eventually find a way to keep it. The cap is what makes
/// "the network cannot run away" a property rather than a hope, and the test
/// that holds it is worth more than the comment.
pub const MAX_BAND_DB: f32 = 24.0;

impl Loss {
    /// Fit this line's filter to the curve.
    ///
    /// `m` is the line's length in samples. Coefficients are replaced but the
    /// filter states are left alone, so this can run while the tail rings.
    ///
    /// Gauss--Newton on two parameters per band --- its gain and its Q ---
    /// plus one flat term, against the weighted residual. Gain alone leaves a
    /// shape error at the corners that grows with how hard the curve is bent,
    /// because the curve is drawn with each band's *small-gain* skirt and a
    /// filter at a large gain has a different one. Q is the freedom that fixes
    /// it: measured over the sample rates and line lengths in the benchmark,
    /// letting the fit move Q takes the worst error at the hardest bend from
    /// 0.235 octaves of decay time to what `docs/BENCHMARK.md` reports.
    pub fn fit(&mut self, fs: f32, m: usize, curve: &Curve) {
        let trip = m as f32 / fs;
        let a0_db = -60.0 * trip / curve.base.max(super::decay::MIN_T60);

        let mut used: [usize; MAX_BANDS] = [0; MAX_BANDS];
        let mut n = 0;
        for (i, b) in curve.bands.iter().enumerate() {
            if b.on && (b.mult - 1.0).abs() >= 1e-4 {
                used[n] = i;
                n += 1;
            }
        }
        self.active = n;
        if n == 0 {
            self.gain = 10f32.powf(a0_db / 20.0);
            for f in &mut self.bands {
                f.unity();
            }
            return;
        }

        // The curve is drawn over the audible band, so that is where it is
        // fitted. Above 20 kHz no decay time is being promised, and fitting
        // there would spend the solve's accuracy on inaudible frequencies.
        let top = (fs * 0.45).min(FIT_TOP);
        let grid: Vec<f32> = (0..GRID)
            .map(|i| FIT_BOTTOM * (top / FIT_BOTTOM).powf(i as f32 / (GRID - 1) as f32))
            .collect();
        let target: Vec<f32> = grid
            .iter()
            .map(|&f| a0_db * (curve.base / curve.t60(f)))
            .collect();
        // Weighted by one over the loss being asked for, because what a person
        // hears is the decay *time*, and time goes as one over the loss. An
        // unweighted fit minimises decibels, which spends its accuracy at the
        // top of the band and leaves the bottom --- where a trip loses a
        // hundredth of a decibel --- free to be half again too long.
        let w: Vec<f32> = target.iter().map(|t| 1.0 / t.abs().max(1e-4)).collect();

        // The parameters: a gain and a log-Q per band, then the flat term.
        let np = 2 * n + 1;
        // Started at the answer the derivation gives --- one filter per band,
        // each with the gain that makes its own centre exactly right. That is
        // correct where the bands do not overlap, so it is a good place to
        // begin and a poor place to stop.
        let mut th = [0.0f32; NP];
        for j in 0..n {
            let b = &curve.bands[used[j]];
            let m = match b.shape {
                Shape::Notch => b.mult.min(1.0),
                _ => b.mult,
            };
            th[j] = (a0_db * (1.0 / m.max(0.01) - 1.0)).clamp(-MAX_BAND_DB, MAX_BAND_DB);
            th[n + j] = 0.0; // log2 of the Q multiplier, starting at one
        }
        th[np - 1] = a0_db;

        let mut best = f32::INFINITY;
        let mut best_th = th;
        for _ in 0..PASSES {
            self.build(fs, curve, &used, &th, n);
            let mut resid = vec![0.0f32; GRID];
            let mut err = 0.0f32;
            for (k, &f) in grid.iter().enumerate() {
                resid[k] = (target[k] - self.loss_db(fs, f)) * w[k];
                err += resid[k] * resid[k];
            }
            if err < best {
                best = err;
                best_th = th;
            }

            // The Jacobian by central differences: each parameter is nudged,
            // the filters are rebuilt, and the change in what they actually do
            // is measured. Differencing the real thing rather than a formula
            // for it means a clamp or a degenerate Q shows up as a column of
            // zeros --- the solve then leaves that parameter alone instead of
            // taking a step into a wall.
            let mut jac = vec![[0.0f32; NP]; GRID];
            for pi in 0..np {
                let h = if pi < n { 0.25 } else { 0.05 };
                let mut plus = th;
                let mut minus = th;
                plus[pi] += h;
                minus[pi] -= h;
                let mut probe = self.clone();
                probe.build(fs, curve, &used, &plus, n);
                let hi: Vec<f32> = grid.iter().map(|&f| probe.loss_db(fs, f)).collect();
                probe.build(fs, curve, &used, &minus, n);
                for (k, &f) in grid.iter().enumerate() {
                    jac[k][pi] = w[k] * (hi[k] - probe.loss_db(fs, f)) / (2.0 * h);
                }
            }

            let Some(step) = solve(&jac, &resid, np) else {
                break;
            };
            // Half a step. Gauss--Newton on a response that is not linear in
            // its own gain overshoots, and an overshoot here is a filter with
            // a Q of forty ringing inside a feedback loop.
            for pi in 0..np {
                th[pi] += 0.5 * step[pi];
            }
            for j in 0..n {
                th[j] = th[j].clamp(-MAX_BAND_DB, MAX_BAND_DB);
                th[n + j] = th[n + j].clamp(-2.0, 2.0);
            }
            th[np - 1] = th[np - 1].clamp(-120.0, 0.0);
        }
        // Whichever pass was actually best, not whichever was last.
        self.build(fs, curve, &used, &best_th, n);
    }

    /// Realise a parameter vector as filters. Separated out because the fit
    /// calls it once per pass, twice per Jacobian column, and once more at the
    /// end --- and a fit that measured one arrangement and left another
    /// running would be the worst kind of bug to find, because everything
    /// would sound almost right.
    fn build(
        &mut self,
        fs: f32,
        curve: &Curve,
        used: &[usize; MAX_BANDS],
        th: &[f32; 2 * MAX_BANDS + 1],
        n: usize,
    ) {
        self.gain = 10f32.powf(th[2 * n].clamp(-120.0, 0.0) / 20.0);
        for j in 0..n {
            let b = &curve.bands[used[j]];
            let (first, second) = self.bands.split_at_mut(MAX_BANDS);
            shape(&mut first[j], &mut second[j], fs, b, th[j], th[n + j]);
            self.tilt[j] = b.shape == Shape::Tilt;
        }
        for j in n..MAX_BANDS {
            self.bands[j].unity();
            self.bands[MAX_BANDS + j].unity();
            self.tilt[j] = false;
        }
        self.active = n;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = x * self.gain;
        for j in 0..self.active {
            y = self.bands[j].process(y);
            if self.tilt[j] {
                y = self.bands[MAX_BANDS + j].process(y);
            }
        }
        y
    }

    pub fn reset(&mut self) {
        for f in &mut self.bands {
            f.reset();
        }
    }

    /// The loss this filter actually applies at `f`, in decibels per trip.
    ///
    /// This reads the coefficients that are running, not the design intent, so
    /// a fit that failed shows up here as the wrong number rather than as the
    /// number that was asked for.
    pub fn loss_db(&self, fs: f32, f: f32) -> f32 {
        // Accumulated in double precision for the same reason the magnitude is
        // computed there: this number is divided into 60 to get a decay time,
        // so its low-order bits are the answer's high-order ones.
        let mut m = self.gain as f64;
        for j in 0..self.active {
            m *= self.bands[j].magnitude(fs, f) as f64;
            if self.tilt[j] {
                m *= self.bands[MAX_BANDS + j].magnitude(fs, f) as f64;
            }
        }
        (20.0 * m.max(1e-300).log10()) as f32
    }

    /// The decay time this line will actually produce at `f`, in seconds.
    ///
    /// The inverse of the identity at the top: if a trip of `m/fs` seconds
    /// loses `L` decibels, sixty decibels take `60/|L|` trips.
    pub fn t60(&self, fs: f32, m: usize, f: f32) -> f32 {
        let l = self.loss_db(fs, f);
        if l >= -1e-6 {
            return f32::INFINITY;
        }
        (m as f32 / fs) * (60.0 / -l)
    }
}

/// The size of the parameter vector: a gain and a Q per band, plus the flat
/// term.
const NP: usize = 2 * MAX_BANDS + 1;

/// How many points the fit is judged on. Log-spaced over the audible band:
/// enough that a narrow band cannot slip between two of them, few enough that
/// the solve is not worth thinking about.
const GRID: usize = 96;
/// How many measure-and-correct passes. The fit keeps the best pass rather
/// than the last, so more of them can only help the answer --- what they cost
/// is time, and this runs when a control moves, not per sample.
const PASSES: usize = 12;
/// The band the decay curve is drawn and fitted over, and the only band any
/// decay figure in this repository refers to.
pub const FIT_BOTTOM: f32 = 20.0;
pub const FIT_TOP: f32 = 20_000.0;
/// Design one band's filters at a given gain, with the fit's Q adjustment in
/// octaves either side of the Q the curve is drawn with.
///
/// `second` is only used by a tilt, which is two shelves: half the amount up
/// below the corner and half down above it, so the *difference* between the
/// ends is the amount --- which is what the control's number means, here and
/// in Noob-Q. There is no single biquad that lifts one end and drops the
/// other, so a tilt genuinely costs two filters per delay line.
fn shape(
    first: &mut Svf,
    second: &mut Svf,
    fs: f32,
    b: &super::decay::Band,
    gain_db: f32,
    log_q: f32,
) {
    let q = b.q() * 2f32.powf(log_q);
    match b.shape {
        Shape::Bell | Shape::Notch => {
            first.set(Kind::Bell, fs, b.freq, q, gain_db);
            second.unity();
        }
        Shape::LowShelf => {
            first.set(Kind::LowShelf, fs, b.freq, q, gain_db);
            second.unity();
        }
        Shape::HighShelf => {
            first.set(Kind::HighShelf, fs, b.freq, q, gain_db);
            second.unity();
        }
        Shape::Tilt => {
            first.set(Kind::LowShelf, fs, b.freq, q, gain_db * 0.5);
            second.set(Kind::HighShelf, fs, b.freq, q, gain_db * -0.5);
        }
    }
}

/// Least squares for a very small system, by normal equations and Gaussian
/// elimination with partial pivoting.
///
/// `n` is at most seven, so the conditioning that would rule normal equations
/// out at any real size is not a concern here --- and a singular system means
/// two bands sitting exactly on top of each other, which returns `None` and
/// leaves the previous gains standing rather than producing a filter nobody
/// designed.
fn solve(rows: &[[f32; NP]], rhs: &[f32], n: usize) -> Option<[f32; NP]> {
    // Normal equations in double precision. The rows are weighted by one over
    // the loss, which near the bottom of the band is a weight of a hundred
    // against a weight of one at the top; squaring that into a normal matrix
    // spans four orders of magnitude before the filters are even considered.
    // In `f32` the solve returned a different answer for the same curve at a
    // different sample rate, which is a solver reporting on its own
    // conditioning rather than on the filters.
    let mut a = [[0.0f64; NP + 1]; NP];
    for (row, &r) in rows.iter().zip(rhs.iter()) {
        for i in 0..n {
            for j in 0..n {
                a[i][j] += row[i] as f64 * row[j] as f64;
            }
            a[i][n] += row[i] as f64 * r as f64;
        }
    }
    // A ridge in proportion to the matrix, not an absolute one: a fixed
    // whisker is a rounding error on a well-scaled system and the whole answer
    // on a badly scaled one.
    let scale = (0..n).map(|i| a[i][i]).fold(0.0f64, f64::max);
    if scale <= 0.0 || !scale.is_finite() {
        return None;
    }
    for (i, row) in a.iter_mut().enumerate().take(n) {
        row[i] += scale * 1e-9;
    }

    for col in 0..n {
        let mut piv = col;
        for r in col + 1..n {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col].abs() < scale * 1e-14 {
            return None;
        }
        a.swap(col, piv);
        let d = a[col][col];
        for v in a[col][col..=n].iter_mut() {
            *v /= d;
        }
        for r in 0..n {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    let (pivot_row, other) = if r < col {
                        let (lo, hi) = a.split_at_mut(col);
                        (&hi[0], &mut lo[r])
                    } else {
                        let (lo, hi) = a.split_at_mut(r);
                        (&lo[col], &mut hi[0])
                    };
                    for j in col..=n {
                        other[j] -= f * pivot_row[j];
                    }
                }
            }
        }
    }
    let mut out = [0.0f32; NP];
    for i in 0..n {
        if !a[i][n].is_finite() {
            return None;
        }
        out[i] = a[i][n] as f32;
    }
    Some(out)
}
