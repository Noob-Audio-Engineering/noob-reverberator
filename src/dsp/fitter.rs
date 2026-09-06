//! Working out the loss filters somewhere other than the audio thread.
//!
//! # Why this exists
//!
//! Fitting the loss is Gauss--Newton with a Jacobian taken by finite
//! differences, and it is not cheap. Measured on a sixteen-line network, one
//! refit costs:
//!
//! | bands in use | one refit |
//! |---|---|
//! | 1 | 2.6 ms |
//! | 4 | 13.8 ms |
//! | 8 | 40 ms |
//! | 16 | 158 ms |
//! | 32 | 695 ms |
//!
//! A block at 48 kHz and 256 samples is **5.3 ms**. So the fit had no business
//! on the audio thread at any band count, and at the limit it would stall it
//! for two-thirds of a second every time a control moved. It was there anyway,
//! because with the first version of this plug-in the number was small enough
//! not to be noticed, which is the way that kind of fault usually arrives.
//!
//! # What crosses the boundary
//!
//! Only numbers. The worker owns its own filters to measure with, and sends
//! back what it worked out; the audio thread applies that to *its* filters,
//! which keeps their state, so a curve can change while the tail is ringing
//! without the tail restarting.
//!
//! While a fit is in flight the reverb keeps running with the filters it has.
//! A decay curve taking a moment to take effect is inaudible; a dropout is
//! not.

use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use super::decay::Curve;
use super::fdn::MAX_LINES;
use super::loss::{Fitted, Loss, Scratch};

/// What the audio thread asks for.
pub struct Job {
    pub fs: f32,
    pub curve: Curve,
    /// The network's line lengths, and how many are in use.
    pub lens: [f32; MAX_LINES],
    pub lines: usize,
    /// One lap of each half of the plate, and one trip of each coil of the
    /// spring. The two coils are different lengths --- that is what a tank is
    /// --- so one fit will not do for both: coefficients solved for one trip
    /// applied to the other decay at the wrong rate, and measured, that put
    /// the spring's tilt at 1.13 octaves where its curve drew 0.58.
    pub plate_laps: [f32; 2],
    pub spring_trips: [f32; 2],
}

/// What comes back.
pub struct Done {
    pub net: [Fitted; MAX_LINES],
    pub plate: [Fitted; 2],
    pub spring: [Fitted; 2],
}

pub struct Fitter {
    tx: SyncSender<Job>,
    rx: Receiver<Done>,
    /// Whether a job is out. One at a time: a second would only be answering
    /// a question already out of date.
    pending: bool,
}

impl Default for Fitter {
    fn default() -> Self {
        Self::new()
    }
}

impl Fitter {
    pub fn new() -> Self {
        // Room for one of each. A full queue means the worker is still busy
        // with a request that is already stale, and the right thing is to
        // drop this one and ask again next block rather than to pile up.
        let (tx, job_rx) = sync_channel::<Job>(1);
        let (done_tx, rx) = sync_channel::<Done>(1);
        thread::Builder::new()
            .name("noob-reverberator-fit".into())
            .spawn(move || {
                // The worker's own filters, used only to measure with.
                let mut net: Vec<Loss> = vec![Loss::default(); MAX_LINES];
                let mut plate = [Loss::default(), Loss::default()];
                let mut spring = [Loss::default(), Loss::default()];
                let mut scratch = Scratch::default();
                // Ends when the sender is dropped, which is when the plug-in
                // instance goes away.
                while let Ok(job) = job_rx.recv() {
                    let mut out = Done {
                        net: [Fitted::default(); MAX_LINES],
                        plate: [Fitted::default(); 2],
                        spring: [Fitted::default(); 2],
                    };
                    let lines = job.lines.min(MAX_LINES);
                    for (i, (l, o)) in net[..lines]
                        .iter_mut()
                        .zip(out.net[..lines].iter_mut())
                        .enumerate()
                    {
                        *o = l.solve_fit(job.fs, job.lens[i] as usize, &job.curve, &mut scratch);
                    }
                    for (i, (l, o)) in plate.iter_mut().zip(out.plate.iter_mut()).enumerate() {
                        *o = l.solve_fit(
                            job.fs,
                            job.plate_laps[i] as usize,
                            &job.curve,
                            &mut scratch,
                        );
                    }
                    for (i, (c, o)) in spring.iter_mut().zip(out.spring.iter_mut()).enumerate() {
                        *o = c.solve_fit(
                            job.fs,
                            job.spring_trips[i] as usize,
                            &job.curve,
                            &mut scratch,
                        );
                    }
                    // Dropped rather than queued if nobody has collected the
                    // last one: the newer answer is the one worth having.
                    let _ = done_tx.try_send(out);
                }
            })
            .expect("the fitting thread would not start");
        Fitter {
            tx,
            rx,
            pending: false,
        }
    }

    /// Ask for a fit. Never blocks; returns whether the request went out.
    pub fn request(&mut self, job: Job) -> bool {
        match self.tx.try_send(job) {
            Ok(()) => {
                self.pending = true;
                true
            }
            Err(_) => false,
        }
    }

    /// Collect a finished fit, if one is ready. Never blocks.
    pub fn collect(&mut self) -> Option<Done> {
        match self.rx.try_recv() {
            Ok(d) => {
                self.pending = false;
                Some(d)
            }
            Err(_) => None,
        }
    }

    pub fn busy(&self) -> bool {
        self.pending
    }
}
