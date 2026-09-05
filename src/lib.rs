//! Noob Reverberator: an algorithmic reverb whose decay time is a curve
//! against frequency rather than a pair of crossovers.
//!
//! It is an affectionate spoof of the reverbs I admire --- Valhalla's,
//! FabFilter's Pro-R, Strymon's BigSky --- and it is not a parity replacement
//! or a clone of any of them. `docs/SURVEY.md` says what I read and what I
//! took from it.
//!
//! **Nothing here claims a margin over any of them**, because I have measured
//! none of them. What this plug-in claims is about itself: that the decay you
//! draw at a frequency is the decay the tail has at that frequency, and
//! `docs/BENCHMARK.md` is where that is checked.

pub mod dsp;
