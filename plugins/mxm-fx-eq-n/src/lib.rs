//! `mxm-fx-eq-n` — a parametric EQ with only the bands you need (`plans/plan-mxm-fx-eq-n.md`).
//!
//! The shell: 24 band slots and two globals as host parameters (`params`), the preset
//! seam (`preset`), band solo as a transient command (`audition`), the Out meter and the editor
//! link (`telemetry`), the analyser's audio half (`analyser`) and the registry other tracks' editors
//! read this one through (`instances`, plan §7.8), over `crates/mxm-fx-eq-n-dsp`. The editor is E3's.

macro_rules! plugin_name {
    () => {
        "mxm-fx-eq-n"
    };
}

pub const NAME: &str = plugin_name!();
pub const CLAP_ID: &str = concat!("dk.mxm.", plugin_name!());

pub mod analyser;
pub mod audition;
pub mod editor;
pub mod instances;
pub mod learned;
pub mod params;
pub mod preset;
pub mod preview;
pub mod telemetry;
pub mod view;

use std::sync::Arc;

use analyser::{BLOCK_FRAMES, Block, Tap};
use audition::Audition;
use mxm_fx_eq_n_dsp::{Engine, MIN_SAMPLE_RATE, Phase};
use nice_plug::prelude::*;
use params::EqParams;
use telemetry::Telemetry;

pub struct MxmFxEqN {
    pub params: Arc<EqParams>,
    telemetry: Arc<Telemetry>,
    audition: Arc<Audition>,
    /// Match's Preview (plan §7.6), and the bands last read from it whole, by slot.
    preview: Arc<preview::Preview>,
    previewed: [Option<preview::Previewed>; mxm_fx_eq_n_dsp::SLOTS],
    /// The undo history: the instance's, so it survives the editor closing (plan §5.4).
    history: editor::history::Shared,
    engine: Box<Engine>,
    tap: Tap,
    /// The ring's editor half, until the editor takes it.
    analyser: Option<rtrb::Consumer<Block>>,
    /// The share tap, post-EQ only, copying while another track's editor watches this one, and
    /// this instance's registry entry (plan §7.8).
    share: Tap,
    instance: Arc<instances::Instance>,
    channels: usize,
    /// The phase the parameters last asked for while active, reported to the host as its latency
    /// (plan §7.7); the engine's own is the one in force.
    asked_phase: Phase,
}

impl Default for MxmFxEqN {
    fn default() -> Self {
        let (tap, consumer) = analyser::ring();
        let (share, share_consumer) = analyser::ring_with(instances::SHARE_BLOCKS);
        let params = Arc::new(EqParams::default());
        let instance = instances::Instance::join_learned(
            params.identity.shared(),
            params.learned.shared(),
            share_consumer,
        );
        Self {
            params,
            telemetry: Arc::new(Telemetry::default()),
            audition: Arc::new(Audition::default()),
            preview: Arc::new(preview::Preview::default()),
            previewed: [None; mxm_fx_eq_n_dsp::SLOTS],
            history: editor::history::Shared::default(),
            engine: Box::new(Engine::new(48_000.0)),
            tap,
            analyser: Some(consumer),
            share,
            instance,
            channels: 2,
            asked_phase: Phase::ZeroLatency,
        }
    }
}

/// The latency `phase` runs at, at `sample_rate` (plan §7.7).
fn latency_of(phase: Phase, sample_rate: f32) -> u32 {
    match phase {
        Phase::ZeroLatency => 0,
        Phase::Linear(resolution) => {
            mxm_fx_eq_n_dsp::linear::Sizes::new(f64::from(sample_rate), resolution).latency() as u32
        }
    }
}

impl MxmFxEqN {
    /// The parameters, the solo request and Match's Preview, into the engine. Solo and Preview are
    /// auditions, played only while an editor holds them, whatever mode the host says it runs in:
    /// Resolve says offline as it plays (the owner, 2026-10-29). A previewed band plays in its slot
    /// while the slot is Off, or in place of the slot's own band where the editor marks it to
    /// replace one Match applied (Off where Apply would turn it Off), static, unkeyed and Stereo, as
    /// Apply writes it; each slot is set once, so a band's transitions run as any band's.
    fn sync(&mut self) {
        let auditioning = self.telemetry.connected();
        if auditioning {
            self.preview.read(&mut self.previewed);
        } else {
            self.previewed = [None; mxm_fx_eq_n_dsp::SLOTS];
        }
        let mut previewing = [false; mxm_fx_eq_n_dsp::SLOTS];
        for (slot, band) in self.params.bands.iter().enumerate() {
            let band = band.band();
            let previewed = self.previewed[slot]
                .filter(|previewed| previewed.replace || band.shape.is_none())
                .map(|previewed| previewed.band);
            previewing[slot] = previewed.is_some();
            self.engine.set_band(slot, previewed.unwrap_or(band));
        }
        for (slot, dynamics) in self.params.dynamics.iter().enumerate() {
            let dynamics = if previewing[slot] {
                mxm_fx_eq_n_dsp::Dynamics::STATIC
            } else {
                dynamics.dynamics()
            };
            self.engine.set_dynamics(slot, dynamics);
        }
        for (slot, key) in self.params.keys.iter().enumerate() {
            self.engine
                .set_side_chain(slot, key.side_chain.value() && !previewing[slot]);
        }
        for (slot, place) in self.params.places.iter().enumerate() {
            let placement = if previewing[slot] {
                mxm_fx_eq_n_dsp::Placement::Stereo
            } else {
                place.placement()
            };
            self.engine.set_placement(slot, placement);
        }
        self.engine.set_output_gain(self.params.output.value());
        self.engine.set_gain_scale(self.params.gain_scale.value());
        self.engine.set_auto_gain(self.params.auto_gain.value());
        let solo = if auditioning {
            self.audition.slot()
        } else {
            None
        };
        self.engine.set_solo(solo);
    }

    /// The engine as the parameters have it, at once: an activation or a reset is not an edit, so
    /// a saved EQ is heard from its first sample rather than fading in from dry. A reset in Linear
    /// phase in realtime hears its kernel once designed, never waiting (`Engine::reset`).
    fn seed(&mut self) {
        self.sync();
        self.engine.reset();
    }

    fn prepare(&mut self, sample_rate: f32, input_channels: usize, offline: bool) -> bool {
        if !(sample_rate.is_finite() && f64::from(sample_rate) >= MIN_SAMPLE_RATE) {
            return false;
        }
        self.channels = input_channels.clamp(1, 2);
        self.telemetry.publish_sample_rate(sample_rate);
        self.telemetry.publish_channels(self.channels);
        self.tap.discontinuity(sample_rate);
        self.share.discontinuity(sample_rate);
        // The phase the parameters hold, built here, off the audio thread (plan §7.7): Linear
        // phase's convolver and design thread, its first kernel designed before the first sample.
        let phase = self.params.phase_mode();
        *self.engine = Engine::with_phase(f64::from(sample_rate), phase, self.channels);
        self.engine.set_offline(offline);
        self.sync();
        self.engine.prime();
        self.engine.reset();
        // What was asked, not what was built: if Linear phase could not start its design thread
        // the engine is Zero latency, and asking again would only restart into the same.
        self.asked_phase = phase;
        self.telemetry.publish_phase(self.engine.phase());
        true
    }

    /// A phase asked for that is not the last one reported: its latency, for the host (plan §7.7).
    /// The engine keeps the phase in force until the host restarts the plugin.
    fn phase_change(&mut self) -> Option<u32> {
        let asked = self.params.phase_mode();
        (asked != self.asked_phase).then(|| {
            self.asked_phase = asked;
            latency_of(asked, self.telemetry.sample_rate())
        })
    }

    /// A block with no side chain, as the shell's tests drive one.
    #[cfg(test)]
    fn process_block(&mut self, channels: &mut [&mut [f32]]) -> ProcessStatus {
        self.process_keyed(channels, None)
    }

    /// A block with the side chain's channels, `key`, if the layout has the port (plan §7.2): only
    /// a keyed band's detector and its solo hear it.
    fn process_keyed(
        &mut self,
        channels: &mut [&mut [f32]],
        key: Option<&[&mut [f32]]>,
    ) -> ProcessStatus {
        self.sync();
        self.tap.set_wanted(self.telemetry.analyser_wanted());
        // A watch from none starts a new generation, so nothing from before it reads as current.
        let watched = self.instance.watched();
        if watched && !self.share.wanted() {
            self.share.discontinuity(self.telemetry.sample_rate());
        }
        self.share.set_wanted(watched);
        let Some(first) = channels.first() else {
            return ProcessStatus::Normal;
        };
        let frames = first.len();
        let stereo = self.channels == 2 && channels.len() >= 2;
        let tagged = if stereo { 2 } else { 1 };
        let metering = self.telemetry.connected();
        let key_wanted = self.telemetry.side_chain_wanted();
        let mut peaks = [0.0f32; 2];

        // Pieces of at most one analyser block, so its pre-EQ copy needs no buffer. The engine's
        // control clock runs on across calls, so the pieces change nothing it does.
        let mut start = 0;
        while start < frames {
            let end = (start + BLOCK_FRAMES).min(frames);
            let length = end - start;
            let wanted = self.tap.wanted();
            let mut pre = [[0.0f32; BLOCK_FRAMES]; 2];
            // The key's channels for this piece; a mono key keys both channels.
            let side = |channel: usize| {
                key.and_then(|key| key.get(channel).or(key.first()))
                    .map(|samples| &samples[start.min(samples.len())..end.min(samples.len())])
            };
            let (left, rest) = channels.split_at_mut(1);
            let left = &mut left[0][start..end];
            if wanted {
                pre[0][..length].copy_from_slice(left);
            }
            if stereo {
                let right = &mut rest[0][start..end];
                if wanted {
                    pre[1][..length].copy_from_slice(right);
                }
                let key = side(0).zip(side(1)).map(|(l, r)| [l, r]);
                self.engine.process_stereo_keyed(left, right, key);
                if watched {
                    self.share.capture_post(tagged, [left, right]);
                }
                if wanted {
                    self.tap.capture(
                        tagged,
                        [&pre[0][..length], &pre[1][..length]],
                        [left, right],
                        key.filter(|_| key_wanted),
                    );
                }
                if metering {
                    peaks[0] = peak_of(peaks[0], left);
                    peaks[1] = peak_of(peaks[1], right);
                }
            } else {
                self.engine.process_mono_keyed(left, side(0));
                if watched {
                    self.share.capture_post(tagged, [left, left]);
                }
                if wanted {
                    let pre = &pre[0][..length];
                    let key = side(0).filter(|_| key_wanted).map(|key| [key, key]);
                    self.tap.capture(tagged, [pre, pre], [left, left], key);
                }
                if metering {
                    peaks[0] = peak_of(peaks[0], left);
                    peaks[1] = peaks[0];
                }
            }
            start = end;
        }
        if metering {
            for (meter, peak) in self.telemetry.output.iter().zip(peaks) {
                meter.publish(peak);
            }
            self.telemetry
                .publish_heard_scale(self.engine.gain_scale() as f32);
            self.telemetry
                .publish_make_up(self.engine.make_up_db() as f32);
            for slot in 0..mxm_fx_eq_n_dsp::SLOTS {
                if let Some((drawn, heard)) = self.engine.gains_db(slot) {
                    self.telemetry
                        .publish_gains(slot, drawn as f32, heard as f32);
                }
            }
        }

        // A band still ringing keeps the host calling; once every state has settled to exact
        // silence a host may sleep the plugin on silent input.
        if self.engine.is_ringing() {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Normal
        }
    }
}

/// The largest magnitude in `samples` and `peak`, keeping a non-finite one: `f32::max` drops NaN,
/// and the meter's policy reads it as a clip.
fn peak_of(peak: f32, samples: &[f32]) -> f32 {
    samples.iter().fold(peak, |peak, x| {
        let magnitude = x.abs();
        if magnitude.is_nan() || magnitude > peak {
            magnitude
        } else {
            peak
        }
    })
}

impl Plugin for MxmFxEqN {
    const NAME: &'static str = NAME;
    const VENDOR: &'static str = "mxm";
    const URL: &'static str = "https://mxm.dk";
    const EMAIL: &'static str = "plugins@mxm.dk";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Four layouts, their order permanent (plan §7.2): the keyed ones first, so a host that never
    /// selects a configuration offers the side chain, then the plain ones MXM Player selects.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            aux_input_ports: &[new_nonzero_u32(2)],
            names: PortNames {
                layout: Some("Stereo with side chain"),
                aux_inputs: &["Side chain"],
                ..PortNames::const_default()
            },
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            aux_input_ports: &[new_nonzero_u32(1)],
            names: PortNames {
                layout: Some("Mono with side chain"),
                aux_inputs: &["Side chain"],
                ..PortNames::const_default()
            },
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = editor::EqEditor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    /// A state from before F1, the side chain or the placements restores every band static, on its
    /// own input, in Stereo, its loaded preset still clean (`preset::add_missing_at_defaults`).
    fn filter_state(state: &mut PluginState) {
        preset::add_missing_at_defaults(state);
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(editor::Handles {
            params: self.params.clone(),
            telemetry: self.telemetry.clone(),
            audition: self.audition.clone(),
            preview: self.preview.clone(),
            history: self.history.clone(),
            analyser: self.analyser.take(),
            instance: self.instance.clone(),
        })
    }

    /// Builds the engine in the phase the parameters hold and **reports its latency** (plan §7.7):
    /// 0 in Zero latency, the kernel's centre and a partition in Linear phase. One latency per
    /// activation, as the kit's conventions have it.
    fn activate(
        &mut self,
        layout: &AudioIOLayout,
        config: &BufferConfig,
        context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // Whether the layout has the side-chain port: all the plugin observes of a route (§7.4).
        self.telemetry
            .publish_side_chain_port(!layout.aux_input_ports.is_empty());
        let prepared = self.prepare(
            config.sample_rate,
            layout
                .main_input_channels
                .map_or(2, |channels| channels.get() as usize),
            config.process_mode == ProcessMode::Offline,
        );
        if prepared {
            context.set_latency_samples(self.engine.latency());
        }
        prepared
    }

    /// Runs after an activation and after a restored state too (the wrapper's state transaction
    /// reactivates and resets), so solo ends there as well.
    fn reset(&mut self) {
        self.audition.end();
        self.tap.discontinuity(self.telemetry.sample_rate());
        self.share.discontinuity(self.telemetry.sample_rate());
        self.seed();
    }

    fn deactivate(&mut self) {
        self.audition.end();
        self.tap.discontinuity(self.telemetry.sample_rate());
        self.share.discontinuity(self.telemetry.sample_rate());
        // Linear phase's design thread ends with its engine; the next activation builds another.
        *self.engine = Engine::new(f64::from(self.telemetry.sample_rate()));
    }

    /// A block. A phase asked for that is not the one in force is reported as its latency, which
    /// nice-plug turns into the host's restart; until then the phase in force plays on (plan §7.7).
    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let Some(latency) = self.phase_change() {
            context.set_latency_samples(latency);
        }
        let key = aux.inputs.first_mut().map(|key| &*key.as_slice());
        self.process_keyed(buffer.as_slice(), key)
    }
}

impl ClapPlugin for MxmFxEqN {
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "A parametric EQ with only the bands you need, up to 24, matched to analogue curves right up to the top of the range",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Equalizer,
        ClapFeature::Mono,
        ClapFeature::Stereo,
    ];

    // Its factory presets, for a host to list and load (`mxm_preset::clap`).
    const CLAP_PRESETS: Option<ClapPresets> = Some(mxm_preset::clap::PRESETS);

    fn preset_loader(&self) -> Option<Arc<dyn ClapPresetLoader>> {
        Some(Arc::new(mxm_preset::clap::Loader(self.params.clone())))
    }
}

impl Vst3Plugin for MxmFxEqN {
    /// Permanent, as `CLAP_ID` is: the CLAP id's hash written out, held to it by
    /// `identity::the_vst3_class_id_is_the_clap_ids_hash`.
    const VST3_CLASS_ID: [u8; 16] = [
        0x18, 0xe7, 0x78, 0x58, 0x44, 0x76, 0x98, 0x70, 0x0c, 0xe2, 0xe5, 0x6d, 0xd9, 0xfa, 0xc6,
        0x75,
    ];
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}

nice_export_clap!(MxmFxEqN);
nice_export_vst3!(MxmFxEqN);

/// The plugin's identity in each format.
#[cfg(test)]
mod identity {
    use super::MxmFxEqN;
    use mxm_plugin_test::formats;

    #[test]
    fn the_vst3_class_id_is_the_clap_ids_hash() {
        formats::the_vst3_class_id_is_the_clap_ids_hash::<MxmFxEqN>();
    }

    #[test]
    fn the_vst3_categories_follow_the_clap_features() {
        formats::the_vst3_categories_follow_the_clap_features::<MxmFxEqN>();
    }
}

#[cfg(test)]
mod tests;

/// What a player reads, in the editor's hover text and in a host's plugin browser, speaks to the
/// player about the sound, never about the machine or the code (`mxm_plugin_test::hover_text`).
#[cfg(test)]
mod speaks_to_the_player {
    #[test]
    fn hover_text() {
        mxm_plugin_test::hover_text::speaks_to_the_player(env!("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn host_description() {
        mxm_plugin_test::hover_text::host_description_speaks_to_the_player(env!(
            "CARGO_MANIFEST_DIR"
        ));
    }
}
