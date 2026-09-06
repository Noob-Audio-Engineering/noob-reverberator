//! The tank's own saturation, in one place so the three architectures agree.
//!
//! A reverb that is only ever linear stays polite however loud it is fed:
//! every pass is the same pass at a smaller level, so a loud tail is a quiet
//! tail turned up. Real tanks are not like that. A plate is driven by a
//! transducer and read by another, a spring has an amplifier at each end, and
//! a chamber has a loudspeaker in it --- each of those compresses every trip,
//! so a loud tail is *denser* than a quiet one rather than the same shape
//! scaled.
//!
//! # Unity slope at the origin is the whole design
//!
//! The decay curve is fitted as a small-signal property: the loss filters are
//! solved for a given loss per trip, and that arithmetic assumes the trip is
//! linear. A saturator that squeezed the last of a tail would make every mode
//! decay at a length other than the one it asks for, and
//! `every_mode_decays_at_the_length_it_claims` would be right to complain.
//!
//! `tanh(g·x)/g` has slope one at zero for any `g`, so a tail on its way out
//! passes through untouched however hard the tank is driven, and only the
//! loud part of it is compressed. That is also what the ear expects: the
//! thickening happens while the reverb is loud and lets go as it dies.

/// How hot each tank runs, relative to the network, so one control gives all
/// three the same squeeze.
///
/// A single knee cannot fit them: driven by the same loud signal the three
/// architectures come out at very different internal levels, and a knee
/// placed for the network is a knee the plate never reaches.
///
/// Measured by `examples/tank_level.rs` under two seconds of noise at 0.7, as
/// the RMS of the signal actually handed to [`saturate`] on each
/// architecture's first mode, and what the control does to the output at the
/// levels those constants give:
///
/// | tank    | loop RMS | wet RMS | against the network | wet RMS driven |
/// |---------|----------|---------|--------------------|----------------|
/// | network | 0.504    | 0.227   | 1.00               | 0.160          |
/// | plate   | 0.073    | 0.109   | 0.15               | 0.066          |
/// | spring  | 0.733    | 0.829   | 1.45               | 0.466          |
///
/// The loop column is the one that matters and the wet column is why: read
/// off the output instead, the plate looks like half the network and is
/// really a seventh of it, because it is scaled on the way out and the others
/// are not. Placing the knee from the output gave the plate a squeeze of
/// three parts in a thousand, which the test called what it was.
///
/// These are this engine's own numbers and nobody else's --- a property of
/// how these three tanks are built here, so re-measure rather than reason if
/// the gain structure moves.
pub const NETWORK_LEVEL: f32 = 1.0;
pub const PLATE_LEVEL: f32 = 0.15;
pub const SPRING_LEVEL: f32 = 1.45;
/// The plate's transducer, which sees the signal on its way in rather than
/// the quieter one going round. Its level is the plug-in's input level.
pub const PLATE_IN_LEVEL: f32 = 0.7;

/// `amount` is 0 for a linear tank and 1 for as hard as it is driven;
/// `level` is how hot this tank runs, from the table above.
///
/// Placed in the feedback path, after the loss filter, so it acts once per
/// trip rather than once on the way in.
#[inline]
pub fn saturate(x: f32, amount: f32, level: f32) -> f32 {
    if amount <= 0.0 {
        return x;
    }
    // Dividing by the level is what puts the knee in the same place for every
    // tank: a quieter loop needs a steeper curve to be squeezed as much.
    // Unity slope at the origin survives it --- `tanh(g x)/g` has slope one
    // at zero whatever `g` is --- so the decay stays where the fit put it
    // however the levels move.
    //
    // At full drive a tank running at its own nominal level comes back about
    // twelve decibels down, which is a hard squeeze and is what the top of
    // the control is for, while a tail a hundredth of that is untouched to
    // four figures.
    let g = (1.0 + 3.0 * amount) / level.max(1e-3);
    (x * g).tanh() / g
}
