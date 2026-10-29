//! The host surface (`docs/briefs/mxm-fx-eq-n.md` A1 and A4; plan §2.1).
//!
//! **Twenty-four band slots, two globals, each slot's dynamics, side-chain switch and placement,
//! the phase mode and its resolution, and auto gain: 293 parameters.** The slots are
//! nested, so their ids are `<field>_<slot>`: `type_1`, `bypass_1`, `freq_1`, `gain_1`, `q_1`,
//! `slope_1`, through `slope_24`. The globals are `output` and `gainscale`. F1's dynamics (plan
//! §7.1) come after them, four per slot: `range_1`, `threshold_1`, `attack_1`, `release_1`, through
//! `release_24`, so no v1 id moved, the side-chain switches after those (plan §7.2), `key_1`
//! through `key_24`, the placements after those (plan §7.3), `place_1` through `place_24`, then
//! F6's `phase` and `resolution` (plan §7.7), not automatable, and last F7's `autogain` (plan §7.8).
//! The tests pin every id; during pre-alpha one may change (the owner, 2026-10-16), from the first
//! release none may. A host keeps its parameter names from
//! load, so a slot is named by its number ("Band 7 frequency"), never by its shape.
//!
//! **The stepped parameters reserve a fixed domain** (plan §2.1). Projects
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

use mxm_fx_eq_n_dsp::dynamics::{
    ATTACK_MS_MAX, ATTACK_MS_MIN, RANGE_DB_MAX, RELEASE_MS_MAX, RELEASE_MS_MIN, THRESHOLD_DB_MAX,
    THRESHOLD_DB_MIN,
};
use mxm_fx_eq_n_dsp::{
    Band, Dynamics, FREQ_MAX, FREQ_MIN, GAIN_RANGE, GAIN_SCALE_MAX, OUTPUT_GAIN_RANGE_DB, Phase,
    Placement, Q_MAX, Q_MIN, Resolution, SLOTS, Shape, Slope,
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

/// A slot's dynamics fields, in declaration order: the suffixes of their ids (plan §7.1).
pub const DYNAMICS_FIELDS: [&str; 4] = ["range", "threshold", "attack", "release"];

/// Where the dynamics start in [`EqParams::erased`]'s order: after every slot and the globals.
pub const DYNAMICS_BASE: usize = SLOTS * BAND_FIELDS.len() + GLOBAL_IDS.len();

/// Where the side-chain switches start in [`EqParams::erased`]'s order, one per slot: after the
/// dynamics (plan §7.2).
pub const KEY_BASE: usize = DYNAMICS_BASE + SLOTS * DYNAMICS_FIELDS.len();

/// Where the placements start in [`EqParams::erased`]'s order, one per slot (plan §7.3).
pub const PLACE_BASE: usize = KEY_BASE + SLOTS;

/// Where the phase mode sits in [`EqParams::erased`]'s order, its resolution after it (plan §7.7).
pub const PHASE_INDEX: usize = PLACE_BASE + SLOTS;
pub const RESOLUTION_INDEX: usize = PHASE_INDEX + 1;
/// Where auto gain sits, last (plan §7.8).
pub const AUTO_GAIN_INDEX: usize = RESOLUTION_INDEX + 1;

/// Every host parameter.
pub const PARAMETERS: usize = AUTO_GAIN_INDEX + 1;

/// The phase mode's stepped domain, reserved: 0 Zero latency, 1 Linear phase, 2–7 unassigned,
/// heard as Zero latency (plan §7.7).
pub const PHASE_MAX: i32 = 7;
/// The resolution's: 0 Low, 1 Medium, 2 High, 3–7 unassigned, heard as Medium.
pub const RESOLUTION_MAX: i32 = 7;

/// The phase modes' names at their values.
pub const PHASE_NAMES: [&str; 2] = ["Zero latency", "Linear phase"];

/// A resolution's name.
pub fn resolution_name(resolution: Resolution) -> &'static str {
    match resolution {
        Resolution::Low => "Low",
        Resolution::Medium => "Medium",
        Resolution::High => "High",
    }
}

/// The resolution at a stored value: an unassigned one is Medium.
pub fn resolution_of(value: i32) -> Resolution {
    usize::try_from(value)
        .ok()
        .and_then(|index| Resolution::ALL.get(index).copied())
        .unwrap_or_default()
}

/// The phase at the stored mode and resolution: an unassigned mode is Zero latency.
pub fn phase_of(mode: i32, resolution: i32) -> Phase {
    if mode == 1 {
        Phase::Linear(resolution_of(resolution))
    } else {
        Phase::ZeroLatency
    }
}

/// The stored values of a phase: its mode and, in Linear phase, its resolution.
pub fn phase_values(phase: Phase) -> (i32, Option<i32>) {
    match phase {
        Phase::ZeroLatency => (0, None),
        Phase::Linear(resolution) => (
            1,
            Resolution::ALL
                .iter()
                .position(|other| *other == resolution)
                .map(|index| index as i32),
        ),
    }
}

/// A stepped parameter's reading and parsing, as nice-plug takes them.
type ValueToString = Arc<dyn Fn(i32) -> String + Send + Sync>;
type StringToValue = Arc<dyn Fn(&str) -> Option<i32> + Send + Sync>;

fn stepped_text(names: &'static [&'static str]) -> ValueToString {
    Arc::new(move |value| {
        match usize::try_from(value)
            .ok()
            .and_then(|index| names.get(index))
        {
            Some(name) => (*name).to_owned(),
            None => format!("Unavailable {value}"),
        }
    })
}

fn stepped_parse(names: &'static [&'static str]) -> StringToValue {
    Arc::new(move |text| {
        let text = text.trim();
        if let Some(index) = names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(text))
        {
            return Some(index as i32);
        }
        text.strip_prefix("Unavailable ")
            .and_then(|number| number.parse().ok())
    })
}

const RESOLUTION_NAMES: [&str; 3] = ["Low", "Medium", "High"];

/// The placement's stepped domain, reserved now as `type`'s and `slope`'s are: 0 Stereo, 1 Left,
/// 2 Right, 3 Mid, 4 Side, 5–7 unassigned (plan §7.3).
pub const PLACE_MAX: i32 = 7;

/// The placement at a stored value: an unassigned one is Stereo.
pub fn placement_of(value: i32) -> Placement {
    usize::try_from(value)
        .ok()
        .and_then(|index| Placement::ALL.get(index).copied())
        .unwrap_or(Placement::Stereo)
}

/// The stored value of a placement.
pub fn placement_value(placement: Placement) -> i32 {
    Placement::ALL
        .iter()
        .position(|other| *other == placement)
        .map_or(0, |index| index as i32)
}

fn placement_text(value: i32) -> String {
    match usize::try_from(value)
        .ok()
        .and_then(|index| Placement::ALL.get(index))
    {
        Some(placement) => placement_name(*placement).to_owned(),
        None => format!("Unavailable {value}"),
    }
}

/// A placement's name, as the host and the editor show it.
pub fn placement_name(placement: Placement) -> &'static str {
    match placement {
        Placement::Stereo => "Stereo",
        Placement::Left => "Left",
        Placement::Right => "Right",
        Placement::Mid => "Mid",
        Placement::Side => "Side",
    }
}

fn placement_parse(text: &str) -> Option<i32> {
    let text = text.trim();
    if let Some(placement) = Placement::ALL
        .iter()
        .find(|placement| placement_name(**placement).eq_ignore_ascii_case(text))
    {
        return Some(placement_value(*placement));
    }
    text.strip_prefix("Unavailable ")
        .and_then(|number| number.parse().ok())
}

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

/// One slot's dynamics (plan §7.1; brief §C): static while Range is 0.
#[derive(Params)]
pub struct DynamicsParams {
    /// How far the band's gain moves with its level: below 0 it cuts what gets loud, above 0 it
    /// lifts it.
    #[id = "range"]
    pub range: FloatParam,
    #[id = "threshold"]
    pub threshold: FloatParam,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
}

impl DynamicsParams {
    pub(crate) fn new(index: usize) -> Self {
        let n = index + 1;
        let defaults = Dynamics::STATIC;
        let db = |name: String, default: f32, min: f64, max: f64| {
            FloatParam::new(
                name,
                default,
                FloatRange::Linear {
                    min: min as f32,
                    max: max as f32,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1))
            .with_string_to_value(Arc::new(|text| {
                text.trim().trim_end_matches("dB").trim().parse().ok()
            }))
        };
        Self {
            range: db(
                format!("Band {n} range"),
                defaults.range_db,
                -RANGE_DB_MAX,
                RANGE_DB_MAX,
            ),
            threshold: db(
                format!("Band {n} threshold"),
                defaults.threshold_db,
                THRESHOLD_DB_MIN,
                THRESHOLD_DB_MAX,
            ),
            attack: FloatParam::new(
                format!("Band {n} attack"),
                defaults.attack_ms,
                FloatRange::Skewed {
                    min: ATTACK_MS_MIN as f32,
                    max: ATTACK_MS_MAX as f32,
                    // 10 ms near the middle of the travel, as a log axis would put it.
                    factor: FloatRange::skew_factor(-2.1),
                },
            )
            .with_value_to_string(Arc::new(attack_text))
            .with_string_to_value(Arc::new(milliseconds_parse)),
            release: FloatParam::new(
                format!("Band {n} release"),
                defaults.release_ms,
                FloatRange::Skewed {
                    min: RELEASE_MS_MIN as f32,
                    max: RELEASE_MS_MAX as f32,
                    // About 140 ms in the middle of the travel.
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_value_to_string(Arc::new(release_text))
            .with_string_to_value(Arc::new(milliseconds_parse)),
        }
    }

    /// The slot's dynamics as the engine takes them.
    pub fn dynamics(&self) -> Dynamics {
        Dynamics {
            range_db: self.range.value(),
            threshold_db: self.threshold.value(),
            attack_ms: self.attack.value(),
            release_ms: self.release.value(),
        }
    }

    /// The slot's dynamics parameters, in the order of [`DYNAMICS_FIELDS`].
    pub fn parameters(&self) -> [&dyn mxm_params::ErasedParam; 4] {
        [&self.range, &self.threshold, &self.attack, &self.release]
    }
}

/// One slot's side-chain switch (plan §7.2): Off, the band's dynamics hear its own input; On, the
/// side-chain input.
#[derive(Params)]
pub struct KeyParams {
    #[id = "key"]
    pub side_chain: BoolParam,
}

impl KeyParams {
    pub(crate) fn new(index: usize) -> Self {
        Self {
            side_chain: BoolParam::new(format!("Band {} side chain", index + 1), false),
        }
    }
}

/// One slot's placement in the stereo image (plan §7.3).
#[derive(Params)]
pub struct PlaceParams {
    #[id = "place"]
    pub placement: IntParam,
}

impl PlaceParams {
    pub(crate) fn new(index: usize) -> Self {
        Self {
            placement: IntParam::new(
                format!("Band {} placement", index + 1),
                0,
                IntRange::Linear {
                    min: 0,
                    max: PLACE_MAX,
                },
            )
            .with_value_to_string(Arc::new(placement_text))
            .with_string_to_value(Arc::new(placement_parse)),
        }
    }

    /// The slot's placement as the engine takes it.
    pub fn placement(&self) -> Placement {
        placement_of(self.placement.value())
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
    /// F1's dynamics, after the globals so that no v1 id moved (plan §7.1). Grouped with their
    /// slots ("Band 7") in a host.
    #[nested(array, group = "Band")]
    pub dynamics: [DynamicsParams; SLOTS],
    /// The side-chain switches, after the dynamics so that nothing moved (plan §7.2).
    #[nested(array, group = "Band")]
    pub keys: [KeyParams; SLOTS],
    /// The placements, after the side-chain switches (plan §7.3).
    #[nested(array, group = "Band")]
    pub places: [PlaceParams; SLOTS],
    /// The phase mode, last (plan §7.7): not automatable, since a change restarts the plugin.
    #[id = "phase"]
    pub phase: IntParam,
    /// Linear phase's resolution: its kernel's length, and so its latency.
    #[id = "resolution"]
    pub resolution: IntParam,
    /// Auto gain, last (plan §7.8): the loudness the curve adds or takes, estimated from it and
    /// undone at the output. Off by default.
    #[id = "autogain"]
    pub auto_gain: BoolParam,
    #[persist = "preset"]
    pub preset: RwLock<mxm_preset::PresetIdentity>,
    /// The analyser's settings: view state, saved with the project, never a preset's (plan §4).
    /// Each field below whose restore may install other than what was saved names its key to the
    /// wrapper ([`VIEW_KEY`], [`IDENTITY_KEY`], [`LEARNED_KEY`]).
    #[persist = "view"]
    pub view: crate::view::ViewField,
    /// The instance's id (plan §7.8): host state beside the view settings, never a preset's, so an
    /// editor showing this track finds it again after a reload.
    #[persist = "instance"]
    pub identity: crate::instances::IdentityField,
    /// What Match learned of this track, and what it matches it to (plan §7.6): host state beside
    /// the view settings, never a preset's.
    #[persist = "match-learned"]
    pub learned: crate::learned::LearnedField,
    #[persist = "match-to"]
    pub match_to: RwLock<crate::learned::MatchTo>,
}

/// The persisted keys of the fields whose restore may install other than what was saved, each the
/// `#[persist]` name above: an id another instance holds, settings or a learn from before a field
/// they now have. nice-plug's wrapper rolls back a whole restore, bands and all, when a field reads
/// back changed and its key did not say so (its `PATCHES.md`, defects 9 and 11); the host test
/// `a_restore_that_reads_back_changed_keeps_its_bands` holds it.
pub const VIEW_KEY: &str = "view";
pub const IDENTITY_KEY: &str = "instance";
pub const LEARNED_KEY: &str = "match-learned";

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
            dynamics: std::array::from_fn(DynamicsParams::new),
            keys: std::array::from_fn(KeyParams::new),
            places: std::array::from_fn(PlaceParams::new),
            phase: IntParam::new(
                "Phase",
                0,
                IntRange::Linear {
                    min: 0,
                    max: PHASE_MAX,
                },
            )
            .non_automatable()
            .with_value_to_string(stepped_text(&PHASE_NAMES))
            .with_string_to_value(stepped_parse(&PHASE_NAMES)),
            resolution: IntParam::new(
                "Resolution",
                1,
                IntRange::Linear {
                    min: 0,
                    max: RESOLUTION_MAX,
                },
            )
            .non_automatable()
            .with_value_to_string(stepped_text(&RESOLUTION_NAMES))
            .with_string_to_value(stepped_parse(&RESOLUTION_NAMES)),
            auto_gain: BoolParam::new("Auto gain", false),
            preset: RwLock::new(mxm_preset::PresetIdentity::default()),
            view: crate::view::ViewField::default(),
            identity: crate::instances::IdentityField::default(),
            learned: crate::learned::LearnedField::default(),
            match_to: RwLock::new(crate::learned::MatchTo::default()),
        }
    }
}

impl EqParams {
    /// Every host parameter, in the order the undo history's indices name them: each slot's
    /// fields in [`BAND_FIELDS`] order, slot by slot, then [`GLOBAL_IDS`], then each slot's
    /// dynamics in [`DYNAMICS_FIELDS`] order from [`DYNAMICS_BASE`], then each slot's side-chain
    /// switch from [`KEY_BASE`], then each slot's placement from [`PLACE_BASE`].
    pub fn erased(&self) -> Vec<&dyn mxm_params::ErasedParam> {
        let mut all: Vec<&dyn mxm_params::ErasedParam> = Vec::with_capacity(PARAMETERS);
        for band in &self.bands {
            all.extend(band.parameters());
        }
        all.push(&self.output);
        all.push(&self.gain_scale);
        for dynamics in &self.dynamics {
            all.extend(dynamics.parameters());
        }
        for key in &self.keys {
            all.push(&key.side_chain);
        }
        for place in &self.places {
            all.push(&place.placement);
        }
        all.push(&self.phase);
        all.push(&self.resolution);
        all.push(&self.auto_gain);
        all
    }

    /// The phase the parameters ask for (plan §7.7).
    pub fn phase_mode(&self) -> Phase {
        phase_of(self.phase.value(), self.resolution.value())
    }

    /// [`erased`](Self::erased)'s parameter at `index`, without building the list: the editor
    /// reads parameters one at a time, every frame.
    pub fn param(&self, index: usize) -> &dyn mxm_params::ErasedParam {
        const FIELDS: usize = BAND_FIELDS.len();
        const DYNAMICS: usize = DYNAMICS_FIELDS.len();
        match index {
            index if index < SLOTS * FIELDS => {
                self.bands[index / FIELDS].parameters()[index % FIELDS]
            }
            index if index == SLOTS * FIELDS => &self.output,
            index if index < DYNAMICS_BASE => &self.gain_scale,
            index if index < KEY_BASE => {
                let at = index - DYNAMICS_BASE;
                self.dynamics[at / DYNAMICS].parameters()[at % DYNAMICS]
            }
            index if index < PLACE_BASE => &self.keys[index - KEY_BASE].side_chain,
            index if index < PHASE_INDEX => &self.places[index - PLACE_BASE].placement,
            PHASE_INDEX => &self.phase,
            RESOLUTION_INDEX => &self.resolution,
            _ => &self.auto_gain,
        }
    }

    /// [`erased`](Self::erased)'s parameters as nice-plug's pointers.
    pub fn pointers(&self) -> Vec<nice_plug::params::internals::ParamPtr> {
        let mut all = Vec::with_capacity(PARAMETERS);
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
        for dynamics in &self.dynamics {
            all.extend([
                dynamics.range.as_ptr(),
                dynamics.threshold.as_ptr(),
                dynamics.attack.as_ptr(),
                dynamics.release.as_ptr(),
            ]);
        }
        for key in &self.keys {
            all.push(key.side_chain.as_ptr());
        }
        for place in &self.places {
            all.push(place.placement.as_ptr());
        }
        all.push(self.phase.as_ptr());
        all.push(self.resolution.as_ptr());
        all.push(self.auto_gain.as_ptr());
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

/// Every slot's dynamics ids, in declaration order: `range_1` … `release_24`.
static DYNAMICS_IDS: LazyLock<[[&'static str; 4]; SLOTS]> = LazyLock::new(|| {
    std::array::from_fn(|slot| {
        DYNAMICS_FIELDS.map(|field| &*Box::leak(format!("{field}_{}", slot + 1).into_boxed_str()))
    })
});

/// Slot `slot`'s (0-based) permanent dynamics ids, in the order of [`DYNAMICS_FIELDS`].
pub fn dynamics_ids(slot: usize) -> &'static [&'static str; 4] {
    &DYNAMICS_IDS[slot]
}

/// Whether `id` is one of F1's dynamics ids, which an older preset or state does not hold.
pub fn is_dynamics_id(id: &str) -> bool {
    DYNAMICS_IDS.iter().any(|ids| ids.contains(&id))
}

/// Every slot's side-chain switch id, `key_1` … `key_24` (plan §7.2).
static KEY_IDS: LazyLock<[&'static str; SLOTS]> = LazyLock::new(|| {
    std::array::from_fn(|slot| &*Box::leak(format!("key_{}", slot + 1).into_boxed_str()))
});

/// Every slot's side-chain switch id, slot by slot.
pub fn key_ids() -> &'static [&'static str; SLOTS] {
    &KEY_IDS
}

/// Whether `id` is a side-chain switch, which a preset or state from before §7.2 does not hold.
pub fn is_key_id(id: &str) -> bool {
    KEY_IDS.contains(&id)
}

/// Every slot's placement id, `place_1` … `place_24` (plan §7.3).
static PLACE_IDS: LazyLock<[&'static str; SLOTS]> = LazyLock::new(|| {
    std::array::from_fn(|slot| &*Box::leak(format!("place_{}", slot + 1).into_boxed_str()))
});

/// Every slot's placement id, slot by slot.
pub fn place_ids() -> &'static [&'static str; SLOTS] {
    &PLACE_IDS
}

/// Whether `id` is a placement, which a preset or state from before §7.3 does not hold.
pub fn is_place_id(id: &str) -> bool {
    PLACE_IDS.contains(&id)
}

/// The phase mode's and resolution's ids (plan §7.7).
pub const PHASE_IDS: [&str; 2] = ["phase", "resolution"];

/// Whether `id` is the phase mode or its resolution, which a preset or state from before F6 does
/// not hold.
pub fn is_phase_id(id: &str) -> bool {
    PHASE_IDS.contains(&id)
}

/// Auto gain's id (plan §7.8), which a preset or state from before F7 does not hold.
pub const AUTO_GAIN_ID: &str = "autogain";

/// Whether `id` is auto gain's.
pub fn is_auto_gain_id(id: &str) -> bool {
    id == AUTO_GAIN_ID
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

/// `0.5 ms`, `9.9 ms`, then whole milliseconds, `10 ms` to `200 ms`. The precision is chosen from
/// the reading rounded to tenths, so the host's round trip is idempotent at the switch.
fn attack_text(ms: f32) -> String {
    let tenths = (ms * 10.0).round() / 10.0;
    if tenths < 10.0 {
        format!("{tenths:.1} ms")
    } else {
        format!("{ms:.0} ms")
    }
}

/// Whole milliseconds, `10 ms` to `999 ms`, then seconds, `1.00 s` to `2.00 s`, the unit chosen
/// from the rounded milliseconds (`mxm_params::text::milliseconds_then_seconds`' rule, in
/// milliseconds).
fn release_text(ms: f32) -> String {
    if ms.round() >= 1_000.0 {
        format!("{:.2} s", ms / 1_000.0)
    } else {
        format!("{ms:.0} ms")
    }
}

/// A typed time back to milliseconds: `250 ms` and `0.25 s` read the same, and a bare number is
/// milliseconds.
fn milliseconds_parse(text: &str) -> Option<f32> {
    let text = text.trim().to_lowercase();
    let (number, scale) = if let Some(rest) = text.strip_suffix("ms") {
        (rest, 1.0)
    } else if let Some(rest) = text.strip_suffix('s') {
        (rest, 1_000.0)
    } else {
        (text.as_str(), 1.0)
    };
    number.trim().parse::<f32>().ok().map(|value| value * scale)
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
    fn the_ids_are_complete_unique_and_293() {
        let params = EqParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(ids.len(), PARAMETERS);
        let mut want: Vec<&str> = (0..SLOTS).flat_map(|slot| *band_ids(slot)).collect();
        want.extend(GLOBAL_IDS);
        want.extend((0..SLOTS).flat_map(|slot| *dynamics_ids(slot)));
        want.extend(key_ids());
        want.extend(place_ids());
        want.extend(PHASE_IDS);
        want.push(AUTO_GAIN_ID);
        assert_eq!(want.len(), 293);
        // In the host's order: every v1 id where it was, the dynamics after them.
        assert_eq!(ids[..DYNAMICS_BASE], want[..DYNAMICS_BASE]);
        assert_eq!(ids[DYNAMICS_BASE..KEY_BASE], want[DYNAMICS_BASE..KEY_BASE]);
        assert_eq!(ids[KEY_BASE..PLACE_BASE], want[KEY_BASE..PLACE_BASE]);
        assert_eq!(ids[PLACE_BASE..PHASE_INDEX], want[PLACE_BASE..PHASE_INDEX]);
        assert_eq!(ids[PHASE_INDEX..], ["phase", "resolution", "autogain"]);
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
        assert_eq!(
            *dynamics_ids(0),
            ["range_1", "threshold_1", "attack_1", "release_1"]
        );
        assert_eq!(dynamics_ids(23)[3], "release_24");
        assert!(is_dynamics_id("attack_7") && !is_dynamics_id("gain_7"));
        assert_eq!((key_ids()[0], key_ids()[23]), ("key_1", "key_24"));
        assert!(is_key_id("key_7") && !is_key_id("attack_7") && !is_dynamics_id("key_7"));
        assert_eq!((place_ids()[0], place_ids()[23]), ("place_1", "place_24"));
        assert!(is_place_id("place_7") && !is_place_id("key_7") && !is_key_id("place_7"));
        assert!(is_phase_id("phase") && is_phase_id("resolution") && !is_phase_id("place_7"));
    }

    /// The editor's indices name the same parameters as the host's order.
    #[test]
    fn the_editor_indices_follow_the_host_order() {
        let params = EqParams::default();
        let erased = params.erased();
        assert_eq!(erased.len(), PARAMETERS);
        for (index, param) in erased.iter().enumerate() {
            assert_eq!(params.param(index).name(), param.name(), "{index}");
        }
        assert_eq!(params.param(DYNAMICS_BASE).name(), "Band 1 range");
        assert_eq!(
            params.param(DYNAMICS_BASE + 4 * 6 + 3).name(),
            "Band 7 release"
        );
        assert_eq!(params.param(KEY_BASE + 6).name(), "Band 7 side chain");
        assert_eq!(params.param(PLACE_BASE + 6).name(), "Band 7 placement");
        assert_eq!(params.param(PHASE_INDEX).name(), "Phase");
        assert_eq!(params.param(RESOLUTION_INDEX).name(), "Resolution");
        assert_eq!(params.param(AUTO_GAIN_INDEX).name(), "Auto gain");
        assert_eq!(params.pointers().len(), PARAMETERS);
    }

    #[test]
    fn a_slot_is_named_by_its_number() {
        let params = EqParams::default();
        assert_eq!(params.bands[6].freq.name(), "Band 7 frequency");
        assert_eq!(params.bands[23].kind.name(), "Band 24 type");
        assert_eq!(params.output.name(), "Output gain");
        assert_eq!(params.dynamics[6].threshold.name(), "Band 7 threshold");
    }

    #[test]
    fn the_stepped_values_keep_their_normalised_places() {
        // A saved project holds these numbers. Moving any of them remaps every saved band.
        let params = EqParams::default();
        for (param, max) in [
            (&params.phase, PHASE_MAX),
            (&params.resolution, RESOLUTION_MAX),
        ] {
            for value in 0..=max {
                let normalized = param.preview_normalized(value);
                assert_eq!(
                    normalized,
                    value as f32 / max as f32,
                    "{} {value}",
                    param.name()
                );
                assert_eq!(param.preview_plain(normalized), value);
            }
        }
        // The phase heard at each value: unassigned ones are Zero latency and Medium (plan §7.7).
        assert_eq!(phase_of(0, 2), Phase::ZeroLatency);
        assert_eq!(phase_of(1, 0), Phase::Linear(Resolution::Low));
        assert_eq!(phase_of(1, 2), Phase::Linear(Resolution::High));
        assert_eq!(phase_of(1, 6), Phase::Linear(Resolution::Medium));
        assert_eq!(phase_of(5, 0), Phase::ZeroLatency);
        assert_eq!(
            (params.phase.to_string(), params.resolution.to_string()),
            ("Zero latency".to_owned(), "Medium".to_owned())
        );
        assert!(params.phase.flags().contains(ParamFlags::NON_AUTOMATABLE));
        assert!(
            params
                .resolution
                .flags()
                .contains(ParamFlags::NON_AUTOMATABLE)
        );
        let band = &params.bands[0];
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
        // The placements (plan §7.3): 0–7, five assigned, the rest Stereo to the DSP.
        let place = &EqParams::default().places[0];
        for value in 0..=PLACE_MAX {
            let normalized = place.placement.preview_normalized(value);
            assert_eq!(normalized, value as f32 / PLACE_MAX as f32, "place {value}");
            assert_eq!(place.placement.preview_plain(normalized), value);
        }
        let placements: Vec<Placement> = (0..=PLACE_MAX).map(placement_of).collect();
        assert_eq!(
            placements,
            [
                Placement::Stereo,
                Placement::Left,
                Placement::Right,
                Placement::Mid,
                Placement::Side,
                Placement::Stereo,
                Placement::Stereo,
                Placement::Stereo,
            ]
        );
        assert_eq!(place.placement.to_string(), "Stereo");
        assert_eq!(placement_text(6), "Unavailable 6");
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
        // Q's switch to one decimal, Attack's to whole milliseconds, Release's to seconds.
        mxm_plugin_test::text_checks::every_text_round_trips(
            &EqParams::default(),
            &[9.995, 9.95, 999.5],
        );
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
        assert_eq!(
            params.dynamics[3].dynamics(),
            Dynamics::STATIC,
            "every slot starts static"
        );
    }

    #[test]
    fn the_dynamics_readings() {
        assert_eq!(attack_text(0.5), "0.5 ms");
        assert_eq!(attack_text(9.96), "10 ms");
        assert_eq!(attack_text(200.0), "200 ms");
        assert_eq!(release_text(150.0), "150 ms");
        assert_eq!(release_text(999.6), "1.00 s");
        assert_eq!(milliseconds_parse("0.25 s"), Some(250.0));
        assert_eq!(milliseconds_parse("12"), Some(12.0));
    }
}
