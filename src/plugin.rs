//! The nih-plug plug-in: VST3 and CLAP, stereo or mono, with its editor in the
//! operating system's web view showing the page from `web/dist`.
//!
//! Almost nothing here is specific to a reverb. The bridge, the parameter
//! mirroring, the editor handle, the UI store and the input/output layouts all
//! come from `noob-vst-webgui-framework-nih`, which is where they belong: they
//! are the same in every plug-in in this organisation, and a copy in each one
//! is five copies to keep agreeing with each other.
//!
//! What is specific is below: the parameter list, generated from the same
//! [`param_specs`](crate::dsp::params::param_specs) the standalone and the
//! page use, and `process`.

use std::sync::Arc;

use include_dir::{Dir, include_dir};
use nih_plug::prelude::*;
use noob_vst_webgui_framework::{Assets, Taper};
use noob_vst_webgui_framework_nih::{
    EditorConfig, PluginHost, StoreSlot, UiStoreParams, noob_identity, stereo_or_mono_io,
    ui_store_fields,
};

use crate::dsp::{
    self,
    engine::{ParamIx, Reverb},
};

/// The built page, embedded at compile time. `--features plugin` without a
/// built `web/dist` is a plug-in with no interface.
static WEB: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web/dist");

/// Asset lookup for the editor: a path relative to the built page's root to
/// its bytes.
fn ui_lookup(path: &str) -> Option<&'static [u8]> {
    WEB.get_file(path).map(|f| f.contents())
}

/// Every host parameter, built from the one list.
///
/// Generated rather than written out: the standalone, the page and the host
/// must agree about ids, ranges and defaults, and the only way to be sure of
/// that is for there to be one place they come from.
/// Turn a [`ParamSpec`]'s taper into the range nih-plug will give the host.
///
/// **Every parameter used to be `FloatRange::Linear` regardless of what its
/// spec said**, which is a declared-and-ignored control of a subtler kind than
/// a knob wired to nothing: the plain values were all correct, so the reverb
/// sounded right and nothing measured wrong. What was wrong was the shape of
/// the host's automation lane. `decay` runs from 0.1 to 30 seconds
/// logarithmically, and drawn linearly the whole useful part of it --- every
/// setting under three seconds --- lives in the bottom tenth of the lane,
/// where it cannot be drawn or nudged.
///
/// Changing this does **not** disturb a saved session: nih-plug serialises
/// `unmodulated_plain_value` and restores with `set_plain_value`, so a project
/// holding 2.4 seconds gets 2.4 seconds back whatever the mapping is. Existing
/// automation *lanes* move, because those are stored as normalised positions,
/// and there is no way to fix the curve without that.
fn host_range(s: &noob_vst_webgui_framework::ParamSpec) -> FloatRange {
    match s.taper {
        Taper::Linear | Taper::Table => FloatRange::Linear {
            min: s.min,
            max: s.max,
        },
        // nih-plug's skew is `min + (max - min) * norm^(1/factor)`, which is
        // the framework's `Skew` written the same way round.
        Taper::Skew(k) if k > 0.0 => FloatRange::Skewed {
            min: s.min,
            max: s.max,
            factor: k,
        },
        Taper::Skew(_) => FloatRange::Linear {
            min: s.min,
            max: s.max,
        },
        // `dsp::params` no longer declares a true logarithm --- see `LogLike`
        // there --- because nih-plug cannot express one and two curves under
        // one name is worse than one curve that is not quite a logarithm. The
        // arm stays so that a spec written elsewhere still maps to something
        // sensible rather than silently to linear.
        Taper::Log => {
            let (min, max) = (s.min.max(f32::MIN_POSITIVE), s.max);
            let mid = (min * max).sqrt();
            let k = (mid - min) / (max - min);
            if k > 0.0 && k < 1.0 {
                FloatRange::Skewed {
                    min,
                    max,
                    factor: 0.5f32.ln() / k.ln(),
                }
            } else {
                FloatRange::Linear { min, max }
            }
        }
    }
}

/// One parameter as the host sees it.
///
/// **A switch has to be a switch.** Everything here used to be a `FloatParam`,
/// and `FloatParam::step_count()` is `None` by construction, so every discrete
/// control reached the host --- and, through `mirror_params`, the plug-in's own
/// page --- claiming to be continuous. Seventy of them: the thirty-two-way mode
/// strip, Shape, Era, the pre-delay division, both toggles, and every band's
/// on switch and shape. In a host that is a mode strip drawn as a knob and an
/// automation lane that can land between two modes.
///
/// The standalone did not have the bug and could not show it either, because
/// it builds its page from `dsp::params` directly. Only the plug-in goes
/// through `param_map`, so only the plug-in was wrong, and a screenshot of the
/// standalone is not a screenshot of the plug-in.
enum HostParam {
    Float(Arc<FloatParam>),
    Int(Arc<IntParam>),
}

pub struct NoobReverberatorParams {
    params: Vec<(String, String, HostParam)>,
    ui_store: StoreSlot,
}

impl Default for NoobReverberatorParams {
    fn default() -> Self {
        let params = dsp::params::param_specs()
            .into_iter()
            .map(|s| {
                // The unit has to outlive the parameter, and the spec does
                // not: nih-plug keeps a `&'static str`.
                let unit: &'static str = Box::leak(s.unit.clone().into_boxed_str());
                let p = if s.steps >= 2 {
                    // A discrete parameter's plain values are its indices, so
                    // the range is exactly the one the spec already carries.
                    let mut ip = IntParam::new(
                        s.name.clone(),
                        s.default.round() as i32,
                        IntRange::Linear {
                            min: s.min.round() as i32,
                            max: s.max.round() as i32,
                        },
                    )
                    .with_unit(unit);
                    if !s.labels.is_empty() {
                        // Names rather than numbers, in the host's own
                        // parameter list as well as on the page --- the page
                        // reads them back out through
                        // `normalized_value_to_string`.
                        let labels = s.labels.clone();
                        ip = ip.with_value_to_string(Arc::new(move |v| {
                            labels
                                .get(v.max(0) as usize)
                                .cloned()
                                .unwrap_or_else(|| v.to_string())
                        }));
                    }
                    if !s.automatable {
                        ip = ip.non_automatable();
                    }
                    HostParam::Int(Arc::new(ip))
                } else {
                    let mut fp =
                        FloatParam::new(s.name.clone(), s.default, host_range(&s)).with_unit(unit);
                    if !s.automatable {
                        fp = fp.non_automatable();
                    }
                    HostParam::Float(Arc::new(fp))
                };
                (s.id.clone(), s.group.clone(), p)
            })
            .collect();
        NoobReverberatorParams {
            params,
            ui_store: StoreSlot::default(),
        }
    }
}

impl UiStoreParams for NoobReverberatorParams {
    fn ui_store(&self) -> &StoreSlot {
        &self.ui_store
    }
}

/// # Safety
///
/// Every pointer in `param_map` comes from an `Arc<FloatParam>` this struct
/// owns and keeps for its whole life, so each one stays valid and unique for
/// as long as nih-plug can reach it.
unsafe impl Params for NoobReverberatorParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        self.params
            .iter()
            .map(|(id, group, p)| {
                let ptr = match p {
                    HostParam::Float(f) => ParamPtr::FloatParam(Arc::as_ptr(f) as *mut FloatParam),
                    HostParam::Int(i) => ParamPtr::IntParam(Arc::as_ptr(i) as *mut IntParam),
                };
                // The group travels too. It was an empty string, so the
                // plug-in's page lost the grouping the standalone's page has.
                (id.clone(), ptr, group.clone())
            })
            .collect()
    }

    ui_store_fields!(ui_store);
}

pub struct NoobReverberator {
    params: Arc<NoobReverberatorParams>,
    host: PluginHost,
    ix: Option<ParamIx>,
    reverb: Reverb,
    asked: Vec<f32>,
    realised: Vec<f32>,
    spectrum: Vec<f32>,
    right: Vec<f32>,
    /// Blocks since the curves were last published.
    since: u32,
}

/// The streams, in the order `dsp::streams` declares them.
const S_METER: usize = 0;
const S_ASKED: usize = 1;
const S_REALISED: usize = 2;
const S_SPECTRUM: usize = 3;

/// Blocks between publishing the two decay curves: they move when a control
/// does, not per block.
const CURVE_EVERY: u32 = 8;

/// The largest block this will be handed without allocating. A host asking for
/// more is served by the fallback in `process`, which allocates once and says
/// nothing --- but every host in normal use is far under this.
const MAX_BLOCK: usize = 8192;

impl Default for NoobReverberator {
    fn default() -> Self {
        let params = Arc::new(NoobReverberatorParams::default());
        let host = PluginHost::new(
            "Noob Reverberator",
            &params,
            dsp::streams(),
            EditorConfig::new(1240, 900).assets(Assets::Lookup(ui_lookup)),
            |b| b.meta(dsp::bridge_meta(48_000.0, false)),
        );
        NoobReverberator {
            params,
            host,
            ix: None,
            reverb: Reverb::new(48_000.0),
            asked: vec![0.0; dsp::CURVE_POINTS],
            realised: vec![0.0; dsp::CURVE_POINTS],
            spectrum: vec![0.0; dsp::CURVE_POINTS],
            right: vec![0.0; MAX_BLOCK],
            since: 0,
        }
    }
}

impl Plugin for NoobReverberator {
    const NAME: &'static str = "Noob Reverberator";
    noob_identity!();

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = stereo_or_mono_io!();

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(self.host.editor())
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.reverb = Reverb::new(buffer_config.sample_rate);
        self.right
            .resize((buffer_config.max_buffer_size as usize).max(MAX_BLOCK), 0.0);
        if self.ix.is_none() {
            self.ix = Some(ParamIx::new(self.host.bridge()));
        }
        true
    }

    fn reset(&mut self) {
        self.reverb.clear();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(ix) = self.ix.as_ref() else {
            return ProcessStatus::Normal;
        };
        let Some(audio) = self.host.audio() else {
            return ProcessStatus::Normal;
        };

        let mut settings = dsp::engine::read_settings(audio, ix);
        // The one thing the parameters cannot say. `None` when the host does
        // not report a tempo, which the pre-delay treats as "not synced"
        // rather than assuming a number.
        settings.tempo = context.transport().tempo.map(|t| t as f32);
        self.reverb.configure(&settings);

        let n = buffer.samples();
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;

        {
            let slices = buffer.as_slice();
            if slices.is_empty() {
                return ProcessStatus::Normal;
            }
            for s in slices.iter() {
                for &v in s[..n].iter() {
                    peak_in = peak_in.max(v.abs());
                }
            }
            if slices.len() >= 2 {
                let (l, r) = slices.split_at_mut(1);
                self.reverb
                    .process(&settings, &mut l[0][..n], &mut r[0][..n]);
            } else {
                // Mono. The engine is stereo throughout, so the one channel is
                // duplicated into a scratch right and the two are averaged
                // back. Averaged and not summed: the two sides of this reverb
                // are genuinely different, and adding them would make a mono
                // instance louder than a stereo one at the same settings.
                if self.right.len() < n {
                    self.right.resize(n, 0.0);
                }
                self.right[..n].copy_from_slice(&slices[0][..n]);
                self.reverb
                    .process(&settings, &mut slices[0][..n], &mut self.right[..n]);
                for (i, v) in slices[0][..n].iter_mut().enumerate() {
                    *v = 0.5 * (*v + self.right[i]);
                }
            }
            for s in slices.iter() {
                for &v in s[..n].iter() {
                    peak_out = peak_out.max(v.abs());
                }
            }
        }

        audio.publish_slice(S_METER, &[peak_in, peak_in, peak_out, peak_out]);
        self.since += 1;
        if self.since >= CURVE_EVERY {
            self.since = 0;
            let lo = dsp::loss::FIT_BOTTOM;
            let hi = dsp::loss::FIT_TOP;
            for i in 0..dsp::CURVE_POINTS {
                let t = i as f32 / (dsp::CURVE_POINTS - 1) as f32;
                let f = lo * (hi / lo).powf(t);
                self.asked[i] = settings.curve.t60(f);
                self.realised[i] = self.reverb.realised_t60(f);
            }
            audio.publish_slice(S_ASKED, &self.asked);
            audio.publish_slice(S_REALISED, &self.realised);
        }
        // The tail moves, so it goes out more often than the curves do.
        if self.since.is_multiple_of(2) {
            self.reverb.fill_spectrum(&mut self.spectrum);
            audio.publish_slice(S_SPECTRUM, &self.spectrum);
        }

        ProcessStatus::Normal
    }
}

impl Vst3Plugin for NoobReverberator {
    /// Sixteen bytes, fixed forever: a host stores this in every project that
    /// uses the plug-in, and changing it makes those projects fail to find it.
    const VST3_CLASS_ID: [u8; 16] = *b"NoobReverberato1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

impl ClapPlugin for NoobReverberator {
    const CLAP_ID: &'static str = "engineering.noobaudio.reverberator";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("An algorithmic reverb whose decay time is drawn against frequency.");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Reverb,
    ];
}

nih_export_vst3!(NoobReverberator);
nih_export_clap!(NoobReverberator);

#[cfg(test)]
mod tests {
    use super::*;

    /// What the plug-in's own page is told about each parameter, which is not
    /// what the standalone is told: `mirror_params` rebuilds the specs from
    /// the nih-plug parameters, so the page in a host sees whatever
    /// `param_map` says rather than what `dsp::params` says.
    #[test]
    fn the_plugin_page_is_told_about_the_discrete_parameters() {
        let params = NoobReverberatorParams::default();
        let mirrored = noob_vst_webgui_framework_nih::mirror_params(&params);
        let want = dsp::params::param_specs();
        let mut wrong = Vec::new();
        for (m, w) in mirrored.iter().zip(want.iter()) {
            assert_eq!(m.0.id, w.id, "the two lists are in different orders");
            if m.0.steps != w.steps {
                wrong.push(format!(
                    "{} steps host {} page {}",
                    w.id, m.0.steps, w.steps
                ));
            }
            // The fourth field this file used to build a parameter without.
            // `steps`, the taper, the group and this one were all declared in
            // the spec list and dropped on the way to nih-plug; each was
            // invisible until something compared the two ends.
            if m.0.automatable != w.automatable {
                wrong.push(format!(
                    "{} automatable host {} page {}",
                    w.id, m.0.automatable, w.automatable
                ));
            }
            if m.0.group != w.group {
                wrong.push(format!(
                    "{} group host {:?} page {:?}",
                    w.id, m.0.group, w.group
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} parameters reach the plug-in page differently from how they were              declared, so a switch is drawn as a knob or a chooser offered to              automation: {}",
            wrong.len(),
            wrong[..wrong.len().min(6)].join(", ")
        );
        println!(
            "  {} parameters keep their steps, their group and their automation flag",
            want.len()
        );
    }

    /// **A parameter has to mean the same thing to the host and to the page.**
    ///
    /// They arrive by different routes. The standalone builds its page
    /// straight from `dsp::params`; the plug-in's page is rebuilt by
    /// `mirror_params` out of whatever this file hands nih-plug. Those were
    /// not the same mapping --- every parameter was `FloatRange::Linear`
    /// whatever its spec said --- and nothing caught it, because the plain
    /// value is right under either one and only the *shape of the lane* was
    /// wrong: the same decay control read 1.7 seconds at half travel in the
    /// standalone and 15 in a host.
    ///
    /// This walks the real path rather than reconstructing one, so it also
    /// covers the discrete parameters, whose mapping does not go through
    /// `host_range` at all.
    #[test]
    fn the_host_and_the_page_agree_on_every_parameter() {
        let params = NoobReverberatorParams::default();
        let mirrored = noob_vst_webgui_framework_nih::mirror_params(&params);
        let want = dsp::params::param_specs();
        assert_eq!(
            mirrored.len(),
            want.len(),
            "the two lists are different lengths"
        );

        let mut worst = (0.0f32, String::new());
        for (m, w) in mirrored.iter().zip(want.iter()) {
            assert_eq!(m.0.id, w.id, "the two lists are in different orders");
            for step in 0..=20 {
                let norm = step as f32 / 20.0;
                let page = w.denormalize(norm);
                let host = m.0.denormalize(norm);
                // Against the width of the range: an absolute tolerance is
                // meaningless across parameters running 0..1 and 20..3000.
                let span = (w.max - w.min).abs().max(1e-6);
                let off = (page - host).abs() / span;
                if off > worst.0 {
                    worst = (off, format!("{} at {norm:.2}", w.id));
                }
                // A thousandth of a lane. The two sides now use the same
                // curve rather than two approximations of one --- see
                // `LogLike` in `dsp::params` --- so anything above floating
                // point noise is a real disagreement.
                //
                // What this catches is what was here: a logarithmic control
                // handed to the host as a linear one, 44% of its range out at
                // the midpoint, and a switch handed over as a continuous knob.
                assert!(
                    off < 1e-3,
                    "`{}` at {norm:.2} of the lane is {page} to the standalone and \
                     {host} to the plug-in, {:.1}% of its range apart",
                    w.id,
                    off * 100.0
                );
            }
        }
        println!(
            "  {} parameters map identically end to end, worst {:.2}% ({})",
            want.len(),
            worst.0 * 100.0,
            worst.1
        );
    }
}
