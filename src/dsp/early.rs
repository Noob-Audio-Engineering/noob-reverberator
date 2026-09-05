//! Early reflections: the first few dozen echoes, which are what say where
//! the walls are.
//!
//! The late tail says how big and how absorbent a space is. The **early**
//! reflections say its shape, and they are what a listener localises on: the
//! delay to the first wall, the pattern of the next few, and how quickly the
//! pattern stops being countable. Strymon's Reflections machine is described
//! as "psycho-acoustically accurate small-space reverb calculating 250 room
//! reflections", and Valhalla's Room notes say the plug-in "generates early
//! and late acoustic energy that provides the spatial and phase cues needed to
//! create an idealized room impression" rather than modelling a real room.
//!
//! This is the second of those. It is a tapped delay line whose taps come from
//! the image-source pattern of a shoebox --- the arrival times a rectangular
//! room really has --- but with the levels and the stereo placement chosen to
//! give the cues rather than to be right about any particular room. A real
//! room's early pattern is a property of where you stood in it, and a plug-in
//! has no way to know that.
//!
//! # Why the taps are laid out from a box and not from noise
//!
//! Random taps sound like a room until the room gets small, and then they
//! sound like a broken tape. A box's arrivals cluster: the first order gives
//! six, the second gives eighteen, and the gaps between clusters shrink in a
//! way the ear reads as a size. Getting that from noise means getting lucky.

use super::delay::Delay;

/// The most taps one pattern can have.
pub const MAX_TAPS: usize = 64;

#[derive(Debug, Clone, Copy, Default)]
struct Tap {
    /// Where to read, in samples.
    at: f32,
    /// Level into the left and right outputs.
    l: f32,
    r: f32,
}

pub struct Early {
    line: Delay,
    taps: [Tap; MAX_TAPS],
    used: usize,
    fs: f32,
}

impl Early {
    pub fn new(fs: f32, max_ms: f32) -> Self {
        let cap = ((fs * max_ms / 1000.0) as usize + 64).next_power_of_two();
        Early {
            line: Delay::with_capacity(cap),
            taps: [Tap::default(); MAX_TAPS],
            used: 0,
            fs,
        }
    }

    pub fn clear(&mut self) {
        self.line.clear();
    }

    pub fn taps(&self) -> usize {
        self.used
    }

    /// Build a pattern for a shoebox of the given size.
    ///
    /// `size_m` is the room's longest wall in metres, `absorb` how much each
    /// bounce costs (0 is a tiled bathroom, 1 a room full of curtains), and
    /// `count` how many arrivals to keep.
    ///
    /// The proportions are fixed at roughly 1 : 0.75 : 0.55, which are close
    /// to the ratios a room is built with to keep its own modes apart. Making
    /// them a control would be four more numbers to get wrong for something
    /// the ear reads as one thing: size.
    pub fn build(&mut self, size_m: f32, absorb: f32, count: usize, width: f32) {
        const C: f32 = 343.0; // metres per second, dry air at 20 °C
        let lx = size_m.clamp(0.5, 60.0);
        let ly = lx * 0.75;
        let lz = lx * 0.55;
        let absorb = absorb.clamp(0.0, 0.98);
        let count = count.clamp(1, MAX_TAPS);

        // Image sources of a box: a listener slightly off centre, so the
        // arrivals do not come in identical pairs that cancel to mono.
        let (px, py, pz) = (lx * 0.42, ly * 0.51, lz * 0.47);
        let mut found: Vec<(f32, f32, f32)> = Vec::new(); // time, level, pan
        let order = 3i32;
        for ix in -order..=order {
            for iy in -order..=order {
                for iz in -order..=order {
                    let n = ix.abs() + iy.abs() + iz.abs();
                    if n == 0 {
                        continue;
                    }
                    let x = ix as f32 * lx + if ix % 2 == 0 { px } else { lx - px } - px;
                    let y = iy as f32 * ly + if iy % 2 == 0 { py } else { ly - py } - py;
                    let z = iz as f32 * lz + if iz % 2 == 0 { pz } else { lz - pz } - pz;
                    let d = (x * x + y * y + z * z).sqrt().max(0.1);
                    // Spherical spreading, and one absorption per bounce.
                    let level = (1.0 - absorb).powi(n) / d;
                    // A source to the side arrives at the near ear first and
                    // louder; that is the whole of the stereo here.
                    let pan = (x / d).clamp(-1.0, 1.0);
                    found.push((d / C, level, pan));
                }
            }
        }
        found.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let cap = self.line.capacity() as f32 - 8.0;
        let mut used = 0;
        let mut loudest = 0.0f32;
        for &(t, level, pan) in found.iter() {
            if used == count {
                break;
            }
            let at = t * self.fs;
            if at < 1.0 || at > cap {
                continue;
            }
            let p = pan * width.clamp(0.0, 1.0);
            // Equal power either side, so moving a tap across does not change
            // how loud the pattern is.
            let a = (p + 1.0) * std::f32::consts::FRAC_PI_4;
            self.taps[used] = Tap {
                at,
                l: level * a.cos(),
                r: level * a.sin(),
            };
            loudest = loudest.max(level);
            used += 1;
        }
        self.used = used;

        // Normalised to the loudest arrival, so changing the size or the
        // absorption moves the *pattern* and not the level. A control that
        // changes two things at once is two controls.
        if loudest > 0.0 {
            let k = 1.0 / loudest;
            for t in &mut self.taps[..used] {
                t.l *= k;
                t.r *= k;
            }
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        self.line.push(x);
        let mut l = 0.0;
        let mut r = 0.0;
        for t in &self.taps[..self.used] {
            let v = self.line.read(t.at);
            l += v * t.l;
            r += v * t.r;
        }
        (l, r)
    }
}
