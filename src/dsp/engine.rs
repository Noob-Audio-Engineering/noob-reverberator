//! The reverberator: everything wired together, and the settings that drive it.

use noob_vst_webgui_framework::{AudioHandle, NoobVstWebguiFramework};

use super::bloom::Bloom;
use super::colour::{Colour, Era};
use super::decay::{Band, Curve, MAX_BANDS, Shape};
use super::delay::Delay;
use super::diffuse::Diffuser;
use super::duck::{AutoGate, Ducker};
use super::early::Early;
use super::fdn::{Fdn, MAX_LINES, prime_lengths};
use super::formant::Choir;
use super::mode::{Arch, MODES};
use super::modulate::Modulator;
use super::pitch::Shifter;
use super::plate::Plate;
use super::shape::{Kind as ShapeKind, Shaper};
use super::spring::Spring;
use super::tape::Tape;

/// Everything the audio thread needs, read once per block.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub bypass: bool,
    pub mix: f32,
    pub output: f32,
    pub freeze: bool,
    pub mode: usize,
    pub decay: f32,
    pub size: f32,
    pub density: f32,
    pub attack_ms: f32,
    pub predelay_ms: f32,
    pub width: f32,
    pub mod_rate: f32,
    pub mod_depth: f32,
    pub mod_random: f32,
    pub early_level: f32,
    pub early_size: f32,
    pub early_absorb: f32,
    pub early_taps: usize,
    pub shift: f32,
    pub shift_mix: f32,
    pub shift_window: f32,
    pub shape: ShapeKind,
    pub shape_hold: f32,
    pub era: Era,
    pub era_amount: f32,
    pub tension: f32,
    pub sections: usize,
    pub lines: usize,
    pub distance: f32,
    pub thickness: f32,
    pub duck: f32,
    pub duck_release: f32,
    pub gate: f32,
    pub gate_hold: f32,
    pub bloom: f32,
    pub bloom_time: f32,
    pub bloom_swell: f32,
    pub tape_mix: f32,
    pub tape_time: f32,
    pub tape_heads: usize,
    pub tape_feedback: f32,
    pub tape_wobble: f32,
    pub tape_drive: f32,
    pub choir_amount: f32,
    pub choir_vowel: f32,
    pub choir_spread: f32,
    pub choir_resonance: f32,
    pub curve: Curve,
}

/// Where each parameter sits, looked up once so the audio thread never
/// searches by name.
pub struct ParamIx {
    pub bypass: usize,
    pub mix: usize,
    pub output: usize,
    pub freeze: usize,
    pub mode: usize,
    pub decay: usize,
    pub size: usize,
    pub density: usize,
    pub attack: usize,
    pub predelay: usize,
    pub width: usize,
    pub mod_rate: usize,
    pub mod_depth: usize,
    pub mod_random: usize,
    pub early_level: usize,
    pub early_size: usize,
    pub early_absorb: usize,
    pub early_taps: usize,
    pub shift: usize,
    pub shift_mix: usize,
    pub shift_window: usize,
    pub shape: usize,
    pub shape_hold: usize,
    pub era: usize,
    pub era_amount: usize,
    pub tension: usize,
    pub sections: usize,
    pub lines: usize,
    pub distance: usize,
    pub thickness: usize,
    pub duck: usize,
    pub duck_release: usize,
    pub gate: usize,
    pub gate_hold: usize,
    pub bloom: usize,
    pub bloom_time: usize,
    pub bloom_swell: usize,
    pub tape_mix: usize,
    pub tape_time: usize,
    pub tape_heads: usize,
    pub tape_feedback: usize,
    pub tape_wobble: usize,
    pub tape_drive: usize,
    pub choir_amount: usize,
    pub choir_vowel: usize,
    pub choir_spread: usize,
    pub choir_resonance: usize,
    pub bands: [[usize; 5]; MAX_BANDS],
}

impl ParamIx {
    /// Resolve every id once.
    ///
    /// A missing id is a bug in [`super::params::param_specs`] rather than
    /// something to survive at runtime, so this says which one.
    pub fn new(bridge: &NoobVstWebguiFramework) -> ParamIx {
        let ix = |id: &str| {
            bridge
                .index_of(id)
                .unwrap_or_else(|| panic!("parameter `{id}` is missing from the spec list"))
        };
        let mut bands = [[0usize; 5]; MAX_BANDS];
        for (i, b) in bands.iter_mut().enumerate() {
            let n = i + 1;
            *b = [
                ix(&format!("band{n}_on")),
                ix(&format!("band{n}_shape")),
                ix(&format!("band{n}_freq")),
                ix(&format!("band{n}_mult")),
                ix(&format!("band{n}_width")),
            ];
        }
        ParamIx {
            bypass: ix("bypass"),
            mix: ix("mix"),
            output: ix("output"),
            freeze: ix("freeze"),
            mode: ix("mode"),
            decay: ix("decay"),
            size: ix("size"),
            density: ix("density"),
            attack: ix("attack"),
            predelay: ix("predelay"),
            width: ix("width"),
            mod_rate: ix("mod_rate"),
            mod_depth: ix("mod_depth"),
            mod_random: ix("mod_random"),
            early_level: ix("early_level"),
            early_size: ix("early_size"),
            early_absorb: ix("early_absorb"),
            early_taps: ix("early_taps"),
            shift: ix("shift"),
            shift_mix: ix("shift_mix"),
            shift_window: ix("shift_window"),
            shape: ix("shape"),
            shape_hold: ix("shape_hold"),
            era: ix("era"),
            era_amount: ix("era_amount"),
            tension: ix("tension"),
            sections: ix("sections"),
            lines: ix("lines"),
            distance: ix("distance"),
            thickness: ix("thickness"),
            duck: ix("duck"),
            duck_release: ix("duck_release"),
            gate: ix("gate"),
            gate_hold: ix("gate_hold"),
            bloom: ix("bloom"),
            bloom_time: ix("bloom_time"),
            bloom_swell: ix("bloom_swell"),
            tape_mix: ix("tape_mix"),
            tape_time: ix("tape_time"),
            tape_heads: ix("tape_heads"),
            tape_feedback: ix("tape_feedback"),
            tape_wobble: ix("tape_wobble"),
            tape_drive: ix("tape_drive"),
            choir_amount: ix("choir_amount"),
            choir_vowel: ix("choir_vowel"),
            choir_spread: ix("choir_spread"),
            choir_resonance: ix("choir_resonance"),
            bands,
        }
    }
}

/// Read every parameter into a [`Settings`].
pub fn read_settings(audio: &AudioHandle, ix: &ParamIx) -> Settings {
    let p = |i: usize| audio.param(i);
    let mut curve = Curve {
        base: p(ix.decay),
        bands: [Band::default(); MAX_BANDS],
    };
    for (i, b) in ix.bands.iter().enumerate() {
        curve.bands[i] = Band {
            on: p(b[0]) >= 0.5,
            // The crate owns the order, so a shape added there reaches the
            // engine without this having to be edited to agree with it.
            shape: Shape::from_index(p(b[1]).round().max(0.0) as usize),
            freq: p(b[2]),
            mult: p(b[3]),
            width: p(b[4]),
        };
    }
    Settings {
        bypass: p(ix.bypass) >= 0.5,
        mix: p(ix.mix) / 100.0,
        output: 10f32.powf(p(ix.output) / 20.0),
        freeze: p(ix.freeze) >= 0.5,
        mode: p(ix.mode).round() as usize,
        decay: p(ix.decay),
        size: p(ix.size) / 100.0,
        density: p(ix.density) / 100.0,
        attack_ms: p(ix.attack),
        predelay_ms: p(ix.predelay),
        width: p(ix.width) / 100.0,
        mod_rate: p(ix.mod_rate),
        mod_depth: p(ix.mod_depth) / 100.0,
        mod_random: p(ix.mod_random) / 100.0,
        early_level: p(ix.early_level) / 100.0,
        early_size: p(ix.early_size),
        early_absorb: p(ix.early_absorb) / 100.0,
        early_taps: p(ix.early_taps).round().max(1.0) as usize,
        shift: p(ix.shift),
        shift_mix: p(ix.shift_mix) / 100.0,
        shift_window: p(ix.shift_window),
        shape: ShapeKind::from_index(p(ix.shape).round() as usize),
        shape_hold: p(ix.shape_hold),
        era: Era::from_index(p(ix.era).round() as usize),
        era_amount: p(ix.era_amount) / 100.0,
        tension: p(ix.tension) / 100.0,
        sections: p(ix.sections).round().max(1.0) as usize,
        lines: p(ix.lines).round().clamp(4.0, 16.0) as usize,
        distance: p(ix.distance) / 100.0,
        thickness: p(ix.thickness) / 100.0,
        duck: p(ix.duck) / 100.0,
        duck_release: p(ix.duck_release),
        gate: p(ix.gate) / 100.0,
        gate_hold: p(ix.gate_hold),
        bloom: p(ix.bloom) / 100.0,
        bloom_time: p(ix.bloom_time),
        bloom_swell: p(ix.bloom_swell),
        tape_mix: p(ix.tape_mix) / 100.0,
        tape_time: p(ix.tape_time),
        tape_heads: p(ix.tape_heads).round().max(1.0) as usize,
        tape_feedback: p(ix.tape_feedback) / 100.0,
        tape_wobble: p(ix.tape_wobble) / 100.0,
        tape_drive: p(ix.tape_drive) / 100.0,
        choir_amount: p(ix.choir_amount) / 100.0,
        choir_vowel: p(ix.choir_vowel),
        choir_spread: p(ix.choir_spread) / 100.0,
        choir_resonance: p(ix.choir_resonance) / 100.0,
        curve,
    }
}

/// The reverberator.
///
/// One of these per instance. It holds every architecture at once rather than
/// building one when a mode is chosen: allocating in the audio thread is not
/// allowed, and a mode change that had to allocate would be a mode change that
/// dropped out.
pub struct Reverb {
    fs: f32,
    arch: Arch,
    net: Fdn,
    plate: Plate,
    spring: Spring,
    early: Early,
    diff: [Diffuser; 2],
    shift: [Shifter; 2],
    shaper: Shaper,
    colour: Colour,
    tape: Tape,
    choir: Choir,
    bloom: Bloom,
    ducker: Ducker,
    gate: AutoGate,
    /// How much saturation the tank is being driven with, from Thickness.
    drive: f32,
    pre: [Delay; 2],
    mods: Vec<Modulator>,
    lens: Vec<f32>,
    inject: Vec<f32>,
    taps: Vec<f32>,
    /// What the tank was last built for, so a block that changes nothing does
    /// no work.
    built: Option<Settings>,
    pre_len: f32,
    attack_gain: f32,
    attack_coef: f32,
}

/// How long a pre-delay may be, in milliseconds. Allocated for once.
const MAX_PREDELAY_MS: f32 = 500.0;

impl Reverb {
    pub fn new(fs: f32) -> Self {
        // Sized for the longest tank the size control can ask for. Allocating
        // for the worst case once is what lets every control move afterwards
        // without allocating again.
        let longest = (fs * 0.6) as usize;
        Reverb {
            fs,
            arch: Arch::Network,
            net: Fdn::new(MAX_LINES, longest, fs),
            plate: Plate::new(fs),
            spring: Spring::new(fs, 120.0),
            early: Early::new(fs, 400.0),
            diff: [Diffuser::new(6, 2_048, 1), Diffuser::new(6, 2_048, 2)],
            shift: [Shifter::new(32_768), Shifter::new(32_768)],
            shaper: Shaper::new(fs),
            colour: Colour::new(fs),
            tape: Tape::new(fs, 2_200.0),
            choir: Choir::new(fs),
            bloom: Bloom::new(fs, 800.0),
            ducker: Ducker::new(fs),
            gate: AutoGate::new(fs),
            drive: 0.0,
            pre: [
                Delay::with_capacity((fs * MAX_PREDELAY_MS / 1000.0) as usize + 64),
                Delay::with_capacity((fs * MAX_PREDELAY_MS / 1000.0) as usize + 64),
            ],
            mods: (0..MAX_LINES)
                .map(|i| Modulator::new(0x9E37_79B9u32.wrapping_mul(i as u32 + 1)))
                .collect(),
            lens: vec![0.0; MAX_LINES],
            inject: vec![0.0; MAX_LINES],
            taps: vec![0.0; MAX_LINES],
            built: None,
            pre_len: 0.0,
            attack_gain: 1.0,
            attack_coef: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.net.clear();
        self.plate.clear();
        self.spring.clear();
        self.early.clear();
        for d in &mut self.diff {
            d.clear();
        }
        for s in &mut self.shift {
            s.clear();
        }
        self.shaper.reset();
        self.colour.reset();
        self.tape.clear();
        self.choir.clear();
        self.bloom.clear();
        self.ducker.reset();
        self.gate.reset();
        for p in &mut self.pre {
            p.clear();
        }
        self.attack_gain = 1.0;
    }

    /// Put a mode's settings on top of a [`Settings`]. Everything stays
    /// movable afterwards, which is the whole point of the mode list.
    pub fn apply_mode(s: &mut Settings, i: usize) {
        let m = &MODES[i.min(MODES.len() - 1)];
        s.decay = m.decay;
        s.size = m.size;
        s.density = m.density;
        s.attack_ms = m.attack;
        s.mod_rate = m.mod_rate;
        s.mod_depth = m.mod_depth / 20.0;
        s.mod_random = m.mod_random;
        s.early_level = m.early;
        s.shift = m.shift;
        s.shift_mix = m.shift_mix;
        s.shape = m.shape;
        s.era = Era::from_index(m.era);
        s.era_amount = m.era_amount;
        s.lines = m.lines;
        s.tape_mix = m.tape;
        s.choir_amount = m.choir;
        s.choir_vowel = m.vowel;
        s.distance = m.distance;
        s.thickness = m.thickness;
        s.bloom = m.bloom;
        s.curve.base = m.decay;
    }
}

impl Reverb {
    /// Rebuild whatever the settings changed. Called once a block, never per
    /// sample, and it does no work when nothing moved.
    pub fn configure(&mut self, s: &Settings) {
        if let Some(old) = self.built.as_ref()
            && same(old, s)
        {
            return;
        }
        self.arch = MODES[s.mode.min(MODES.len() - 1)].arch;
        let size = s.size.clamp(0.05, 4.0);

        match self.arch {
            Arch::Network => {
                let n = s.lines.clamp(4, MAX_LINES);
                // A network's shortest line sets the lowest frequency it holds
                // a mode at; its longest sets how sparse the early sound is.
                // Both scale with size, which is the only thing size can
                // honestly mean for a network of delay lines.
                let shortest = self.fs * 0.013 * size;
                let longest = self.fs * 0.075 * size;
                prime_lengths(n, shortest, longest, &mut self.lens);
                if self.net.lines() != n {
                    self.net = Fdn::new(n, (self.fs * 0.6) as usize, self.fs);
                }
                self.net.set_lengths(&self.lens[..n], &s.curve);
            }
            Arch::Plate => self.plate.set(size, s.density, &s.curve),
            Arch::Spring => self
                .spring
                .set(s.tension, s.sections, 30.0 * size, &s.curve),
            Arch::Early => {}
        }

        self.plate
            .set_modulation(s.mod_rate, s.mod_depth * 40.0, s.mod_random);
        for m in &mut self.mods {
            m.set_rate(self.fs, s.mod_rate);
        }
        // Distance is a macro, and honest about it: walking away from a
        // source means less of the early pattern, more diffusion by the time
        // it arrives, a longer build-up and a little less top from the air in
        // between. Every one of those is a control that already exists, so
        // this moves them rather than adding a fifth thing that does the same
        // job differently. Pro-R's own description is the specification: "a
        // longer build-up and a more diffuse tail".
        let far = s.distance.clamp(0.0, 1.0);
        let density = (s.density + far * (1.0 - s.density) * 0.7).clamp(0.0, 1.0);
        for d in &mut self.diff {
            d.set_scale(size * (1.0 + far * 0.5), 900.0);
            d.set_amount(density);
            d.set_modulation(self.fs, s.mod_rate * 0.7, s.mod_depth * 12.0, s.mod_random);
        }
        for sh in &mut self.shift {
            sh.set_window(s.shift_window);
            sh.set_semitones(s.shift);
        }
        self.early
            .build(s.early_size, s.early_absorb, s.early_taps, s.width);
        self.shaper.set(s.shape, s.shape_hold);
        self.colour.set(s.era, s.era_amount, self.fs);
        self.tape.set(
            s.tape_time,
            s.tape_heads,
            s.tape_feedback,
            s.tape_wobble,
            s.tape_drive,
        );
        self.choir.set(
            s.choir_vowel,
            s.choir_spread,
            s.choir_amount,
            s.choir_resonance,
        );
        self.pre_len = (s.predelay_ms.clamp(0.0, MAX_PREDELAY_MS) * self.fs / 1000.0).max(1.0);
        // A one-pole rise on the wet path. This is not a mode's build-up ---
        // that is a property of the line lengths --- it is the control
        // somebody reaches for when they want the reverb to arrive late, and
        // it is honest about being a fade.
        // Distance lengthens the build-up, which is the attack fade.
        let attack_ms = s.attack_ms + far * 220.0;
        self.attack_coef = if attack_ms > 0.5 {
            (-1.0f32 / (attack_ms * 0.001 * self.fs)).exp()
        } else {
            0.0
        };
        self.built = Some(*s);
    }

    /// The decay the running tank will actually produce at `f`, in seconds.
    /// Published so the panel can draw what the machine is doing beside what
    /// was asked for.
    pub fn realised_t60(&self, f: f32) -> f32 {
        match self.arch {
            Arch::Network => self.net.t60_at(f),
            _ => f32::NAN,
        }
    }

    /// One block, in place.
    pub fn process(&mut self, s: &Settings, l: &mut [f32], r: &mut [f32]) {
        if s.bypass {
            return;
        }
        let n = l.len().min(r.len());
        let lines = self.net.lines();
        for i in 0..n {
            let (dry_l, dry_r) = (l[i], r[i]);

            // Pre-delay first: it is the gap before the room answers, so
            // everything after it is the room.
            self.pre[0].push(dry_l);
            self.pre[1].push(dry_r);
            let (pl, pr) = (
                self.pre[0].read(self.pre_len),
                self.pre[1].read(self.pre_len),
            );

            // Bloom regenerates before the tank hears anything, so the tank's
            // input grows rather than the tail being faded in. That is why it
            // is here and not on the wet path with the attack.
            let (pl, pr) = if s.bloom > 0.0 {
                (self.bloom.process(pl), pr)
            } else {
                (pl, pr)
            };

            // The tape sits between the pre-delay and the tank, which is what
            // makes it a Magneto rather than an echo after a reverb: its
            // repeats are what the room hears, so each one gets its own tail.
            let (pl, pr) = if s.tape_mix > 0.0 {
                let dry = 0.5 * (pl + pr);
                let wet = self.tape.process(dry);
                let m = s.tape_mix;
                (pl * (1.0 - m) + wet * m, pr * (1.0 - m) + wet * m)
            } else {
                (pl, pr)
            };

            let (el, er) = self.early.process(0.5 * (pl + pr));

            // Frozen: the tank is sealed and hears nothing new. Its own decay
            // still applies, so this is a very long tail rather than a hold,
            // and the panel says which.
            let feed = if s.freeze { 0.0 } else { 1.0 };
            // Thickness's saturation, on the way in. `tanh` is bounded, so
            // driving it harder makes the tank denser and cannot make it
            // louder without limit --- which matters when the thing being
            // driven is a feedback loop.
            let sat = |v: f32| {
                if self.drive > 0.0 {
                    let k = 1.0 + self.drive * 5.0;
                    (v * k).tanh() / k.tanh()
                } else {
                    v
                }
            };
            let (pl, pr) = (sat(pl), sat(pr));
            let (mut wl, mut wr) = match self.arch {
                Arch::Network => {
                    let a = self.diff[0].process(pl * feed);
                    let b = self.diff[1].process(pr * feed);
                    for k in 0..lines {
                        // Alternating signs, so an impulse does not arrive in
                        // phase on every line and cancel through the mix.
                        let src = if k % 2 == 0 { a } else { b };
                        self.inject[k] = if k % 4 < 2 { src } else { -src };
                        let d = s.mod_depth * 40.0;
                        let off = self.mods[k].next(d, s.mod_random);
                        self.net.modulate(k, off);
                    }
                    self.net
                        .process(&self.inject[..lines], &mut self.taps[..lines]);
                    let mut a = 0.0;
                    let mut b = 0.0;
                    for (k, &v) in self.taps[..lines].iter().enumerate() {
                        if k % 2 == 0 {
                            a += v;
                        } else {
                            b += v;
                        }
                    }
                    let k = 2.0 / lines as f32;
                    (a * k, b * k)
                }
                Arch::Plate => self.plate.process(0.5 * (pl + pr) * feed),
                Arch::Spring => {
                    let v = self.spring.process(0.5 * (pl + pr) * feed);
                    (v, v)
                }
                Arch::Early => (0.0, 0.0),
            };

            // The transposed copy goes back in with the tail rather than
            // replacing it, so a shimmer still has a reverb under it.
            if s.shift_mix > 0.0 {
                let a = self.shift[0].process(wl);
                let b = self.shift[1].process(wr);
                wl += a * s.shift_mix;
                wr += b * s.shift_mix;
            }

            // The choir goes on the tail and not on the input: a vowel is
            // made of resonances at fixed frequencies, and it is the dense
            // late energy that has something at all of them for those
            // resonances to find.
            let (wl, wr) = self.choir.process(wl, wr);

            let (cl, cr) = self.colour.process(wl, wr);
            let mut ol = cl + el * s.early_level;
            let mut or = cr + er * s.early_level;

            // The attack fade, and then the shape.
            if self.attack_coef > 0.0 {
                let target = 1.0;
                self.attack_gain = target + (self.attack_gain - target) * self.attack_coef;
                ol *= self.attack_gain;
                or *= self.attack_gain;
            }
            let dry_mono = 0.5 * (dry_l + dry_r);
            let g =
                self.shaper.next(dry_mono) * self.ducker.next(dry_mono) * self.gate.next(dry_mono);
            ol *= g;
            or *= g;

            // Width, as a mid/side scale. At zero the two sides are the same
            // reverb, at two hundred per cent the sides are doubled.
            let mid = 0.5 * (ol + or);
            let side = 0.5 * (ol - or) * s.width;
            ol = mid + side;
            or = mid - side;

            let wet = s.mix * s.output;
            let dry = 1.0 - s.mix;
            l[i] = dry_l * dry + ol * wet;
            r[i] = dry_r * dry + or * wet;
        }
    }
}

/// Whether two settings ask for the same tank, so a block can skip the
/// rebuild.
///
/// Compared field by field rather than by deriving equality, on purpose: `mix`
/// and `output` change every time somebody touches a fader and change nothing
/// about the tank. Refitting sixteen loss filters because a fader moved would
/// be a real cost for no reason, and it is the kind of cost that only shows up
/// on the machine of whoever is automating the control.
fn same(a: &Settings, b: &Settings) -> bool {
    a.mode == b.mode
        && a.decay == b.decay
        && a.size == b.size
        && a.density == b.density
        && a.attack_ms == b.attack_ms
        && a.predelay_ms == b.predelay_ms
        && a.width == b.width
        && a.mod_rate == b.mod_rate
        && a.mod_depth == b.mod_depth
        && a.mod_random == b.mod_random
        && a.early_size == b.early_size
        && a.early_absorb == b.early_absorb
        && a.early_taps == b.early_taps
        && a.shift == b.shift
        && a.shift_window == b.shift_window
        && a.shape == b.shape
        && a.shape_hold == b.shape_hold
        && a.era == b.era
        && a.era_amount == b.era_amount
        && a.tension == b.tension
        && a.sections == b.sections
        && a.lines == b.lines
        && a.distance == b.distance
        && a.thickness == b.thickness
        && a.duck == b.duck
        && a.duck_release == b.duck_release
        && a.gate == b.gate
        && a.gate_hold == b.gate_hold
        && a.bloom == b.bloom
        && a.bloom_time == b.bloom_time
        && a.bloom_swell == b.bloom_swell
        && a.tape_time == b.tape_time
        && a.tape_heads == b.tape_heads
        && a.tape_feedback == b.tape_feedback
        && a.tape_wobble == b.tape_wobble
        && a.tape_drive == b.tape_drive
        && a.choir_vowel == b.choir_vowel
        && a.choir_spread == b.choir_spread
        && a.choir_amount == b.choir_amount
        && a.choir_resonance == b.choir_resonance
        && curves_match(&a.curve, &b.curve)
}

fn curves_match(a: &Curve, b: &Curve) -> bool {
    if a.base != b.base {
        return false;
    }
    a.bands.iter().zip(b.bands.iter()).all(|(x, y)| {
        x.on == y.on
            && x.shape == y.shape
            && x.freq == y.freq
            && x.mult == y.mult
            && x.width == y.width
    })
}
