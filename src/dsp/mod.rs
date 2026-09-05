//! Noob Reverberator's engine.
//!
//! The plug-in is about one thing: **the decay time is drawn against
//! frequency, and the network is fitted to it.** [`decay`] holds the curve a
//! person draws, [`loss`] turns it into the filter each delay line needs, and
//! everything else is the reverberator those lines are wired into.

pub mod colour;
pub mod decay;
pub mod delay;
pub mod diffuse;
pub mod early;
pub mod engine;
pub mod fdn;
pub mod filters;
pub mod loss;
pub mod measure;
pub mod mode;
pub mod modulate;
pub mod params;
pub mod pitch;
pub mod plate;
pub mod shape;
pub mod spring;
pub mod svf;

#[cfg(test)]
mod tests;
