//! The permanent host surface (`docs/briefs/mxm-fx-eq-n.md` A1 and A4; plan §2.1).
//!
//! **Twenty-four band slots and two globals, 146 parameters.** The slots are nested, so their ids
//! are `<field>_<slot>`: `type_1`, `bypass_1`, `freq_1`, `gain_1`, `q_1`, `slope_1`, through
//! `slope_24`. The globals are `output` and `gainscale`. Every id is permanent. A host keeps its
//! parameter names from load, so a slot is named by its number ("Band 7 frequency"), never by its
//! shape.
//!
//! **The stepped parameters reserve a fixed domain from this first release** (plan §2.1). Projects
//! and presets store the normalised value, so a range that grew would move every saved shape. The
//! type is 0–31: 0 Off, 1–8 the eight shapes, the rest unassigned and read as unavailable, Off to the
//! DSP. The slope is 0–15: 0–7 the eight slopes, the rest unassigned and read as 12 dB/octave. A new
//! shape or slope takes the next unassigned value. `the_stepped_values_keep_their_normalised_places`
//! pins every value's place.
//!
//! **Nothing here is smoothed by nice-plug.** The DSP glides frequency, gain and Q in their own
//! domains, and fades every structural change (`crates/mxm-fx-eq-n-dsp/AGENTS.md`). A host smoother on
//! top would lag the drawn curve behind the heard one.

use std::sync::{Arc, LazyLock, RwLock};

use mxm_fx_eq_n_dsp::{
    Band, FREQ_MAX, FREQ_MIN, GAIN_RANGE, GAIN_SCALE_MAX, OUTPUT_GAIN_RANGE_DB, Q_MAX, Q_MIN,
    SLOTS, Shape, Slope,
};
use nice_plug::prelude::*;

/// The type's fixed domain: 0 Off, 1–8 the shapes, 9–31 unassigned.
pub const TYPE_MAX: i32 = 31;

/// The slope's fixed domain: 0–7 the slopes, 8–15 unassigned (Brickwall arrives with phase modes,
/// F6).
pub const SLOPE_MAX: i32 = 15;

/// The shapes at their permanent type values, 1–8.
pub const SHAPES: [Shape; 8] = [
    Shape::Bell,
    Shape::LowShelf,
    Shape::HighShelf,
    Shape::LowCut,
    Shape::HighCut,
    Shape::Notch,
    Shape::BandPass,
    Shape::TiltShelf,
];

/// The slope a new or unassigned slope value reads as.
pub const DEFAULT_SLOPE: Slope = Slope::Db12;

/// A slot's fields, in declaration order: the suffixes of its ids.
pub const BAND_FIELDS: [&str; 6] = ["type", "bypass", "freq", "gain", "q", "slope"];

/// The globals' ids, in declaration order.
pub const GLOBAL_IDS: [&str; 2] = ["output", "gainscale"];

/// The shape at a type value, or none for Off and for an unassigned value.
pub fn shape_of(value: i32) -> Option<Shape> {
    usize::try_from(value)
        .ok()
        .and_then(|value| value.checked_sub(1))
        .and_then(|index| SHAPES.get(index).copied())
}

/// The type value of a shape, 0 for Off.
pub fn type_of(shape: Option<Shape>) -> i32 {
    shape.map_or(0, |shape| {
        SHAPES
            .iter()
            .position(|other| *other == shape)
            .map_or(0, |index| index as i32 + 1)
    })
}

/// The slope at a slope value, or none for an unassigned value.
pub fn slope_of(value: i32) -> Option<Slope> {
    usize::try_from(value)
        .ok()
        .and_then(|index| Slope::ALL.get(index).copied())
}

/// The slope value of a slope.
pub fn slope_value(slope: Slope) -> i32 {
    Slope::ALL
        .iter()
        .position(|other| *other == slope)
        .map_or(0, |index| index as i32)
}

/// A shape's name, as the host and the editor show it.
pub fn shape_name(shape: Shape) -> &'static str {
    match shape {
        Shape::Bell => "Bell",
        Shape::LowShelf => "Low shelf",
        Shape::HighShelf => "High shelf",
        Shape::LowCut => "Low cut",
        Shape::HighCut => "High cut",
        Shape::Notch => "Notch",
        Shape::BandPass => "Band pass",
        Shape::TiltShelf => "Tilt shelf",
    }
}

/// One band slot.
#[derive(Params)]
pub struct BandParams {
    /// Off, or the band's shape.
    #[id = "type"]
    pub kind: IntParam,
    /// Heard on or off without losing the shape.
    #[id = "bypass"]
    pub bypass: BoolParam,
    #[id = "freq"]
    pub freq: FloatParam,
    /// In dB: an EQ band's gain is its curve's height, glided and scaled in dB by the DSP.
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "q"]
    pub q: FloatParam,
    #[id = "slope"]
    pub slope: IntParam,
}

impl BandParams {
    fn new(index: usize) -> Self {
        let n = index + 1;
        Self {
            kind: IntParam::new(
                format!("Band {n} type"),
                0,
                IntRange::Linear {
                    min: 0,
                    max: TYPE_MAX,
                },
            )
            .with_value_to_string(Arc::new(type_text))
            .with_string_to_value(Arc::new(type_value)),
            bypass: BoolParam::new(format!("Band {n} bypass"), false),
            freq: FloatParam::new(format!("Band {n} frequency"), 1_000.0, frequency_range())
                .with_value_to_string(Arc::new(frequency_text))
                .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            gain: FloatParam::new(
                format!("Band {n} gain"),
                0.0,
                FloatRange::Linear {
                    min: -GAIN_RANGE as f32,
                    max: GAIN_RANGE as f32,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1))
            .with_string_to_value(Arc::new(|text| {
                text.trim().trim_end_matches("dB").trim().parse().ok()
            })),
            q: FloatParam::new(format!("Band {n} Q"), 1.0, q_range())
                .with_value_to_string(Arc::new(q_text))
                .with_string_to_value(Arc::new(|text| text.trim().parse().ok())),
            slope: IntParam::new(
                format!("Band {n} slope"),
                slope_value(DEFAULT_SLOPE),
                IntRange::Linear {
                    min: 0,
                    max: SLOPE_MAX,
                },
            )
            .with_value_to_string(Arc::new(slope_text))
            .with_string_to_value(Arc::new(slope_parse)),
        }
    }

    /// The slot as the engine takes it.
    pub fn band(&self) -> Band {
        Band {
            shape: shape_of(self.kind.value()),
            bypass: self.bypass.value(),
            freq: self.freq.value(),
            gain_db: self.gain.value(),
            q: self.q.value(),
            slope: slope_of(self.slope.value()).unwrap_or(DEFAULT_SLOPE),
        }
    }

    /// The slot's parameters, in the order of [`BAND_FIELDS`].
    pub fn parameters(&self) -> [&dyn mxm_params::ErasedParam; 6] {
        [
            &self.kind,
            &self.bypass,
            &self.freq,
            &self.gain,
            &self.q,
            &self.slope,
        ]
    }
}

#[derive(Params)]
pub struct EqParams {
    #[nested(array, group = "Band")]
    pub bands: [BandParams; SLOTS],
    /// Last in the chain (plan §2.2). Linear gain, shown in dB (`plugins/AGENTS.md`).
    #[id = "output"]
    pub output: FloatParam,
    /// Multiplies every bell, shelf and tilt gain; cuts, notches and band passes stay as drawn.
    #[id = "gainscale"]
    pub gain_scale: FloatParam,
    #[persist = "preset"]
    pub preset: RwLock<mxm_preset::PresetIdentity>,
    /// The analyser's settings: view state, saved with the project, never a preset's (plan §4).
    #[persist = "view"]
    pub view: crate::view::ViewField,
}

impl Default for EqParams {
    fn default() -> Self {
        Self {
            bands: std::array::from_fn(BandParams::new),
            output: FloatParam::new(
                "Output gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-OUTPUT_GAIN_RANGE_DB as f32),
                    max: util::db_to_gain(OUTPUT_GAIN_RANGE_DB as f32),
                    factor: FloatRange::gain_skew_factor(
                        -OUTPUT_GAIN_RANGE_DB as f32,
                        OUTPUT_GAIN_RANGE_DB as f32,
                    ),
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            gain_scale: FloatParam::new(
                "Gain scale",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: GAIN_SCALE_MAX as f32,
                },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            preset: RwLock::new(mxm_preset::PresetIdentity::default()),
            view: crate::view::ViewField::default(),
        }
    }
}

impl EqParams {
    /// Every host parameter, in the order the undo history's indices name them: each slot's
    /// fields in [`BAND_FIELDS`] order, slot by slot, then [`GLOBAL_IDS`].
    pub fn erased(&self) -> Vec<&dyn mxm_params::ErasedParam> {
        let mut all: Vec<&dyn mxm_params::ErasedParam> = Vec::with_capacity(SLOTS * 6 + 2);
        for band in &self.bands {
            all.extend(band.parameters());
        }
        all.push(&self.output);
        all.push(&self.gain_scale);
        all
    }

    /// [`erased`](Self::erased)'s parameter at `index`, without building the list: the editor
    /// reads parameters one at a time, every frame.
    pub fn param(&self, index: usize) -> &dyn mxm_params::ErasedParam {
        const FIELDS: usize = 6;
        match index {
            index if index < SLOTS * FIELDS => {
                self.bands[index / FIELDS].parameters()[index % FIELDS]
            }
            index if index == SLOTS * FIELDS => &self.output,
            _ => &self.gain_scale,
        }
    }

    /// [`erased`](Self::erased)'s parameters as nice-plug's pointers.
    pub fn pointers(&self) -> Vec<nice_plug::params::internals::ParamPtr> {
        let mut all = Vec::with_capacity(SLOTS * 6 + 2);
        for band in &self.bands {
            all.extend([
                band.kind.as_ptr(),
                band.bypass.as_ptr(),
                band.freq.as_ptr(),
                band.gain.as_ptr(),
                band.q.as_ptr(),
                band.slope.as_ptr(),
            ]);
        }
        all.push(self.output.as_ptr());
        all.push(self.gain_scale.as_ptr());
        all
    }
}

/// Every slot's ids, in declaration order: `type_1` … `slope_24`.
static BAND_IDS: LazyLock<[[&'static str; 6]; SLOTS]> = LazyLock::new(|| {
    std::array::from_fn(|slot| {
        BAND_FIELDS.map(|field| &*Box::leak(format!("{field}_{}", slot + 1).into_boxed_str()))
    })
});

/// Slot `slot`'s (0-based) permanent ids, in the order of [`BAND_FIELDS`].
pub fn band_ids(slot: usize) -> &'static [&'static str; 6] {
    &BAND_IDS[slot]
}

/// Frequency: 10 Hz to 30 kHz, skewed so the middle of the travel is near 1 kHz, as a log axis
/// would put it.
fn frequency_range() -> FloatRange {
    FloatRange::Skewed {
        min: FREQ_MIN as f32,
        max: FREQ_MAX as f32,
        factor: FloatRange::skew_factor(-2.5),
    }
}

/// Q: 0.025 to 40, skewed so Q 1 sits in the middle of the travel, as a log axis would put it.
fn q_range() -> FloatRange {
    FloatRange::Skewed {
        min: Q_MIN as f32,
        max: Q_MAX as f32,
        factor: FloatRange::skew_factor(-2.4),
    }
}

/// `38.5 Hz`, `440.0 Hz`, `1.25 kHz`. The unit is chosen from the rounded reading, so the host's
/// text round trip is idempotent at the switch (`plugins/AGENTS.md`, *Parameters*).
fn frequency_text(hz: f32) -> String {
    let tenths = (hz * 10.0).round() / 10.0;
    if tenths < 1_000.0 {
        format!("{tenths:.1} Hz")
    } else {
        format!("{:.2} kHz", hz / 1_000.0)
    }
}

/// Three significant figures: `0.025`, `0.707`, `1.41`, `12.5`. The precision is chosen from the
/// rounded reading, for the same idempotence.
fn q_text(q: f32) -> String {
    let thousandths = (q * 1_000.0).round() / 1_000.0;
    if thousandths < 1.0 {
        return format!("{thousandths:.3}");
    }
    let hundredths = (q * 100.0).round() / 100.0;
    if hundredths < 10.0 {
        format!("{hundredths:.2}")
    } else {
        format!("{q:.1}")
    }
}

fn type_text(value: i32) -> String {
    match (value, shape_of(value)) {
        (_, Some(shape)) => shape_name(shape).to_owned(),
        (0, None) => "Off".to_owned(),
        (other, None) => format!("Unavailable {other}"),
    }
}

fn type_value(text: &str) -> Option<i32> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("Off") {
        return Some(0);
    }
    if let Some(shape) = SHAPES
        .iter()
        .find(|shape| shape_name(**shape).eq_ignore_ascii_case(text))
    {
        return Some(type_of(Some(*shape)));
    }
    text.strip_prefix("Unavailable ")
        .unwrap_or(text)
        .parse::<i32>()
        .ok()
        .filter(|value| (0..=TYPE_MAX).contains(value))
}

fn slope_text(value: i32) -> String {
    slope_of(value).map_or_else(
        || format!("Unavailable {value}"),
        |slope| format!("{} dB/oct", slope.db_per_octave()),
    )
}

fn slope_parse(text: &str) -> Option<i32> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("Unavailable ") {
        return rest
            .parse::<i32>()
            .ok()
            .filter(|value| (0..=SLOPE_MAX).contains(value));
    }
    let db: u16 = text
        .trim_end_matches("dB/oct")
        .trim_end_matches("dB")
        .trim()
        .parse()
        .ok()?;
    Slope::ALL
        .iter()
        .find(|slope| slope.db_per_octave() == db)
        .map(|slope| slope_value(*slope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::Param;

    #[test]
    fn the_ids_are_complete_unique_permanent_and_146() {
        let params = EqParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(ids.len(), 146);
        let mut want: Vec<&str> = (0..SLOTS).flat_map(|slot| *band_ids(slot)).collect();
        want.extend(GLOBAL_IDS);
        assert_eq!(want.len(), 146);
        for id in &want {
            assert!(ids.iter().any(|other| other == id), "missing {id}");
        }
        for (index, id) in ids.iter().enumerate() {
            assert_eq!(
                ids.iter().position(|other| other == id),
                Some(index),
                "duplicate {id}"
            );
        }
        // The permanent spellings, first and last slot.
        assert_eq!(
            *band_ids(0),
            ["type_1", "bypass_1", "freq_1", "gain_1", "q_1", "slope_1"]
        );
        assert_eq!(band_ids(23)[5], "slope_24");
    }

    #[test]
    fn a_slot_is_named_by_its_number() {
        let params = EqParams::default();
        assert_eq!(params.bands[6].freq.name(), "Band 7 frequency");
        assert_eq!(params.bands[23].kind.name(), "Band 24 type");
        assert_eq!(params.output.name(), "Output gain");
    }

    #[test]
    fn the_stepped_values_keep_their_normalised_places() {
        // A saved project holds these numbers. Moving any of them remaps every saved band.
        let band = &EqParams::default().bands[0];
        for value in 0..=TYPE_MAX {
            let normalized = band.kind.preview_normalized(value);
            assert_eq!(normalized, value as f32 / TYPE_MAX as f32, "type {value}");
            assert_eq!(band.kind.preview_plain(normalized), value);
        }
        for value in 0..=SLOPE_MAX {
            let normalized = band.slope.preview_normalized(value);
            assert_eq!(normalized, value as f32 / SLOPE_MAX as f32, "slope {value}");
            assert_eq!(band.slope.preview_plain(normalized), value);
        }
        let shipped: Vec<(i32, Option<Shape>)> = (0..=8).map(|v| (v, shape_of(v))).collect();
        assert_eq!(
            shipped,
            vec![
                (0, None),
                (1, Some(Shape::Bell)),
                (2, Some(Shape::LowShelf)),
                (3, Some(Shape::HighShelf)),
                (4, Some(Shape::LowCut)),
                (5, Some(Shape::HighCut)),
                (6, Some(Shape::Notch)),
                (7, Some(Shape::BandPass)),
                (8, Some(Shape::TiltShelf)),
            ]
        );
        let slopes: Vec<u16> = (0..8)
            .map(|v| slope_of(v).unwrap().db_per_octave())
            .collect();
        assert_eq!(slopes, [6, 12, 18, 24, 36, 48, 72, 96]);
        assert_eq!(slope_of(8), None);
        assert_eq!(shape_of(9), None);
    }

    #[test]
    fn an_unassigned_value_reads_as_unavailable_and_plays_as_its_default() {
        let params = EqParams::default();
        let band = &params.bands[0];
        assert_eq!(
            band.kind.normalized_value_to_string(9.0 / 31.0, false),
            "Unavailable 9"
        );
        assert_eq!(
            band.slope.normalized_value_to_string(12.0 / 15.0, false),
            "Unavailable 12"
        );
        assert_eq!(
            shape_of(TYPE_MAX),
            None,
            "an unassigned type is Off to the DSP"
        );
        assert_eq!(slope_of(SLOPE_MAX).unwrap_or(DEFAULT_SLOPE), Slope::Db12);
    }

    /// Every parameter's text survives the host's round trip, across the collection's grids and
    /// switch points (`mxm_plugin_test::text_checks`), and Q's own switch from two decimals to one.
    /// The stepped values must also parse back to themselves, `Unavailable n` included.
    #[test]
    fn every_reading_survives_the_hosts_text_round_trip() {
        mxm_plugin_test::text_checks::every_text_round_trips(&EqParams::default(), &[9.995]);
        let band = &EqParams::default().bands[0];
        for (param, max) in [(&band.kind, TYPE_MAX), (&band.slope, SLOPE_MAX)] {
            for value in 0..=max {
                let text = param.normalized_value_to_string(param.preview_normalized(value), false);
                let parsed = param.string_to_normalized_value(&text);
                assert_eq!(parsed, Some(param.preview_normalized(value)), "{text}");
            }
        }
    }

    #[test]
    fn a_slot_reads_into_the_engines_band() {
        let params = EqParams::default();
        let band = params.bands[3].band();
        assert_eq!(band.shape, None, "every slot starts Off");
        assert_eq!(band.slope, DEFAULT_SLOPE);
        assert_eq!(band.freq, 1_000.0);
        assert_eq!(band.gain_db, 0.0);
        assert_eq!(band.q, 1.0);
        assert!(!band.bypass);
    }
}
