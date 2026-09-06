//! Reading a decay time back out of a tail.
//!
//! Everything here works on **rendered audio** and knows nothing about the
//! filters that made it. That is the point: the fit can tell you what it
//! intended, and this tells you what came out, and a plug-in whose claim is
//! "the decay you draw is the decay you get" needs the second one.
//!
//! The method is the standard one for rooms: filter the impulse response into
//! a band, integrate its energy backwards from the end (Schroeder), and fit a
//! straight line to the part of that curve between −5 dB and −35 dB. Sixty
//! decibels is twice that span, so the answer is the fit's slope scaled --- a
//! `T30`. Measuring the last sixty decibels directly would be measuring the
//! noise floor of whatever is left in the network.

use super::svf::{Kind, Svf};

/// The decay time of `ir` in an octave band around `f`, in seconds.
///
/// `None` when the tail does not fall far enough to fit --- too short a
/// recording, or a band with nothing in it. Returning nothing is the honest
/// answer there; returning a number fitted to noise is how a measurement comes
/// to agree with whatever it is asked to.
pub fn t60_in_band(ir: &[f32], fs: f32, f: f32) -> Option<f32> {
    if f <= 0.0 || f >= fs * 0.45 {
        return None;
    }
    // Four band-passes in series. Two was not enough, and the way that showed
    // is worth recording: on a tail tilted four to one across the band, the
    // 500 Hz reading came out **a full octave long** --- 2.83 s against a
    // drawn 1.43 s --- because five-second energy at the top of the band leaks
    // through a gentle skirt and is what is left by the time the energy curve
    // has fallen thirty decibels. The fit then measures the neighbour.
    //
    // Four sections put the skirts far enough down that the band measures
    // itself. It costs nothing that matters: this runs on a rendered tail, not
    // in the audio thread.
    let mut bp: [Svf; 4] = [Svf::default(); 4];
    for s in bp.iter_mut() {
        s.set(Kind::BandPass, fs, f, 1.41, 0.0);
    }
    let filtered: Vec<f32> = ir
        .iter()
        .map(|&x| bp.iter_mut().fold(x, |v, s| s.process(v)))
        .collect();

    // Backwards energy integration, in double precision: the sum runs over
    // hundreds of thousands of squares spanning sixty decibels, and in `f32`
    // the small ones at the end vanish into the large ones at the start ---
    // which bends the curve exactly where the fit is reading it.
    let mut edc = vec![0.0f64; filtered.len() + 1];
    for i in (0..filtered.len()).rev() {
        edc[i] = edc[i + 1] + (filtered[i] as f64) * (filtered[i] as f64);
    }
    let total = edc[0];
    if total <= 0.0 {
        return None;
    }

    let db: Vec<f64> = edc[..filtered.len()]
        .iter()
        .map(|&e| 10.0 * (e / total).max(1e-300).log10())
        .collect();

    let start = db.iter().position(|&d| d <= -5.0)?;
    let end = db.iter().position(|&d| d <= -35.0)?;
    if end <= start + 16 {
        return None;
    }

    // Least squares on the straight part.
    let n = (end - start) as f64;
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (i, &y) in db.iter().enumerate().take(end).skip(start) {
        let x = i as f64;
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
    }
    let denom = n * sxx - sx * sx;
    if denom.abs() < 1e-12 {
        return None;
    }
    let slope = (n * sxy - sx * sy) / denom; // decibels per sample
    if slope >= -1e-12 {
        return None;
    }
    Some((-60.0 / slope / fs as f64) as f32)
}

/// The octave centres a tail is measured at: ISO centres from 31.5 Hz up.
pub const BANDS: [f32; 10] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
