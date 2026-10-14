//! The shared preset system's seam (`crates/mxm-preset`; plan §5.4) and the factory set.
//!
//! **Parameters only.** The slots are parameters, so a preset has no custom state: its bands land in
//! their saved slots (decision 10), and Init is generated from the defaults, every slot Off. The
//! factory set is about a dozen sounds named for the job (brief B, *Factory presets*), each a few
//! bands in the lowest slots, so a band the player adds takes the next one.

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
        FACTORY_FILES
    }
}

/// The shipped set, in the order `FACTORY_DESIGN` states it.
///
/// Generated, never hand-edited: `write_the_factory_presets` writes the files from the design, and
/// `shipped_files_are_exactly_the_readable_design` fails when they differ.
pub const FACTORY_FILES: &[(&str, &str)] = &[
    (
        "Gentle low cut",
        include_str!("../presets/gentle-low-cut.json"),
    ),
    (
        "Voice presence",
        include_str!("../presets/voice-presence.json"),
    ),
    ("De-mud", include_str!("../presets/de-mud.json")),
    ("Air", include_str!("../presets/air.json")),
    ("Bass weight", include_str!("../presets/bass-weight.json")),
    (
        "Tilt brighter",
        include_str!("../presets/tilt-brighter.json"),
    ),
    ("Tilt darker", include_str!("../presets/tilt-darker.json")),
    (
        "Telephone band",
        include_str!("../presets/telephone-band.json"),
    ),
    (
        "Band pass sweep",
        include_str!("../presets/band-pass-sweep.json"),
    ),
    ("Tight kick", include_str!("../presets/tight-kick.json")),
    ("Snare crack", include_str!("../presets/snare-crack.json")),
    (
        "Hum notch 50 Hz",
        include_str!("../presets/hum-notch-50-hz.json"),
    ),
    (
        "Hum notch 60 Hz",
        include_str!("../presets/hum-notch-60-hz.json"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{
        BandParams, DEFAULT_SLOPE, SHAPES, shape_of, slope_of, slope_value, type_of,
    };
    use mxm_fx_eq_n_dsp::{
        Design, FREQ_MIN, GAIN_RANGE, Q_MAX, Q_MIN, Shape, Slope, design, max_frequency,
    };
    use mxm_preset::{INIT_NAME, Instrument, Value, factory, library};
    use nice_plug::params::{Param, Params};

    /// One band of a design, in the units the editor shows. A design's bands take slots 1, 2, 3….
    #[derive(Clone, Copy)]
    struct Designed {
        shape: Shape,
        freq: f32,
        gain_db: f32,
        q: f32,
        slope: Slope,
    }

    /// The defaults a new band of each shape takes (brief A4): Q 0.707 for shelves and cuts, 1 for
    /// the rest; a gainless shape stores 0 dB and a shape without a slope the default slope.
    const fn bell(freq: f32, gain_db: f32, q: f32) -> Designed {
        Designed {
            shape: Shape::Bell,
            freq,
            gain_db,
            q,
            slope: DEFAULT_SLOPE,
        }
    }
    const fn low_shelf(freq: f32, gain_db: f32) -> Designed {
        Designed {
            shape: Shape::LowShelf,
            freq,
            gain_db,
            q: 0.707,
            slope: DEFAULT_SLOPE,
        }
    }
    const fn high_shelf(freq: f32, gain_db: f32) -> Designed {
        Designed {
            shape: Shape::HighShelf,
            ..low_shelf(freq, gain_db)
        }
    }
    const fn low_cut(freq: f32, slope: Slope) -> Designed {
        Designed {
            shape: Shape::LowCut,
            freq,
            gain_db: 0.0,
            q: 0.707,
            slope,
        }
    }
    const fn high_cut(freq: f32, slope: Slope) -> Designed {
        Designed {
            shape: Shape::HighCut,
            ..low_cut(freq, slope)
        }
    }
    const fn notch(freq: f32, q: f32) -> Designed {
        Designed {
            shape: Shape::Notch,
            freq,
            gain_db: 0.0,
            q,
            slope: DEFAULT_SLOPE,
        }
    }
    const fn band_pass(freq: f32, q: f32) -> Designed {
        Designed {
            shape: Shape::BandPass,
            ..notch(freq, q)
        }
    }
    /// A tilt's Q is fixed (brief A5), so it keeps the parameter's default.
    const fn tilt(freq: f32, gain_db: f32) -> Designed {
        Designed {
            shape: Shape::TiltShelf,
            freq,
            gain_db,
            q: 1.0,
            slope: DEFAULT_SLOPE,
        }
    }

    /// **The factory set, readable.** Every preset is generated from this table; nothing else
    /// decides a shipped value. The output stays at 0 dB and the gain scale at 100 %, so a preset
    /// changes the sound only through its bands.
    const FACTORY_DESIGN: &[(&str, Category, &[Designed])] = &[
        (
            "Gentle low cut",
            Category::Fx,
            &[low_cut(60.0, Slope::Db12)],
        ),
        (
            "Voice presence",
            Category::Fx,
            &[
                low_cut(80.0, Slope::Db18),
                bell(3_500.0, 3.0, 1.0),
                high_shelf(10_000.0, 1.5),
            ],
        ),
        ("De-mud", Category::Fx, &[bell(300.0, -3.0, 1.2)]),
        ("Air", Category::Fx, &[high_shelf(12_000.0, 3.0)]),
        (
            "Bass weight",
            Category::Fx,
            &[low_cut(25.0, Slope::Db24), low_shelf(100.0, 3.0)],
        ),
        ("Tilt brighter", Category::Fx, &[tilt(1_000.0, 2.0)]),
        ("Tilt darker", Category::Fx, &[tilt(1_000.0, -2.0)]),
        (
            "Telephone band",
            Category::Fx,
            &[
                low_cut(300.0, Slope::Db36),
                bell(1_200.0, 3.0, 1.0),
                high_cut(3_400.0, Slope::Db36),
            ],
        ),
        ("Band pass sweep", Category::Fx, &[band_pass(1_000.0, 2.0)]),
        (
            "Tight kick",
            Category::Percussion,
            &[
                low_cut(30.0, Slope::Db24),
                bell(60.0, 3.0, 1.4),
                bell(350.0, -4.0, 1.5),
                bell(4_000.0, 3.0, 1.2),
            ],
        ),
        (
            "Snare crack",
            Category::Percussion,
            &[
                low_cut(80.0, Slope::Db12),
                bell(200.0, 2.0, 1.2),
                bell(900.0, -3.0, 2.0),
                bell(5_000.0, 3.0, 1.0),
            ],
        ),
        (
            "Hum notch 50 Hz",
            Category::Fx,
            &[
                notch(50.0, 10.0),
                notch(100.0, 10.0),
                notch(150.0, 10.0),
                notch(200.0, 10.0),
            ],
        ),
        (
            "Hum notch 60 Hz",
            Category::Fx,
            &[
                notch(60.0, 10.0),
                notch(120.0, 10.0),
                notch(180.0, 10.0),
                notch(240.0, 10.0),
            ],
        ),
    ];

    /// A design as a complete preset: Init, then each band's six fields in its slot, every text
    /// read in the preset's own state (`mxm_preset::refresh_texts`).
    fn generated(params: &EqParams, name: &str, category: Category, bands: &[Designed]) -> Preset {
        let mut preset = Preset::init(params);
        preset.name = name.to_owned();
        preset.category = category;
        for (slot, band) in bands.iter().enumerate() {
            let fields = &params.bands[slot];
            let values = [
                fields.kind.preview_normalized(type_of(Some(band.shape))),
                fields.bypass.preview_normalized(false),
                fields.freq.preview_normalized(band.freq),
                fields.gain.preview_normalized(band.gain_db),
                fields.q.preview_normalized(band.q),
                fields.slope.preview_normalized(slope_value(band.slope)),
            ];
            for (id, v) in band_ids(slot).iter().zip(values) {
                preset.params.insert(
                    (*id).to_owned(),
                    Value {
                        v,
                        text: String::new(),
                    },
                );
            }
        }
        mxm_preset::refresh_texts(params, &mut preset);
        preset
    }

    #[test]
    #[ignore = "writes the generated factory preset files"]
    fn write_the_factory_presets() {
        let params = EqParams::default();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, category, bands) in FACTORY_DESIGN {
            let preset = generated(&params, name, *category, bands);
            std::fs::write(dir.join(library::file_name(name)), preset.to_json()).unwrap();
        }
    }

    #[test]
    fn shipped_files_are_exactly_the_readable_design() {
        let params = EqParams::default();
        assert_eq!(FACTORY_FILES.len(), FACTORY_DESIGN.len());
        for ((name, category, bands), (file_name, text)) in FACTORY_DESIGN.iter().zip(FACTORY_FILES)
        {
            assert_eq!(
                name, file_name,
                "FACTORY_FILES is not in the design's order"
            );
            assert_eq!(
                Preset::parse(text, crate::CLAP_ID).unwrap(),
                generated(&params, name, *category, bands),
                "regenerate {name}: `cargo test -p mxm-fx-eq-n --lib write_the_factory_presets -- --ignored`"
            );
        }
    }

    /// **About a dozen, named for the job, and every shape used** (brief B, *Factory presets*): the
    /// brief's examples by name, so a renamed one is a decision rather than a drift.
    #[test]
    fn the_set_is_the_briefs_and_uses_every_shape() {
        assert!((10..=16).contains(&FACTORY_DESIGN.len()));
        let names: Vec<_> = FACTORY_DESIGN.iter().map(|(name, _, _)| *name).collect();
        for example in [
            "Gentle low cut",
            "Voice presence",
            "De-mud",
            "Air",
            "Tight kick",
            "Tilt brighter",
            "Tilt darker",
            "Telephone band",
        ] {
            assert!(names.contains(&example), "the brief's {example} is missing");
        }
        for shape in SHAPES {
            assert!(
                FACTORY_DESIGN
                    .iter()
                    .any(|(_, _, bands)| bands.iter().any(|band| band.shape == shape)),
                "no factory preset uses {shape:?}"
            );
        }
    }

    /// Every designed value inside its range at the lowest common rate, and stored as a band of
    /// its shape would be: a gainless shape at 0 dB, a slope only where the shape has one.
    #[test]
    fn every_design_is_inside_the_ranges_and_true_to_its_shape() {
        for (name, _, bands) in FACTORY_DESIGN {
            assert!(!bands.is_empty() && bands.len() <= SLOTS, "{name}");
            for band in *bands {
                let freq = f64::from(band.freq);
                assert!(
                    (FREQ_MIN..=max_frequency(44_100.0)).contains(&freq),
                    "{name}: {freq} Hz"
                );
                assert!(f64::from(band.gain_db).abs() <= GAIN_RANGE, "{name}");
                assert!((Q_MIN..=Q_MAX).contains(&f64::from(band.q)), "{name}");
                if !band.shape.has_gain() {
                    assert_eq!(band.gain_db, 0.0, "{name}: a gainless band stores 0 dB");
                }
                if !band.shape.has_slope() {
                    assert_eq!(band.slope, DEFAULT_SLOPE, "{name}");
                }
            }
        }
    }

    #[test]
    fn every_file_is_complete_distinct_categorised_and_for_this_plugin() {
        let params = EqParams::default();
        let mut seen = Vec::new();
        for (name, text) in FACTORY_FILES {
            let preset =
                Preset::parse(text, crate::CLAP_ID).unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(&preset.name, name);
            assert_ne!(preset.category, Category::Uncategorised, "{name}");
            assert_eq!(preset.params.len(), 146, "{name}");
            assert!(preset.resolve(&params).1.is_empty(), "{name} is incomplete");
            assert!(
                preset.state.is_none(),
                "{name}: presets are parameters only"
            );
            assert!(
                !seen.iter().any(|other| other == &preset.params),
                "{name} duplicates another"
            );
            seen.push(preset.params);
        }
    }

    /// The shipped band of one slot, read from a preset's stored values as the engine would.
    fn stored_band(preset: &Preset, band: &BandParams, slot: usize) -> mxm_fx_eq_n_dsp::Band {
        let v = |field: usize| preset.params[band_ids(slot)[field]].v;
        mxm_fx_eq_n_dsp::Band {
            shape: shape_of(band.kind.preview_plain(v(0))),
            bypass: band.bypass.preview_plain(v(1)),
            freq: band.freq.preview_plain(v(2)),
            gain_db: band.gain.preview_plain(v(3)),
            q: band.q.preview_plain(v(4)),
            slope: slope_of(band.slope.preview_plain(v(5))).unwrap_or(DEFAULT_SLOPE),
        }
    }

    /// **Every preset is heard, and leaves the slots after its bands free**: its bands sit in slots
    /// 1…n, none bypassed, the rest Off; the output and the gain scale stay at Init's; and
    /// the shipped cascade's response moves at least 1 dB somewhere between 20 Hz and 20 kHz.
    #[test]
    fn every_factory_preset_is_heard_and_fills_the_lowest_slots() {
        let params = EqParams::default();
        let init = Preset::init(&params);
        let rate = 48_000.0;
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).unwrap();
            let bands: Vec<_> = (0..SLOTS)
                .map(|slot| stored_band(&preset, &params.bands[slot], slot))
                .collect();
            let used = bands.iter().take_while(|band| band.shape.is_some()).count();
            assert!(used > 0, "{name} has no band");
            assert!(
                bands[used..].iter().all(|band| band.shape.is_none()),
                "{name} leaves a gap in its slots"
            );
            assert!(bands.iter().all(|band| !band.bypass), "{name}");
            for id in GLOBAL_IDS {
                assert_eq!(preset.params[id], init.params[id], "{name}: {id}");
            }
            let cascades: Vec<_> = bands[..used]
                .iter()
                .map(|band| {
                    design(
                        Design {
                            shape: band.shape.unwrap(),
                            slope: band.slope,
                            freq: f64::from(band.freq),
                            gain_db: f64::from(band.gain_db),
                            q: f64::from(band.q),
                        },
                        rate,
                    )
                })
                .collect();
            let widest = (0..=300)
                .map(|step| 20.0 * 1_000f64.powf(f64::from(step) / 300.0))
                .map(|freq| {
                    cascades
                        .iter()
                        .map(|cascade| cascade.response_db(freq, rate))
                        .sum::<f64>()
                        .abs()
                })
                .fold(0.0, f64::max);
            assert!(
                widest >= 1.0,
                "{name} moves the response {widest} dB at most"
            );
        }
    }

    #[test]
    fn factory_begins_with_the_generated_init() {
        let params = EqParams::default();
        let all = factory(&params);
        assert_eq!(
            all.len(),
            FACTORY_FILES.len() + 1,
            "a factory file failed to parse"
        );
        assert_eq!(all[0].name, INIT_NAME);
    }

    #[test]
    fn user_files_are_namespaced_to_the_permanent_id() {
        if let Some(root) = library::user_root(crate::CLAP_ID) {
            assert!(root.to_string_lossy().contains(crate::CLAP_ID));
        }
    }

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
    /// (`mxm_preset::stale_factory_texts`).
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
