//! `mxm-fx-eq-n` — a parametric EQ with only the bands you need (`plans/plan-mxm-fx-eq-n.md`).
//!
//! The shell: 24 band slots and two globals as permanent host parameters (`params`), the preset
//! seam (`preset`), band solo as a transient command (`audition`), the Out meter and the editor
//! link (`telemetry`) and the analyser's audio half (`analyser`), over `crates/mxm-fx-eq-n-dsp`. The
//! editor is E3's.

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
pub mod params;
pub mod preset;
pub mod telemetry;
pub mod view;

use std::sync::Arc;

use analyser::{BLOCK_FRAMES, Block, Tap};
use audition::Audition;
use mxm_fx_eq_n_dsp::{Engine, MIN_SAMPLE_RATE};
use nice_plug::prelude::*;
use params::EqParams;
use telemetry::Telemetry;

pub struct MxmFxEqN {
    pub params: Arc<EqParams>,
    telemetry: Arc<Telemetry>,
    audition: Arc<Audition>,
    /// The undo history: the instance's, so it survives the editor closing (plan §5.4).
    history: editor::history::Shared,
    engine: Box<Engine>,
    tap: Tap,
    /// The ring's editor half, until the editor takes it.
    analyser: Option<rtrb::Consumer<Block>>,
    channels: usize,
    offline: bool,
}

impl Default for MxmFxEqN {
    fn default() -> Self {
        let (tap, consumer) = analyser::ring();
        Self {
            params: Arc::new(EqParams::default()),
            telemetry: Arc::new(Telemetry::default()),
            audition: Arc::new(Audition::default()),
            history: editor::history::Shared::default(),
            engine: Box::new(Engine::new(48_000.0)),
            tap,
            analyser: Some(consumer),
            channels: 2,
            offline: false,
        }
    }
}

impl MxmFxEqN {
    /// The parameters and the solo request, into the engine. Solo is an audition: never in an
    /// offline render, and never without an editor to hold it.
    fn sync(&mut self) {
        for (slot, band) in self.params.bands.iter().enumerate() {
            self.engine.set_band(slot, band.band());
        }
        for (slot, dynamics) in self.params.dynamics.iter().enumerate() {
            self.engine.set_dynamics(slot, dynamics.dynamics());
        }
        for (slot, key) in self.params.keys.iter().enumerate() {
            self.engine.set_side_chain(slot, key.side_chain.value());
        }
        for (slot, place) in self.params.places.iter().enumerate() {
            self.engine.set_placement(slot, place.placement());
        }
        self.engine.set_output_gain(self.params.output.value());
        self.engine.set_gain_scale(self.params.gain_scale.value());
        let solo = if self.offline || !self.telemetry.connected() {
            None
        } else {
            self.audition.slot()
        };
        self.engine.set_solo(solo);
    }

    /// The engine as the parameters have it, at once: an activation or a reset is not an edit, so
    /// a saved EQ is heard from its first sample rather than fading in from dry.
    fn seed(&mut self) {
        self.sync();
        self.engine.reset();
    }

    fn prepare(&mut self, sample_rate: f32, input_channels: usize, offline: bool) -> bool {
        if !(sample_rate.is_finite() && f64::from(sample_rate) >= MIN_SAMPLE_RATE) {
            return false;
        }
        self.channels = input_channels.clamp(1, 2);
        self.offline = offline;
        self.telemetry.publish_sample_rate(sample_rate);
        self.telemetry.publish_channels(self.channels);
        self.tap.discontinuity(sample_rate);
        *self.engine = Engine::new(f64::from(sample_rate));
        self.seed();
        true
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
        let Some(first) = channels.first() else {
            return ProcessStatus::Normal;
        };
        let frames = first.len();
        let stereo = self.channels == 2 && channels.len() >= 2;
        let tagged = if stereo { 2 } else { 1 };
        let metering = self.telemetry.connected();
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
                if wanted {
                    self.tap.capture(
                        tagged,
                        [&pre[0][..length], &pre[1][..length]],
                        [left, right],
                    );
                }
                if metering {
                    peaks[0] = peak_of(peaks[0], left);
                    peaks[1] = peak_of(peaks[1], right);
                }
            } else {
                self.engine.process_mono_keyed(left, side(0));
                if wanted {
                    let pre = &pre[0][..length];
                    self.tap.capture(tagged, [pre, pre], [left, left]);
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
            history: self.history.clone(),
            analyser: self.analyser.take(),
        })
    }

    fn activate(
        &mut self,
        layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.prepare(
            config.sample_rate,
            layout
                .main_input_channels
                .map_or(2, |channels| channels.get() as usize),
            config.process_mode == ProcessMode::Offline,
        )
    }

    /// Runs after an activation and after a restored state too (the wrapper's state transaction
    /// reactivates and resets), so solo ends there as well.
    fn reset(&mut self) {
        self.audition.end();
        self.tap.discontinuity(self.telemetry.sample_rate());
        self.seed();
    }

    fn deactivate(&mut self) {
        self.audition.end();
        self.tap.discontinuity(self.telemetry.sample_rate());
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
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
}

nice_export_clap!(MxmFxEqN);

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
