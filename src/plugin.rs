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
use noob_vst_webgui_framework::Assets;
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
pub struct NoobReverberatorParams {
    params: Vec<(String, Arc<FloatParam>)>,
    ui_store: StoreSlot,
}

impl Default for NoobReverberatorParams {
    fn default() -> Self {
        let params = dsp::params::param_specs()
            .into_iter()
            .map(|s| {
                let range = FloatRange::Linear {
                    min: s.min,
                    max: s.max,
                };
                // The unit has to outlive the parameter, and the spec does
                // not: nih-plug keeps a `&'static str`.
                let unit: &'static str = Box::leak(s.unit.clone().into_boxed_str());
                let p = FloatParam::new(s.name.clone(), s.default, range).with_unit(unit);
                (s.id.clone(), Arc::new(p))
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
            .map(|(id, p)| {
                (
                    id.clone(),
                    ParamPtr::FloatParam(Arc::as_ptr(p) as *mut FloatParam),
                    String::new(),
                )
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
    right: Vec<f32>,
    /// Blocks since the curves were last published.
    since: u32,
}

/// The streams, in the order `dsp::streams` declares them.
const S_METER: usize = 0;
const S_ASKED: usize = 1;
const S_REALISED: usize = 2;

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
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let Some(ix) = self.ix.as_ref() else {
            return ProcessStatus::Normal;
        };
        let Some(audio) = self.host.audio() else {
            return ProcessStatus::Normal;
        };

        let settings = dsp::engine::read_settings(audio, ix);
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
