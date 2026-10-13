//! The shared preset system's seam (`crates/mxm-preset`; plan §5.4).
//!
//! **Parameters only.** The slots are parameters, so a preset has no custom state: its bands land in
//! their saved slots (decision 10), and Init is generated from the defaults, every slot Off. The
//! factory set is E4's, about a dozen sounds named for the job (brief B, *Factory presets*).

use std::sync::RwLock;

use mxm_fx_eq_n_dsp::SLOTS;
pub use mxm_preset::{Category, Preset, PresetIdentity};

use crate::params::{EqParams, GLOBAL_IDS, band_ids};

impl mxm_preset::Instrument for EqParams {
    fn clap_id(&self) -> &'static str {
        crate::CLAP_ID
    }

    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_params::ErasedParam)> {
        let mut out = Vec::with_capacity(SLOTS * 6 + GLOBAL_IDS.len());
        for (slot, band) in self.bands.iter().enumerate() {
            out.extend(band_ids(slot).iter().copied().zip(band.parameters()));
        }
        let globals: [&dyn mxm_params::ErasedParam; 2] = [&self.output, &self.gain_scale];
        out.extend(GLOBAL_IDS.into_iter().zip(globals));
        out
    }

    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }

    fn factory_files(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mxm_preset::Instrument;
    use nice_plug::params::{Param, Params};

    #[test]
    fn every_host_parameter_is_declared_once_under_its_own_id() {
        let params = EqParams::default();
        let declared = params.parameters();
        assert_eq!(declared.len(), 146);
        let host: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        for (index, (id, _)) in declared.iter().enumerate() {
            assert_eq!(
                declared.iter().filter(|(other, _)| other == id).count(),
                1,
                "{index}: {id}"
            );
            assert!(
                host.iter().any(|other| other == id),
                "{id} is not a host parameter"
            );
        }
    }

    /// **Every stored `text` is this plugin's own formatting of the `v` beside it**
    /// (`mxm_preset::stale_factory_texts`). The factory set is E4's; the check is here before it.
    #[test]
    fn every_factory_text_is_this_plugins_formatting_of_its_value() {
        let stale = mxm_preset::stale_factory_texts(&EqParams::default());
        assert!(
            stale.is_empty(),
            "{} stale texts — regenerate the files:\n{}",
            stale.len(),
            stale.join("\n")
        );
    }

    #[test]
    fn the_init_preset_is_the_parameter_defaults_and_the_empty_eq() {
        let params = EqParams::default();
        let init = Preset::init(&params);
        assert_eq!(init.params.len(), 146);
        for (id, param) in params.parameters() {
            assert_eq!(
                init.params[id].v,
                param.default_normalised(),
                "{id} differs from its default"
            );
        }
        // Every slot Off, the output at 0 dB and the gain scale at 100 % (brief A1).
        for slot in 0..SLOTS {
            assert_eq!(
                init.params[band_ids(slot)[0]].v,
                0.0,
                "slot {slot} is not Off"
            );
        }
        let bands: Vec<_> = params.bands.iter().map(|band| band.band()).collect();
        assert!(bands.iter().all(|band| band.shape.is_none()));
        assert_eq!(params.output.default_plain_value(), 1.0);
        assert_eq!(params.gain_scale.default_plain_value(), 1.0);
    }
}
