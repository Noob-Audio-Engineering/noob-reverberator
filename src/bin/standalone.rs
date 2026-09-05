//! Noob Reverberator without a DAW: a fake audio thread runs a demo source
//! through the reverb at 48 kHz / 256 samples, publishes the meter and the two
//! decay curves, and serves the page from `web/dist`.
//!
//! ```text
//! cargo run --bin noob-reverberator-standalone -- [--port N] [--open] [--dir path]
//! ```
//!
//! | flag | meaning |
//! |---|---|
//! | `--port N` | insist on port `N` |
//! | `--open` | open the page in the system browser |
//! | `--dir path` | serve this directory instead of `web/dist` |

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use noob_reverberator::dsp::{self, engine::Reverb};
use noob_vst_webgui_framework::{AudioHandle, FileStore, ServerConfig};

const SR: f32 = 48_000.0;
const BLOCK: usize = 256;

/// The demo source: a click every so often, and a little noise under it, so
/// both the transient behaviour and the tail are audible without a DAW.
struct Source {
    n: u64,
    rng: u32,
}

impl Source {
    fn next(&mut self) -> f32 {
        self.n += 1;
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let noise = (self.rng >> 8) as f32 / 8_388_608.0 - 1.0;
        // A click twice a second, which is what a reverb is for.
        let period = (SR * 0.5) as u64;
        let t = self.n % period;
        let click = if t < 24 {
            (1.0 - t as f32 / 24.0).powi(2)
        } else {
            0.0
        };
        click * 0.8 + noise * 0.004
    }
}

/// The streams are addressed by index, in the order `dsp::streams` lists them.
const S_METER: usize = 0;
const S_ASKED: usize = 1;
const S_REALISED: usize = 2;

fn audio_thread(mut audio: AudioHandle, ix: dsp::engine::ParamIx, blocks: Arc<AtomicU64>) {
    let mut rev = Reverb::new(SR);
    let mut src = Source {
        n: 0,
        rng: 0x1234_5678,
    };
    let mut l = vec![0.0f32; BLOCK];
    let mut r = vec![0.0f32; BLOCK];
    let mut asked = vec![0.0f32; dsp::CURVE_POINTS];
    let mut realised = vec![0.0f32; dsp::CURVE_POINTS];
    let block_dur = Duration::from_secs_f64(BLOCK as f64 / SR as f64);
    let mut next = Instant::now();
    let mut n: u64 = 0;

    loop {
        let settings = dsp::engine::read_settings(&audio, &ix);
        rev.configure(&settings);

        let mut peak_in = 0.0f32;
        for i in 0..BLOCK {
            let x = src.next();
            l[i] = x;
            r[i] = x;
            peak_in = peak_in.max(x.abs());
        }
        rev.process(&settings, &mut l, &mut r);

        let mut peak_out = 0.0f32;
        for i in 0..BLOCK {
            peak_out = peak_out.max(l[i].abs()).max(r[i].abs());
        }
        audio.publish_slice(S_METER, &[peak_in, peak_in, peak_out, peak_out]);

        // The curves, a few times a second: they only move when a control
        // does, and publishing them every block would be a hundred and
        // eighty-seven redraws a second of a line that has not changed.
        if n.is_multiple_of(8) {
            fill_curves(&settings, &rev, &mut asked, &mut realised);
            audio.publish_slice(S_ASKED, &asked);
            audio.publish_slice(S_REALISED, &realised);
        }

        n += 1;
        blocks.store(n, Ordering::Relaxed);
        next += block_dur;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}

/// Both curves, on the same log-frequency grid the page draws on.
fn fill_curves(s: &dsp::engine::Settings, rev: &Reverb, asked: &mut [f32], realised: &mut [f32]) {
    let lo = dsp::loss::FIT_BOTTOM;
    let hi = dsp::loss::FIT_TOP;
    for i in 0..dsp::CURVE_POINTS {
        let t = i as f32 / (dsp::CURVE_POINTS - 1) as f32;
        let f = lo * (hi / lo).powf(t);
        asked[i] = s.curve.t60(f);
        realised[i] = rev.realised_t60(f);
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut port: Option<u16> = None;
    let mut open = false;
    let mut dir: Option<PathBuf> = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                port = args.get(i).and_then(|v| v.parse().ok());
            }
            "--open" => open = true,
            "--dir" => {
                i += 1;
                dir = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => {
                println!("noob-reverberator standalone [--port N] [--open] [--dir path]");
                return;
            }
            other => eprintln!("ignoring unknown argument `{other}`"),
        }
        i += 1;
    }

    println!(
        "noob-reverberator standalone {} — starting",
        env!("CARGO_PKG_VERSION")
    );

    let (bridge, ix) = dsp::build_bridge("Noob Reverberator", SR, true);
    let _store = FileStore::attach(&bridge, FileStore::default_path("noob-reverberator"));

    let audio = bridge.take_audio().expect("the audio handle is taken once");
    let blocks = Arc::new(AtomicU64::new(0));
    {
        let blocks = blocks.clone();
        thread::Builder::new()
            .name("fake-audio".into())
            .spawn(move || audio_thread(audio, ix, blocks))
            .expect("the audio thread would not start");
    }

    let dir = dir.unwrap_or_else(|| PathBuf::from("web/dist"));
    let cfg = match port {
        Some(p) => ServerConfig::default().port(p),
        None => ServerConfig::default().prefer_port(4251),
    };
    let server = match noob_vst_webgui_framework::serve(&bridge, cfg.assets_dir(&dir)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("the server would not start: {e}");
            std::process::exit(1);
        }
    };
    let url = format!("http://{}/", server.addr());
    println!("serving on {url}");
    if open
        && let Err(e) = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
    {
        eprintln!("could not open a browser: {e}");
    }

    loop {
        thread::sleep(Duration::from_secs(1));
        println!(
            "clients {}  blocks {}",
            server.client_count(),
            blocks.load(Ordering::Relaxed)
        );
    }
}
