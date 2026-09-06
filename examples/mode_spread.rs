//! How different do the modes actually sound from one another?
//!
//! `cargo run --release --features plugin --example mode_spread`
//!
//! Every mode is rendered from the same impulse and reduced to the handful of
//! things a listener separates reverbs by: how long it rings in each octave,
//! how bright the tail is, how quickly the echoes fill in, how long it takes
//! to arrive, how much it moves, and how wide it is. Then every pair is
//! compared.
//!
//! The point is to find modes that are *the same reverb under two names*. A
//! mode is allowed to be close to its neighbours --- a room and a small hall
//! are close in the world too --- but a table of thirty-two names implies
//! thirty-two places to stand.
use noob_reverberator::dsp::engine::{Reverb, read_settings};
use noob_reverberator::dsp::measure::t60_in_band;
use noob_reverberator::dsp::mode::MODES;

const FS: f32 = 48_000.0;
const BANDS: [f32; 6] = [125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0];

/// What one mode sounds like, in numbers.
struct Face {
    name: &'static str,
    arch: String,
    /// T60 per octave band, in log2 seconds --- so a ratio is a distance.
    t60: [f32; 6],
    /// The tail's brightness: spectral centroid in log2 hertz.
    centroid: f32,
    /// How long the reverb takes to reach its loudest, in log2 milliseconds.
    build: f32,
    /// Echo density: how filled-in the first 80 ms is, 0 sparse to 1 solid.
    density: f32,
    /// How much the tail moves, as the spread of its own envelope.
    movement: f32,
    /// 0 is mono, 1 is fully decorrelated.
    width: f32,
}

fn render(i: usize) -> (Vec<f32>, Vec<f32>) {
    let (bridge, ix) = noob_reverberator::dsp::build_bridge("probe", FS, true);
    let audio = bridge.take_audio().expect("the audio handle");
    let mut s = read_settings(&audio, &ix);
    Reverb::apply_mode(&mut s, i);
    s.mode = i;
    s.mix = 1.0;
    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    let n = (FS * 4.0) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    l[0] = 1.0;
    r[0] = 1.0;
    rev.process(&s, &mut l, &mut r);
    (l, r)
}

fn face(i: usize) -> Face {
    let (l, r) = render(i);
    face_from(MODES[i].name, i, &l, &r)
}

fn face_from(name: &'static str, i: usize, l: &[f32], r: &[f32]) -> Face {
    let mono: Vec<f32> = l.iter().zip(r.iter()).map(|(a, b)| 0.5 * (a + b)).collect();

    let mut t60 = [0.0f32; 6];
    for (k, f) in BANDS.iter().enumerate() {
        // A mode with no tail reads as very short rather than as missing.
        t60[k] = t60_in_band(&mono, FS, *f).unwrap_or(0.01).max(0.01).log2();
    }

    // Brightness, from the energy in each band over the whole tail.
    let mut num = 0.0f32;
    let mut den = 0.0f32;
    for f in BANDS.iter() {
        let e = band_energy(&mono, *f);
        num += e * f.log2();
        den += e;
    }
    let centroid = if den > 0.0 { num / den } else { 10.0 };

    // Build-up: where the envelope peaks, in milliseconds.
    let env = envelope(&mono, (FS * 0.005) as usize);
    let peak_at = env
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(k, _)| k)
        .unwrap_or(0);
    let build = ((peak_at as f32 / FS * 1000.0).max(0.5)).log2();

    // Echo density: what fraction of the first 80 ms is above a tenth of its
    // own peak. A few discrete echoes leave most of it empty; a diffuse tank
    // fills it.
    let head = &mono[..(FS * 0.08) as usize];
    let hp = head.iter().fold(0.0f32, |a, b| a.max(b.abs())).max(1e-9);
    let density = head.iter().filter(|v| v.abs() > hp * 0.1).count() as f32 / head.len() as f32;

    // Movement: how much the envelope wobbles about its own decay. A modulated
    // tank breathes; a static one falls smoothly.
    let movement = wobble(&env);

    // Width: one minus the correlation between the two sides.
    let width = 1.0 - correlation(l, r).abs();

    Face {
        name,
        arch: format!("{:?}", MODES[i].arch).to_lowercase(),
        t60,
        centroid,
        build,
        density,
        movement,
        width,
    }
}

fn band_energy(x: &[f32], f: f32) -> f32 {
    // One-pole band-pass, enough to weigh a band against its neighbours.
    let w = (std::f32::consts::TAU * f / FS).tan();
    let (mut lp, mut bp) = (0.0f32, 0.0f32);
    let g = w / (1.0 + w);
    let mut e = 0.0f32;
    for &v in x {
        let hp = v - lp - bp;
        bp += g * hp;
        lp += g * bp;
        e += bp * bp;
    }
    (e / x.len() as f32).sqrt()
}

fn envelope(x: &[f32], win: usize) -> Vec<f32> {
    x.chunks(win.max(1))
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
        .collect()
}

/// How far the envelope departs from a straight line in decibels, which is
/// what a smooth exponential decay would be.
fn wobble(env: &[f32]) -> f32 {
    let db: Vec<f32> = env
        .iter()
        .map(|v| 20.0 * v.max(1e-9).log10())
        .take_while(|v| *v > -80.0)
        .collect();
    if db.len() < 8 {
        return 0.0;
    }
    let n = db.len() as f32;
    let sx: f32 = (0..db.len()).map(|i| i as f32).sum();
    let sy: f32 = db.iter().sum();
    let sxx: f32 = (0..db.len()).map(|i| (i * i) as f32).sum();
    let sxy: f32 = db.iter().enumerate().map(|(i, v)| i as f32 * v).sum();
    let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    let icept = (sy - slope * sx) / n;
    let var: f32 = db
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let d = v - (slope * i as f32 + icept);
            d * d
        })
        .sum::<f32>()
        / n;
    var.sqrt()
}

fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (mut sab, mut saa, mut sbb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        sab += a[i] * b[i];
        saa += a[i] * a[i];
        sbb += b[i] * b[i];
    }
    if saa <= 0.0 || sbb <= 0.0 {
        return 1.0;
    }
    sab / (saa.sqrt() * sbb.sqrt())
}

/// Distance between two modes, in units where 1.0 is "clearly a different
/// reverb". Each term is scaled by how much of it a listener needs to notice.
fn distance(a: &Face, b: &Face) -> f32 {
    let mut d = 0.0f32;
    // A third of an octave of decay time in any band is audible.
    for k in 0..6 {
        d += ((a.t60[k] - b.t60[k]) / 0.33).powi(2);
    }
    // A third of an octave of brightness.
    d += ((a.centroid - b.centroid) / 0.33).powi(2);
    // A factor of two in build-up time.
    d += (a.build - b.build).powi(2);
    // A fifth of the range of density, movement and width.
    d += ((a.density - b.density) / 0.2).powi(2);
    d += ((a.movement - b.movement) / 2.0).powi(2);
    d += ((a.width - b.width) / 0.2).powi(2);
    (d / 11.0).sqrt()
}

/// The same mode with a decay curve on it, to show what the axis is worth.
fn face_with_curve(i: usize, hf_mult: f32) -> Face {
    let (bridge, ix) = noob_reverberator::dsp::build_bridge("probe", FS, true);
    let audio = bridge.take_audio().expect("the audio handle");
    let mut s = read_settings(&audio, &ix);
    Reverb::apply_mode(&mut s, i);
    s.mode = i;
    s.mix = 1.0;
    // One high shelf: the top of the band decays `hf_mult` times as long.
    s.curve.bands[0] = noob_reverberator::dsp::decay::Band {
        on: true,
        shape: noob_reverberator::dsp::decay::Shape::HighShelf,
        freq: 1_200.0,
        mult: hf_mult,
        width: 1.0,
    };
    let mut rev = Reverb::new(FS);
    rev.configure(&s);
    let n = (FS * 4.0) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    l[0] = 1.0;
    r[0] = 1.0;
    rev.process(&s, &mut l, &mut r);
    let mut f = face_from(MODES[i].name, i, &l, &r);
    f.arch = format!("{:?} x{hf_mult}", MODES[i].arch).to_lowercase();
    f
}

fn main() {
    let faces: Vec<Face> = (0..MODES.len()).map(face).collect();

    println!(
        "{:<24} {:<8} {:>8} {:>8} {:>8} {:>7}",
        "mode", "arch", "t60@125", "t60@4k", "tilt oct", "bright"
    );
    for f in &faces {
        println!(
            "{:<24} {:<8} {:>8.2} {:>8.2} {:>8.2} {:>7.0}",
            f.name,
            f.arch,
            f.t60[0].exp2(),
            f.t60[5].exp2(),
            f.t60[0] - f.t60[5],
            f.centroid.exp2()
        );
    }
    let tilts: Vec<f32> = faces.iter().map(|f| f.t60[0] - f.t60[5]).collect();
    let lo = tilts.iter().cloned().fold(f32::MAX, f32::min);
    let hi = tilts.iter().cloned().fold(f32::MIN, f32::max);
    println!(
        "
tilt (low decay against high, in octaves) spans {lo:.2} to {hi:.2}"
    );
    let cs: Vec<f32> = faces.iter().map(|f| f.centroid).collect();
    let clo = cs.iter().cloned().fold(f32::MAX, f32::min);
    let chi = cs.iter().cloned().fold(f32::MIN, f32::max);
    println!(
        "brightness spans {:.0} Hz to {:.0} Hz, which is {:.2} octaves",
        clo.exp2(),
        chi.exp2(),
        chi - clo
    );

    println!(
        "
{:<24} {:<8} {:>7} {:>7} {:>7} {:>7} {:>6} {:>6}",
        "mode", "arch", "t60@1k", "bright", "build", "dens", "move", "width"
    );
    for f in &faces {
        println!(
            "{:<24} {:<8} {:>7.2} {:>7.2} {:>7.1} {:>7.3} {:>6.2} {:>6.2}",
            f.name,
            f.arch,
            f.t60[3].exp2(),
            f.centroid.exp2(),
            f.build.exp2(),
            f.density,
            f.movement,
            f.width
        );
    }

    // Every pair, closest first.
    let mut pairs = Vec::new();
    for i in 0..faces.len() {
        for j in (i + 1)..faces.len() {
            pairs.push((distance(&faces[i], &faces[j]), i, j));
        }
    }
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    println!("\nThe twenty closest pairs (under 1.0 is hard to tell apart):");
    for (d, i, j) in pairs.iter().take(20) {
        println!(
            "  {d:>5.2}  {:<24} / {:<24} [{} / {}]",
            faces[*i].name, faces[*j].name, faces[*i].arch, faces[*j].arch
        );
    }

    // How isolated is each mode? Its distance to its nearest neighbour.
    let mut nearest: Vec<(f32, usize, usize)> = (0..faces.len())
        .map(|i| {
            let (d, j) = (0..faces.len())
                .filter(|j| *j != i)
                .map(|j| (distance(&faces[i], &faces[j]), j))
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                .unwrap();
            (d, i, j)
        })
        .collect();
    nearest.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let crowded = nearest.iter().filter(|(d, _, _)| *d < 1.0).count();
    println!(
        "\n{crowded} of {} modes have a neighbour closer than 1.0.",
        faces.len()
    );
    println!("Loneliest modes (the ones that really are their own thing):");
    for (d, i, j) in nearest.iter().rev().take(6) {
        println!(
            "  {d:>5.2}  {:<24} nearest is {}",
            faces[*i].name, faces[*j].name
        );
    }

    // What the untouched axis is worth. Every mode above has a flat decay
    // curve; these are the same hall with the top of the band decaying half
    // and a fifth as long, which is what a real hall does.
    println!(
        "
The same Concert Hall, with a decay curve on it:"
    );
    let flat = &faces[0];
    for mult in [0.5f32, 0.2] {
        let c = face_with_curve(0, mult);
        println!(
            "  highs x{mult}: t60 {:.2} s at 125 Hz, {:.2} s at 4 kHz, tilt {:.2} oct,              distance from flat {:.2}",
            c.t60[0].exp2(),
            c.t60[5].exp2(),
            c.t60[0] - c.t60[5],
            distance(flat, &c)
        );
    }
}
