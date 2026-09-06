//! Render every mode to a WAV file, so the one check I cannot do is quick.
//!
//!     cargo run --release --features plugin --example render_modes -- out
//!
//! Everything else about these thirty-two modes is measured: each realises its
//! own decay curve at five frequencies, in the right direction and magnitude,
//! and no two of them produce the same audio. What no measurement here settles
//! is whether the curves I chose are the *right* curves --- whether a hall
//! that loses 1.15 octaves off its top sounds like a hall. That needs ears,
//! and I have none.
//!
//! So this writes the files to listen to. Two per mode: the impulse response,
//! which shows the tail's colour and length plainly, and a short musical
//! source through it, which is how anybody actually judges a reverb. Wet only
//! --- the dry would mask exactly what is being judged.
//!
//! Compare `01-concert-hall-music.wav` against `05-plate-music.wav` against
//! `13-undertow-music.wav`: those three should be obviously different rooms,
//! and if any of them is wrong it is one line in `src/dsp/mode.rs`.
use noob_reverberator::dsp::engine::{Reverb, read_settings};
use noob_reverberator::dsp::mode::MODES;
use std::io::Write;

const FS: f32 = 48_000.0;

/// A source with a transient and a tone: a reverb is judged on what it does to
/// both, and an impulse alone flatters a tail that would smear a chord.
fn source(n: usize) -> Vec<f32> {
    let mut x = vec![0.0f32; n];
    // Three clicks, so the early reflections and the build-up are audible.
    for k in 0..3 {
        let at = (FS * (0.02 + 0.42 * k as f32)) as usize;
        for i in 0..48 {
            if at + i < n {
                let e = 1.0 - i as f32 / 48.0;
                x[at + i] += e * e * if i % 2 == 0 { 0.9 } else { -0.9 };
            }
        }
    }
    // A short chord, so the tail has something to hold and colour.
    let start = (FS * 1.35) as usize;
    let len = (FS * 0.45) as usize;
    for i in 0..len {
        if start + i >= n {
            break;
        }
        let t = i as f32 / FS;
        let e = (-(t * 6.0)).exp() * (1.0 - (-(t * 400.0)).exp());
        let mut v = 0.0;
        for f in [220.0f32, 277.2, 329.6, 440.0] {
            v += (std::f32::consts::TAU * f * t).sin();
        }
        x[start + i] += 0.16 * e * v;
    }
    x
}

fn write_wav(path: &std::path::Path, l: &[f32], r: &[f32]) -> std::io::Result<()> {
    let n = l.len().min(r.len());
    let bytes = (n * 2 * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + bytes).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&2u16.to_le_bytes())?; // stereo
    f.write_all(&(FS as u32).to_le_bytes())?;
    f.write_all(&((FS as u32) * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&bytes.to_le_bytes())?;
    for i in 0..n {
        for v in [l[i], r[i]] {
            let s = (v.clamp(-1.0, 1.0) * 32_767.0) as i16;
            f.write_all(&s.to_le_bytes())?;
        }
    }
    f.flush()
}

fn slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn main() -> std::io::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "out".into()));
    std::fs::create_dir_all(&dir)?;

    for (i, m) in MODES.iter().enumerate() {
        let (bridge, ix) = noob_reverberator::dsp::build_bridge("render", FS, true);
        // Written the way the mode strip writes it, so these are the modes as
        // the plug-in makes them and not as a test imagines them.
        for (id, v) in noob_reverberator::dsp::mode::param_set(i) {
            if let Some(at) = bridge.index_of(&id) {
                bridge.set_param(at, v);
            }
        }
        let audio = bridge.take_audio().expect("the audio handle");
        let mut s = read_settings(&audio, &ix);
        s.mode = i;
        // Wet only: the dry would mask the thing being judged.
        s.mix = 1.0;

        let tail = (s.curve.t60(1_000.0) * 1.6).clamp(2.5, 14.0);
        let n = (FS * (2.0 + tail)) as usize;

        for (kind, mut l) in [
            ("ir", {
                let mut v = vec![0.0f32; n];
                v[0] = 1.0;
                v
            }),
            ("music", source(n)),
        ] {
            let mut r = l.clone();
            let mut rev = Reverb::new(FS);
            rev.configure(&s);
            rev.process(&s, &mut l, &mut r);
            // Normalised, because a mode that is quieter is not a mode that is
            // worse and a listener comparing thirty-two files should not be
            // comparing their levels.
            let peak = l
                .iter()
                .chain(r.iter())
                .fold(0.0f32, |a, b| a.max(b.abs()))
                .max(1e-9);
            let g = 0.89 / peak;
            for v in l.iter_mut().chain(r.iter_mut()) {
                *v *= g;
            }
            let path = dir.join(format!("{:02}-{}-{kind}.wav", i + 1, slug(m.name)));
            write_wav(&path, &l, &r)?;
        }
        println!("{:02} {:<24} {}", i + 1, m.name, m.blurb);
    }
    println!(
        "\n{} modes, two files each, in {}",
        MODES.len(),
        dir.display()
    );
    Ok(())
}
