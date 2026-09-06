//! Pre-delay in time, or in the tempo of whatever is playing.
//!
//! Pro-R offers both: a free pre-delay in milliseconds, and one synchronised
//! to the host with an offset "from 50% to 200% of the synchronized time".
//! The offset is the part worth copying --- a pre-delay locked exactly to an
//! eighth note is often slightly wrong, and being able to sit just behind or
//! just ahead of the beat is what makes a synced delay usable rather than
//! merely correct.
//!
//! # What happens with no tempo
//!
//! The standalone has no transport, and a host can report none. Synced to a
//! tempo that does not exist, the honest answer is to fall back to the free
//! time rather than to invent 120 --- a pre-delay that silently assumes a
//! tempo is a pre-delay that is wrong in a way nobody can see.

/// The longest pre-delay the line can hold, in milliseconds.
///
/// Two seconds, and the reason is the synced divisions rather than the free
/// control: the free one goes to 500 ms, which is Pro-R's range, but a dotted
/// quarter at 120 bpm is 750 ms and at 60 bpm it is 1,500. A buffer sized for
/// the free control silently clamps those to something that is not the
/// division it says --- measured, a dotted quarter came out as a plain one.
pub const MAX_PREDELAY_MS: f32 = 2_000.0;

/// The divisions on offer. **Append only**: a saved state stores the index.
pub const SYNC_NAMES: [&str; 11] = [
    "Free", "1/4", "1/4T", "1/4.", "1/8", "1/8T", "1/8.", "1/16", "1/16T", "1/16.", "1/32",
];

/// How many beats each division is worth. A beat is a quarter note, which is
/// what a host's tempo is in.
const BEATS: [f32; 11] = [
    0.0,       // Free
    1.0,       // 1/4
    2.0 / 3.0, // 1/4 triplet
    1.5,       // 1/4 dotted
    0.5,       // 1/8
    1.0 / 3.0, // 1/8 triplet
    0.75,      // 1/8 dotted
    0.25,      // 1/16
    1.0 / 6.0, // 1/16 triplet
    0.375,     // 1/16 dotted
    0.125,     // 1/32
];

/// The pre-delay to use, in milliseconds.
///
/// `free_ms` is the control's own value, used when the division is `Free` or
/// when the host has not said what tempo it is playing at. `offset` is a
/// multiplier on the synced time, so 0.5 sits half a division early and 2.0
/// a division late.
pub fn predelay_ms(division: usize, free_ms: f32, tempo: Option<f32>, offset: f32) -> f32 {
    let beats = BEATS[division.min(BEATS.len() - 1)];
    match tempo {
        Some(bpm) if beats > 0.0 && bpm > 1.0 => {
            let beat_ms = 60_000.0 / bpm;
            (beats * beat_ms * offset.clamp(0.5, 2.0)).clamp(0.0, MAX_PREDELAY_MS)
        }
        // No division, or nothing to sync to.
        _ => free_ms,
    }
}
