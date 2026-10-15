//! The columns to the graph's right (plan §5.1, decision 30): the Band card for the selected band
//! alone in the first, then Output, Analyser, Resonances (plan §7.5) and Match (plan §7.6, decision
//! 34) in the second. Each body is
//! a `mxm_ui::tree`, measured and drawn from one description, and every
//! control is the collection's own, through the shared binding where it edits one host value.
//! Every card is one of the cursor's (`navigation::bar_card`), and every control joins it.
//!
//! The Band card is reserved to its tallest shape, so changing a band's shape, or selecting none,
//! moves nothing below it. With no band selected it has no subject, and it says so with its title
//! alone ("Band —"): no sentence on the panel (`plugins/AGENTS.md`, *No help text*). Its hover says
//! how to get one.

use egui::{Rect, Ui, vec2};
use mxm_fx_eq_n_dsp::{Shape, Slope};
use mxm_params::StepLaw;
use mxm_params::binding::{Bound, toggle_labelled};
use mxm_ui::navigation;
use mxm_ui::space::SPACE_3;
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{self, Kind, Node, leaf, row_gap, stack};
use nice_plug::prelude::Param;

use super::{
    ANALYSER_CARD, BAND_CARD, EqApp, Field, FindRefusal, MATCH_CARD, OUTPUT_CARD, RESONANCES_CARD,
    SOLO_KEY,
};
use crate::params::{EqParams, SHAPES, band_ids, shape_name};
use crate::view::{Resolution, Show, Speed, Tilt, ViewSettings};

/// The second column's titles, top to bottom.
pub const OUTPUT_TITLE: &str = "Output";
pub const ANALYSER_TITLE: &str = "Analyser";
pub const RESONANCES_TITLE: &str = "Resonances";
pub const MATCH_TITLE: &str = "Match";

/// A slider in the column.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Slider {
    Frequency,
    Gain,
    Q,
    Output,
    GainScale,
    Range,
    Threshold,
    Attack,
    Release,
}

impl Slider {
    /// A control of the selected band's, rather than of the whole EQ.
    fn of_the_band(self) -> bool {
        !matches!(self, Slider::Output | Slider::GainScale)
    }
}

/// What a leaf of the column's cards draws.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Leaf {
    Bypass,
    Shape,
    Slider(Slider),
    Slope,
    Solo,
    Delete,
    Analyser,
    Show,
    Resolution,
    Speed,
    Tilt,
    /// How far a dynamic band's gain is moving now (plan §7.1).
    Moving,
    /// Whether a dynamic band listens to the side chain (plan §7.2).
    SideChain,
    /// Where the band works in the stereo image (plan §7.3).
    Placement,
    /// A Stereo band into a Left/Right or Mid/Side pair (plan §7.3).
    Split,
    /// The side chain's spectrum as a reference line (plan §7.4).
    SideChainView,
    /// Holds the spectrum so its peaks can be grabbed (plan §7.4).
    Freeze,
    /// Listens for resonances and proposes cuts (plan §7.5).
    Find,
    /// The share of each resonance a proposed cut takes.
    Amount,
    /// Makes every proposed cut as a band.
    ApplyAll,
    /// Listens to the side chain and the input and proposes a match (plan §7.6).
    Match,
    /// The share of the difference from the reference the match takes.
    MatchAmount,
    /// At most how many bands the match proposes.
    Detail,
    /// How long each input has been heard.
    MatchHeard,
    /// Makes the match's bands.
    MatchApply,
    /// Zero latency or Linear phase (plan §7.7).
    PhaseMode,
    /// Linear phase's resolution.
    LinearResolution,
}

const SHAPE_DESCRIPTION: &str = "What the band does to the sound around its frequency.";
const SLOPE_DESCRIPTION: &str = "How steeply the cut falls away past its frequency.";
const BYPASS_DESCRIPTION: &str = "Takes the band out of the sound and keeps its settings.";
const MOVING_DESCRIPTION: &str =
    "How far the band is moving now: left of the middle it cuts, right of it lifts.";
/// The moving meter's height, and the dB from its middle to an end.
const MOVING_HEIGHT: f32 = 10.0;
const MOVING_SPAN_DB: f32 = 30.0;
const SOLO_DESCRIPTION: &str = "Plays only what this band works on, until it is pressed again.";
const SOLO_MONO_DESCRIPTION: &str = "This band works on one part of a stereo image, which a mono track does not have, so there is \
     nothing of it to play.";
const SOLO_KEYED_DESCRIPTION: &str =
    "Plays the side chain as this band hears it, until it is pressed again.";
const SIDE_CHAIN_DESCRIPTION: &str = "The band moves with the side-chain input, heard through the \
     band's own range, instead of its own input. With nothing routed there, the band stays as drawn.";
const NO_BAND_DESCRIPTION: &str =
    "Click a point on the graph to show its band here, or click the line to add one.";
const GAIN_SCALE_DESCRIPTION: &str =
    "Scales every band's gain together; cuts, notches and band passes stay as drawn.";
/// The gain scale's hover while no band in the sound has a gain, when the slider is faded.
const GAIN_SCALE_UNHEARD: &str = concat!(
    "Scales the gain of bells, shelves and tilts. No band in the sound has one, so it ",
    "changes nothing now; cuts, notches and band passes stay as drawn."
);
const DELETE_DESCRIPTION: &str = "Removes the band. Undo brings it back.";
const FREEZE_DESCRIPTION: &str = "Holds the spectrum's highest levels of the last two seconds and \
     marks its peaks. Drag a peak to make a bell there. Any analyser change lets it run again.";
const SIDE_CHAIN_VIEW_DESCRIPTION: &str = "Shows the side-chain input's spectrum as a dashed line, \
     to compare with the sound. With nothing routed to the side chain, the line stays flat.";
const PLACEMENT_DESCRIPTION: &str = "Where the band works: on both channels, on the left or the \
     right alone, or on the middle or the sides of the stereo image. A Mid or Side cut also moves \
     the image a little near its frequency.";
const PLACEMENT_MONO_DESCRIPTION: &str = "This band works on one part of a stereo image, which a \
     mono track does not have, so it is not heard here. Choose Stereo to hear it.";
const SPLIT_DESCRIPTION: &str = "Splits the band into a left and a right copy, to set each on \
     its own. Alt-click splits it into a middle and a sides copy instead.";
const SPLIT_PLACED_DESCRIPTION: &str =
    "This band already works on one channel. Only a Stereo band splits.";
const SPLIT_FULL_DESCRIPTION: &str = "Every band slot is in use, so there is no room for a copy.";
const FIND_DESCRIPTION: &str = "Listens to the sound coming in for resonances, peaks that stay put \
     while the notes move, and proposes a cut for each. Below about 200 Hz a fast bass line can \
     look like one. Needs the analyser.";
const FIND_FULL_DESCRIPTION: &str =
    "Every band slot is in use, so no cut can be proposed. Delete a band to make room.";
const FIND_NO_GAIN_DESCRIPTION: &str =
    "The gain scale is at 0 %, where a bell does nothing, so no cut is proposed.";
const AMOUNT_DESCRIPTION: &str = "How much of each resonance a proposed cut takes away.";
const APPLY_ALL_DESCRIPTION: &str = "Makes every proposed cut a band, in one step Undo takes back.";
/// The Amount slider's key in the cursor's registry and its held value's.
const AMOUNT_KEY: &str = "amount";
const MATCH_DESCRIPTION: &str = "Listens to the sound coming in and to the side chain, and proposes \
     the bands that bring the sound's tone towards the side chain's. Route a reference track to \
     the side chain. Needs the analyser.";
const MATCH_NO_PORT_DESCRIPTION: &str = "Needs a side chain, which this layout does not have. Use \
     the EQ with its side-chain input to match to a reference track.";
const MATCH_FULL_DESCRIPTION: &str =
    "Every band slot is in use, so no band can be proposed. Delete a band to make room.";
const MATCH_NO_GAIN_DESCRIPTION: &str =
    "The gain scale is at 0 %, where a bell or a shelf does nothing, so no band is proposed.";
const MATCH_AMOUNT_DESCRIPTION: &str =
    "How much of the difference from the reference the match takes.";
const DETAIL_DESCRIPTION: &str =
    "At most how many bands the match proposes. Fewer keeps the broadest differences.";
const MATCH_HEARD_DESCRIPTION: &str = "How long the sound coming in and the side chain have been \
     heard. Thirty seconds or more of each makes a steadier match.";
const MATCH_APPLY_DESCRIPTION: &str = "Makes the proposed bands, in one step Undo takes back.";
const MATCH_AMOUNT_KEY: &str = "match amount";
const DETAIL_KEY: &str = "detail";
/// The heard reading's height.
const HEARD_HEIGHT: f32 = 16.0;

/// A slider on view state, not a parameter (plan §7.5, §7.6): Find's Amount, Match's Amount and
/// Detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewSlider {
    FindAmount,
    MatchAmount,
    Detail,
}

/// Detail's steps: one band fine, four coarse.
const DETAIL_SPAN: f64 = (super::matcher::MAX_DETAIL - 1) as f64;

impl ViewSlider {
    fn key(self) -> &'static str {
        match self {
            ViewSlider::FindAmount => AMOUNT_KEY,
            ViewSlider::MatchAmount => MATCH_AMOUNT_KEY,
            ViewSlider::Detail => DETAIL_KEY,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ViewSlider::Detail => "Detail",
            _ => "Amount",
        }
    }

    fn description(self) -> &'static str {
        match self {
            ViewSlider::FindAmount => AMOUNT_DESCRIPTION,
            ViewSlider::MatchAmount => MATCH_AMOUNT_DESCRIPTION,
            ViewSlider::Detail => DETAIL_DESCRIPTION,
        }
    }

    fn widest(self) -> &'static str {
        match self {
            ViewSlider::Detail => "24 bands",
            _ => "100 %",
        }
    }

    fn normalised(self, settings: &ViewSettings) -> f64 {
        match self {
            ViewSlider::FindAmount => f64::from(settings.amount.min(100)) / 100.0,
            ViewSlider::MatchAmount => f64::from(settings.match_amount.min(100)) / 100.0,
            ViewSlider::Detail => f64::from(settings.match_detail() - 1) / DETAIL_SPAN,
        }
    }

    fn default(self) -> f64 {
        self.normalised(&ViewSettings::default())
    }

    fn steps(self) -> mxm_ui::control::Steps {
        match self {
            ViewSlider::Detail => mxm_ui::control::Steps {
                fine_up: 1.0 / DETAIL_SPAN,
                fine_down: 1.0 / DETAIL_SPAN,
                coarse_up: 4.0 / DETAIL_SPAN,
                coarse_down: 4.0 / DETAIL_SPAN,
            },
            _ => mxm_ui::control::Steps::DEFAULT,
        }
    }

    fn text(self, normalised: f64) -> String {
        match self {
            ViewSlider::Detail => match detail_of(normalised) {
                1 => "1 band".to_owned(),
                bands => format!("{bands} bands"),
            },
            _ => format!("{} %", (normalised * 100.0).round() as u32),
        }
    }

    /// A typed value, as the normalised value; `None` for anything else.
    fn parse(self, typed: &str) -> Option<f64> {
        match self {
            ViewSlider::Detail => {
                let number: f64 = typed
                    .trim()
                    .trim_end_matches("bands")
                    .trim_end_matches("band")
                    .trim()
                    .parse()
                    .ok()?;
                number
                    .is_finite()
                    .then(|| (number.round() - 1.0).clamp(0.0, DETAIL_SPAN) / DETAIL_SPAN)
            }
            _ => parse_percent(typed),
        }
    }

    fn write(self, settings: &mut ViewSettings, normalised: f64) {
        let percent = (normalised * 100.0).round().clamp(0.0, 100.0) as u8;
        match self {
            ViewSlider::FindAmount => settings.amount = percent,
            ViewSlider::MatchAmount => settings.match_amount = percent,
            ViewSlider::Detail => settings.match_detail = detail_of(normalised),
        }
    }
}

/// Detail at a normalised value: 1 to 24 bands.
fn detail_of(normalised: f64) -> u8 {
    (1.0 + (normalised.clamp(0.0, 1.0) * DETAIL_SPAN).round()) as u8
}

/// What the Band card offers of placement (plan §7.3): the choice and Split in a stereo layout;
/// in mono nothing for a Stereo band, and the way back to Stereo for a placed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlacementRow {
    Offered,
    Restore,
    Absent,
}

fn slopes() -> Vec<String> {
    Slope::ALL
        .iter()
        .map(|slope| format!("{} dB/oct", slope.db_per_octave()))
        .collect()
}

fn shapes() -> Vec<String> {
    SHAPES
        .iter()
        .map(|shape| shape_name(*shape).to_owned())
        .collect()
}

/// The binding of a slider, labelled for its card.
fn bound(params: &EqParams, slot: usize, slider: Slider) -> Bound<'_> {
    let band = &params.bands[slot];
    let ids = band_ids(slot);
    let dynamics = &params.dynamics[slot];
    let dynamics_ids = crate::params::dynamics_ids(slot);
    match slider {
        Slider::Range => Bound::new(
            dynamics_ids[0],
            &dynamics.range,
            "How far the band moves when it gets loud: below 0 it cuts, above 0 it lifts. At 0 \
             the band stays as drawn.",
        )
        .labelled("Range"),
        Slider::Threshold => Bound::new(
            dynamics_ids[1],
            &dynamics.threshold,
            "How loud the band has to get before it moves.",
        )
        .labelled("Threshold"),
        Slider::Attack => Bound::new(
            dynamics_ids[2],
            &dynamics.attack,
            "How fast the band moves when it gets loud.",
        )
        .labelled("Attack"),
        Slider::Release => Bound::new(
            dynamics_ids[3],
            &dynamics.release,
            "How fast the band comes back when the sound falls.",
        )
        .labelled("Release"),
        Slider::Frequency => Bound::new(
            ids[Field::Freq as usize],
            &band.freq,
            "The frequency the band works around.",
        )
        .labelled("Frequency")
        .law(StepLaw::Hertz),
        Slider::Gain => Bound::new(
            ids[Field::Gain as usize],
            &band.gain,
            "How much the band lifts or lowers.",
        )
        .labelled("Gain"),
        Slider::Q => Bound::new(
            ids[Field::Q as usize],
            &band.q,
            "How narrow the band is: higher is narrower.",
        )
        .labelled("Q"),
        Slider::Output => Bound::new(
            "output",
            &params.output,
            "The level after every band, last in the chain.",
        )
        .labelled("Output"),
        Slider::GainScale => gain_scale(params, GAIN_SCALE_DESCRIPTION),
    }
}

fn gain_scale<'a>(params: &'a EqParams, description: &'static str) -> Bound<'a> {
    Bound::new("gainscale", &params.gain_scale, description).labelled("Gain scale")
}

fn slider_leaf(params: &EqParams, slot: usize, slider: Slider) -> Node<Leaf> {
    let bound = bound(params, slot, slider);
    let param = bound.param;
    leaf(
        Leaf::Slider(slider),
        Kind::Slider {
            label: bound.painted().to_owned(),
            quiet: false,
            widest: mxm_ui::control::widest_value(|n| param.format(n as f32)),
        },
    )
}

fn selector_leaf(key: Leaf, label: &str, options: Vec<String>) -> Node<Leaf> {
    leaf(
        key,
        Kind::Selector {
            label: label.to_owned(),
            options,
            caption: false,
            width: None,
        },
    )
}

fn button_leaf(key: Leaf, label: &str) -> Node<Leaf> {
    leaf(
        key,
        Kind::Button {
            label: label.to_owned(),
            min: vec2(0.0, mxm_ui::space::MIN_TARGET),
            fills: false,
        },
    )
}

/// One shape's Band body, for slot `slot`.
pub(crate) fn band_body(
    params: &EqParams,
    slot: usize,
    shape: Shape,
    slope: Slope,
    placement: PlacementRow,
) -> Node<Leaf> {
    let bypass = leaf(
        Leaf::Bypass,
        Kind::Toggle {
            label: "Bypass".to_owned(),
        },
    );
    // Placement shares Bypass's row, so the card that holds it still opens inside the budget.
    let first = if placement == PlacementRow::Absent {
        bypass
    } else {
        let names = mxm_fx_eq_n_dsp::Placement::ALL
            .map(|placement| crate::params::placement_name(placement).to_owned())
            .to_vec();
        tree::row(vec![
            bypass,
            selector_leaf(Leaf::Placement, "Placement", names),
        ])
    };
    let mut rows = vec![
        first,
        selector_leaf(Leaf::Shape, "Shape", shapes()),
        slider_leaf(params, slot, Slider::Frequency),
    ];
    if shape.has_gain() {
        rows.push(slider_leaf(params, slot, Slider::Gain));
    }
    if shape.has_q(slope) {
        rows.push(slider_leaf(params, slot, Slider::Q));
    }
    if shape.has_slope() {
        rows.push(selector_leaf(Leaf::Slope, "Slope", slopes()));
    }
    if shape.is_dynamic() {
        // Two to a row, so the card that holds them still opens inside the budget (§8).
        for pair in [
            [Slider::Range, Slider::Threshold],
            [Slider::Attack, Slider::Release],
        ] {
            rows.push(tree::row(
                pair.map(|slider| slider_leaf(params, slot, slider)).into(),
            ));
        }
        rows.push(leaf(
            Leaf::SideChain,
            Kind::Toggle {
                label: "Side chain".to_owned(),
            },
        ));
        rows.push(leaf(
            Leaf::Moving,
            Kind::Custom {
                min_width: 0.0,
                height: tree::Height::Fixed(MOVING_HEIGHT),
                fills: true,
            },
        ));
    }
    let mut actions = Vec::new();
    if shape.offers_solo() {
        actions.push(leaf(
            Leaf::Solo,
            Kind::Toggle {
                label: "Bypassed".to_owned(),
            },
        ));
    }
    if placement == PlacementRow::Offered {
        actions.push(button_leaf(Leaf::Split, "Split"));
    }
    actions.push(button_leaf(Leaf::Delete, "Delete band"));
    rows.push(row_gap(SPACE_3, actions));
    stack(rows)
}

/// The Band card's body: the selected band's, reserved to the tallest shape's.
pub fn band_tree(app: &EqApp) -> Node<Leaf> {
    let params = &app.params;
    let alternatives = vec![
        band_body(params, 0, Shape::Bell, Slope::Db12, PlacementRow::Offered),
        band_body(params, 0, Shape::LowCut, Slope::Db12, PlacementRow::Offered),
    ];
    let shown = app
        .selected
        .and_then(|slot| {
            let band = app.band_now(slot);
            let placement = if !app.telemetry.mono() {
                PlacementRow::Offered
            } else if app.placement_heard(slot) {
                PlacementRow::Absent
            } else {
                PlacementRow::Restore
            };
            band.shape
                .map(|shape| band_body(params, slot, shape, band.slope, placement))
        })
        .unwrap_or_else(|| stack(Vec::new()));
    tree::reserve(shown, alternatives)
}

pub fn band_title(app: &EqApp) -> String {
    match app.selected {
        Some(slot) => format!("Band {}", slot + 1),
        None => "Band —".to_owned(),
    }
}

pub fn output_tree(app: &EqApp) -> Node<Leaf> {
    let names = |all: &[&str]| all.iter().map(|name| (*name).to_owned()).collect();
    stack(vec![
        slider_leaf(&app.params, 0, Slider::Output),
        slider_leaf(&app.params, 0, Slider::GainScale),
        selector_leaf(Leaf::PhaseMode, "Phase", names(&crate::params::PHASE_NAMES)),
        selector_leaf(
            Leaf::LinearResolution,
            "Resolution",
            names(&LINEAR_RESOLUTIONS),
        ),
    ])
}

const LINEAR_RESOLUTIONS: [&str; 3] = ["Low", "Medium", "High"];
const PHASE_ZERO_DESCRIPTION: &str = "Zero latency keeps the sound on time, its phase turning near \
     each band as an analogue equaliser's does. Linear phase turns no phase, so a Mid or Side band \
     moves nothing across and a steep cut keeps a hit's shape, at a delay; a narrow low cut then \
     rings before the note as well as after.";
const RESOLUTION_DESCRIPTION: &str = "How finely Linear phase follows narrow bands: Low delays the \
     sound least, High holds a narrow low band best and delays most.";
const PENDING_DESCRIPTION: &str =
    " The change is heard once the host restarts the equaliser, which most do at once.";

const SHOW_OPTIONS: [&str; 3] = ["Pre", "Post", "Both"];
const RESOLUTION_OPTIONS: [&str; 3] = ["Low", "Medium", "High"];
const SPEED_OPTIONS: [&str; 3] = ["Slow", "Medium", "Fast"];
const TILT_OPTIONS: [&str; 4] = ["0 dB/oct", "1.5 dB/oct", "3 dB/oct", "4.5 dB/oct"];

/// The Analyser card. Its toggles share a row, so the opening stays inside the budget (§8); the
/// Side chain view is offered only on a layout with the port (plan §7.4).
pub fn analyser_tree(side_chain_port: bool) -> Node<Leaf> {
    let options = |all: &[&str]| all.iter().map(|option| (*option).to_owned()).collect();
    let toggle = |key: Leaf, label: &str| {
        leaf(
            key,
            Kind::Toggle {
                label: label.to_owned(),
            },
        )
    };
    let mut toggles = vec![
        toggle(Leaf::Analyser, "Analyser"),
        toggle(Leaf::Freeze, "Freeze"),
    ];
    if side_chain_port {
        toggles.push(toggle(Leaf::SideChainView, "Side chain"));
    }
    stack(vec![
        tree::row(toggles),
        selector_leaf(Leaf::Show, "Show", options(&SHOW_OPTIONS)),
        selector_leaf(Leaf::Resolution, "Resolution", options(&RESOLUTION_OPTIONS)),
        selector_leaf(Leaf::Speed, "Speed", options(&SPEED_OPTIONS)),
        selector_leaf(Leaf::Tilt, "Tilt", options(&TILT_OPTIONS)),
    ])
}

/// The Resonances card (plan §7.5): Find and Apply all on a row, Amount under them.
pub fn resonances_tree() -> Node<Leaf> {
    stack(vec![
        tree::row(vec![
            leaf(
                Leaf::Find,
                Kind::Toggle {
                    label: "Find".to_owned(),
                },
            ),
            button_leaf(Leaf::ApplyAll, "Apply all"),
        ]),
        leaf(
            Leaf::Amount,
            Kind::Slider {
                label: "Amount".to_owned(),
                quiet: false,
                widest: "100 %".to_owned(),
            },
        ),
    ])
}

/// The Match card (plan §7.6): Match and Apply on a row, Amount and Detail under them, then how long
/// each input has been heard.
pub fn match_tree() -> Node<Leaf> {
    let slider = |key: Leaf, label: &str, widest: &str| {
        leaf(
            key,
            Kind::Slider {
                label: label.to_owned(),
                quiet: false,
                widest: widest.to_owned(),
            },
        )
    };
    stack(vec![
        tree::row(vec![
            leaf(
                Leaf::Match,
                Kind::Toggle {
                    label: "Match".to_owned(),
                },
            ),
            button_leaf(Leaf::MatchApply, "Apply"),
        ]),
        slider(Leaf::MatchAmount, "Amount", "100 %"),
        slider(Leaf::Detail, "Detail", "24 bands"),
        leaf(
            Leaf::MatchHeard,
            Kind::Custom {
                min_width: 0.0,
                height: tree::Height::Fixed(HEARD_HEIGHT),
                fills: true,
            },
        ),
    ])
}

/// A typed Amount, `35`, `35%` or `35 %`, as a share; `None` for anything else.
fn parse_percent(typed: &str) -> Option<f64> {
    let number: f64 = typed.trim().trim_end_matches('%').trim().parse().ok()?;
    number.is_finite().then(|| (number / 100.0).clamp(0.0, 1.0))
}

const METER_PEAK_HOLD_SECONDS: f64 = 0.450;
const METER_RELEASE_SECONDS: f64 = 0.300;

/// The Out meter's display: an instant attack, a visible 450 ms hold, then a 300 ms release, so a
/// one-block clip stays in the red long enough to be seen. Display only; nothing feeds back.
#[derive(Default)]
pub struct MeterDisplay {
    pub output: [f32; 2],
    hold_until: [f64; 2],
    last_time: Option<f64>,
}

impl MeterDisplay {
    pub fn update(&mut self, now: f64, observed: [f32; 2]) {
        let elapsed = self
            .last_time
            .map_or(0.0, |last| (now - last).clamp(0.0, 0.25));
        self.last_time = Some(now);
        let release = (-elapsed / METER_RELEASE_SECONDS).exp() as f32;
        for ((shown, hold_until), observed) in self
            .output
            .iter_mut()
            .zip(self.hold_until.iter_mut())
            .zip(observed)
        {
            let value = if observed.is_finite() {
                observed.clamp(0.0, 1.0)
            } else {
                0.0
            };
            if value >= *shown && value > 0.0 {
                *shown = value;
                *hold_until = now + METER_PEAK_HOLD_SECONDS;
            } else if now >= *hold_until {
                *shown = value.max(*shown * release);
            }
        }
    }
}

impl EqApp {
    /// The first column: the Band card alone (decision 30).
    pub(crate) fn first_column(&mut self, ui: &mut Ui, tokens: &Tokens) {
        let title = band_title(self);
        // With no band selected the Band card has no controls, so it is no stop of the cursor's:
        // VIEW from the graph would land on nothing and fall back to the graph.
        let absent = self.selected.is_none().then_some(NO_BAND_DESCRIPTION);
        self.card(
            ui,
            tokens,
            BAND_CARD,
            &title,
            |app, _| band_tree(app),
            true,
            absent,
        );
    }

    /// The second column: Output, Analyser, Resonances and Match, top to bottom, each one of the
    /// cursor's.
    pub(crate) fn second_column(&mut self, ui: &mut Ui, tokens: &Tokens) {
        self.card(
            ui,
            tokens,
            OUTPUT_CARD,
            OUTPUT_TITLE,
            |app, _| output_tree(app),
            false,
            None,
        );
        ui.add_space(SPACE_3);
        self.card(
            ui,
            tokens,
            ANALYSER_CARD,
            ANALYSER_TITLE,
            |app, _| analyser_tree(app.telemetry.side_chain_port()),
            false,
            None,
        );
        ui.add_space(SPACE_3);
        self.card(
            ui,
            tokens,
            RESONANCES_CARD,
            RESONANCES_TITLE,
            |_, _| resonances_tree(),
            false,
            None,
        );
        ui.add_space(SPACE_3);
        self.card(
            ui,
            tokens,
            MATCH_CARD,
            MATCH_TITLE,
            |_, _| match_tree(),
            false,
            None,
        );
    }

    /// One card: its tree drawn in a module card the cursor knows (`navigation::bar_card`), with
    /// its outline, or, not a `stop`, out of the cursor's sequence. `reserved` keeps the body as
    /// tall as its tallest alternative.
    #[allow(clippy::too_many_arguments)]
    fn card(
        &mut self,
        ui: &mut Ui,
        tokens: &Tokens,
        key: u64,
        title: &str,
        tree: impl Fn(&EqApp, &Ui) -> Node<Leaf>,
        reserved: bool,
        absent: Option<&'static str>,
    ) {
        if let Some(hover) = absent {
            navigation::bar_card_absent(ui.ctx(), key);
            let rect = self.card_body(ui, tokens, title, tree, reserved);
            ui.interact(
                rect,
                ui.id().with(("absent card", key)),
                egui::Sense::hover(),
            )
            .on_hover_text(hover);
            return;
        }
        navigation::bar_card(ui, key, |ui| {
            let rect = self.card_body(ui, tokens, title, tree, reserved);
            navigation::paint_card(ui, tokens, key, rect);
            rect
        });
    }

    fn card_body(
        &mut self,
        ui: &mut Ui,
        tokens: &Tokens,
        title: &str,
        tree: impl Fn(&EqApp, &Ui) -> Node<Leaf>,
        reserved: bool,
    ) -> Rect {
        ui.scope(|ui| {
            mxm_ui::ModuleCard::new(title).show(ui, tokens, |ui| {
                let body = tree(self, ui);
                let drawn = tree::show(ui, tokens, &body, |ui, leaf, rect| {
                    self.paint_leaf(ui, tokens, *leaf, rect);
                });
                if reserved {
                    // Reserved to the tallest shape, so nothing below moves when the band
                    // or its shape changes: what is not shown leaves its room at the foot.
                    let reserved = body.reserved_height(ui, drawn.width());
                    ui.add_space((reserved - drawn.height()).max(0.0));
                }
            });
        })
        .response
        .rect
    }

    pub(crate) fn paint_leaf(&mut self, ui: &mut Ui, tokens: &Tokens, leaf: Leaf, rect: Rect) {
        let gui = self.gui.clone();
        let Some(gui) = gui else {
            return;
        };
        let setter = gui.param_setter();
        let selected = self.selected.filter(|&slot| self.band_in_use(slot));
        match leaf {
            Leaf::Bypass => {
                if let Some(slot) = selected {
                    let band = &self.params.bands[slot];
                    toggle_labelled(
                        ui,
                        tokens,
                        band_ids(slot)[Field::Bypass as usize],
                        &band.bypass,
                        "Bypass",
                        BYPASS_DESCRIPTION,
                        &setter,
                        0.0,
                    );
                }
            }
            Leaf::SideChain => {
                if let Some(slot) = selected {
                    toggle_labelled(
                        ui,
                        tokens,
                        crate::params::key_ids()[slot],
                        &self.params.keys[slot].side_chain,
                        "Side chain",
                        SIDE_CHAIN_DESCRIPTION,
                        &setter,
                        0.0,
                    );
                }
            }
            Leaf::Placement => {
                if let Some(slot) = selected {
                    use mxm_fx_eq_n_dsp::Placement;
                    let current = self.placement_now(slot);
                    // In mono a placed band is offered only the way back (plan §7.3).
                    let restore = !self.placement_heard(slot);
                    let choices: Vec<Placement> = if restore {
                        vec![current, Placement::Stereo]
                    } else {
                        Placement::ALL.to_vec()
                    };
                    let names: Vec<&str> = choices
                        .iter()
                        .map(|placement| crate::params::placement_name(*placement))
                        .collect();
                    let mut index = choices
                        .iter()
                        .position(|placement| *placement == current)
                        .unwrap_or(0);
                    let stereo = choices.iter().position(|p| *p == Placement::Stereo);
                    let name = self.params.places[slot].placement.name().to_owned();
                    let description = if restore {
                        PLACEMENT_MONO_DESCRIPTION
                    } else {
                        PLACEMENT_DESCRIPTION
                    };
                    let changed = navigation::at(ui, crate::params::place_ids()[slot], |ui| {
                        mxm_ui::control::selector_named(
                            ui,
                            tokens,
                            "Placement",
                            &name,
                            &names,
                            &mut index,
                            None,
                            stereo,
                            description,
                        )
                    });
                    if changed && choices[index] != current {
                        self.set_placement(slot, choices[index]);
                    }
                }
            }
            Leaf::Split => {
                if let Some(slot) = selected {
                    let refused = self.split_refused(slot);
                    let hover = match refused {
                        None => SPLIT_DESCRIPTION,
                        Some(super::SplitRefusal::Full) => SPLIT_FULL_DESCRIPTION,
                        Some(super::SplitRefusal::Placed | super::SplitRefusal::Mono) => {
                            SPLIT_PLACED_DESCRIPTION
                        }
                    };
                    let pressed = navigation::at(ui, "split", |ui| {
                        let response = ui
                            .add_enabled(
                                refused.is_none(),
                                egui::Button::new("Split")
                                    .min_size(vec2(0.0, mxm_ui::space::MIN_TARGET)),
                            )
                            .on_hover_text(hover)
                            .on_disabled_hover_text(hover);
                        navigation::mark_unclaimed(ui, response.id, response.rect);
                        response
                            .clicked()
                            .then(|| ui.input(|input| input.modifiers.alt))
                    });
                    if let Some(mid_side) = pressed {
                        self.split(slot, mid_side);
                    }
                }
            }
            Leaf::Shape => {
                if let Some(slot) = selected {
                    let current = self.band_now(slot).shape.unwrap_or(Shape::Bell);
                    let options = shapes();
                    let options: Vec<&str> = options.iter().map(String::as_str).collect();
                    let mut index = SHAPES
                        .iter()
                        .position(|shape| *shape == current)
                        .unwrap_or(0);
                    let name = self.params.bands[slot].kind.name().to_owned();
                    // The type reserves values no shape has yet, so the binding's one-cell-per-value
                    // selector does not fit it: the shapes alone are offered, and the choice is
                    // written as one gesture.
                    let changed = navigation::at(ui, band_ids(slot)[Field::Type as usize], |ui| {
                        mxm_ui::control::selector_named(
                            ui,
                            tokens,
                            "Shape",
                            &name,
                            &options,
                            &mut index,
                            None,
                            Some(0),
                            SHAPE_DESCRIPTION,
                        )
                    });
                    if changed {
                        self.set_shape(slot, SHAPES[index]);
                    }
                }
            }
            Leaf::Slope => {
                if let Some(slot) = selected {
                    let slope = self.band_now(slot).slope;
                    let options = slopes();
                    let options: Vec<&str> = options.iter().map(String::as_str).collect();
                    let mut index = Slope::ALL
                        .iter()
                        .position(|other| *other == slope)
                        .unwrap_or(1);
                    let name = self.params.bands[slot].slope.name().to_owned();
                    let changed = navigation::at(ui, band_ids(slot)[Field::Slope as usize], |ui| {
                        mxm_ui::control::selector_named(
                            ui,
                            tokens,
                            "Slope",
                            &name,
                            &options,
                            &mut index,
                            None,
                            Some(1),
                            SLOPE_DESCRIPTION,
                        )
                    });
                    if changed {
                        self.set_slope(slot, Slope::ALL[index]);
                    }
                }
            }
            Leaf::Slider(slider) => {
                let params = self.params.clone();
                let slot = if slider.of_the_band() {
                    selected
                } else {
                    Some(0)
                };
                if let Some(slot) = slot {
                    // No band in the sound has a gain to scale: the slider fades, and its hover
                    // says why. It stays a control, as every parameter is (plan §8).
                    let unheard = slider == Slider::GainScale && !self.gain_heard();
                    let bound = if unheard {
                        gain_scale(&params, GAIN_SCALE_UNHEARD)
                    } else {
                        bound(&params, slot, slider)
                    };
                    let text = &mut self.text;
                    ui.scope(|ui| {
                        if unheard {
                            ui.multiply_opacity(ui.visuals().disabled_alpha());
                        }
                        bound.slider(ui, tokens, &setter, text);
                    });
                }
            }
            Leaf::Solo => {
                if let Some(slot) = selected {
                    let bypassed = self.band_now(slot).bypass;
                    let mut on = self.soloed() == Some(slot);
                    let label = if bypassed { "Bypassed" } else { "Solo" };
                    // A toggle (the owner, 2026-10-14: held, the band could not be edited while it
                    // played), drawn as every toggle is, the Analyser's beside it (the owner: "Is
                    // there no standard for that?"). OPEN presses it as a click does.
                    let pressed = navigation::at(ui, SOLO_KEY, |ui| {
                        // Disabled whenever the band offers none: bypassed, or placed in mono
                        // (plan §7.3), the hover saying which.
                        ui.add_enabled_ui(self.offers_solo(slot), |ui| {
                            mxm_ui::control::toggle_sized(
                                ui,
                                tokens,
                                label,
                                "Solo",
                                &mut on,
                                false,
                                if !self.placement_heard(slot) {
                                    SOLO_MONO_DESCRIPTION
                                } else if self.listens_to_key(slot) {
                                    SOLO_KEYED_DESCRIPTION
                                } else {
                                    SOLO_DESCRIPTION
                                },
                                rect.width(),
                            )
                        })
                        .inner
                    });
                    if pressed {
                        self.toggle_solo(slot);
                    }
                }
            }
            Leaf::Delete => {
                if let Some(slot) = selected {
                    let clicked = navigation::at(ui, "delete", |ui| {
                        let response = ui
                            .add(
                                egui::Button::new("Delete band")
                                    .min_size(vec2(0.0, mxm_ui::space::MIN_TARGET)),
                            )
                            .on_hover_text(DELETE_DESCRIPTION);
                        navigation::mark_unclaimed(ui, response.id, response.rect);
                        response.clicked()
                    });
                    if clicked {
                        self.delete(slot);
                    }
                }
            }
            Leaf::Analyser => {
                let mut settings = self.params.view.get();
                let changed = navigation::at(ui, "analyser", |ui| {
                    mxm_ui::control::toggle_sized(
                        ui,
                        tokens,
                        "Analyser",
                        "Analyser",
                        &mut settings.analyser,
                        false,
                        "Shows the sound's spectrum behind the curve.",
                        0.0,
                    )
                });
                if changed {
                    self.set_view(settings);
                }
            }
            Leaf::Freeze => {
                let analyser = self.params.view.get().analyser;
                let mut on = self.freezing();
                let changed = navigation::at(ui, "freeze", |ui| {
                    ui.add_enabled_ui(analyser, |ui| {
                        mxm_ui::control::toggle_sized(
                            ui,
                            tokens,
                            "Freeze",
                            "Freeze",
                            &mut on,
                            false,
                            FREEZE_DESCRIPTION,
                            0.0,
                        )
                    })
                    .inner
                });
                if changed {
                    if on {
                        self.ask_freeze();
                    } else {
                        self.unfreeze();
                    }
                }
            }
            Leaf::Find => {
                let analyser = self.params.view.get().analyser;
                let mut on = self.find;
                let description = match self.find_refusal() {
                    Some(FindRefusal::Full) if self.find => FIND_FULL_DESCRIPTION,
                    Some(FindRefusal::NoGain) if self.find => FIND_NO_GAIN_DESCRIPTION,
                    _ => FIND_DESCRIPTION,
                };
                let changed = navigation::at(ui, "find", |ui| {
                    ui.add_enabled_ui(analyser, |ui| {
                        mxm_ui::control::toggle_sized(
                            ui,
                            tokens,
                            "Find",
                            "Find resonances",
                            &mut on,
                            false,
                            description,
                            0.0,
                        )
                    })
                    .inner
                });
                if changed {
                    self.set_find(on);
                }
            }
            Leaf::ApplyAll => {
                let any = !self.proposals.is_empty();
                let clicked = navigation::at(ui, "apply all", |ui| {
                    let response = ui
                        .add_enabled(
                            any,
                            egui::Button::new("Apply all")
                                .min_size(vec2(0.0, mxm_ui::space::MIN_TARGET)),
                        )
                        .on_hover_text(APPLY_ALL_DESCRIPTION)
                        .on_disabled_hover_text(APPLY_ALL_DESCRIPTION);
                    navigation::mark_unclaimed(ui, response.id, response.rect);
                    response.clicked()
                });
                if clicked {
                    let all: Vec<usize> = (0..self.proposals.len()).collect();
                    self.apply_proposals(&all, false);
                }
            }
            Leaf::PhaseMode | Leaf::LinearResolution => self.paint_phase(ui, tokens, leaf),
            Leaf::Amount => self.paint_view_slider(ui, tokens, rect, ViewSlider::FindAmount),
            Leaf::MatchAmount => self.paint_view_slider(ui, tokens, rect, ViewSlider::MatchAmount),
            Leaf::Detail => self.paint_view_slider(ui, tokens, rect, ViewSlider::Detail),
            Leaf::Match => {
                let settings = self.params.view.get();
                let offered = self.match_offered(&settings);
                let mut on = self.matching;
                let description = if !self.telemetry.side_chain_port() {
                    MATCH_NO_PORT_DESCRIPTION
                } else {
                    match self.find_refusal() {
                        Some(FindRefusal::Full) if self.matching => MATCH_FULL_DESCRIPTION,
                        Some(FindRefusal::NoGain) if self.matching => MATCH_NO_GAIN_DESCRIPTION,
                        _ => MATCH_DESCRIPTION,
                    }
                };
                let changed = navigation::at(ui, "match", |ui| {
                    ui.add_enabled_ui(offered, |ui| {
                        mxm_ui::control::toggle_sized(
                            ui,
                            tokens,
                            "Match",
                            "Match a reference",
                            &mut on,
                            false,
                            description,
                            0.0,
                        )
                    })
                    .inner
                });
                if changed {
                    self.set_match(on);
                }
            }
            Leaf::MatchApply => {
                let settings = self.params.view.get();
                let current = self.match_current(&settings);
                let clicked = navigation::at(ui, "match apply", |ui| {
                    let response = ui
                        .add_enabled(
                            current,
                            egui::Button::new("Apply")
                                .min_size(vec2(0.0, mxm_ui::space::MIN_TARGET)),
                        )
                        .on_hover_text(MATCH_APPLY_DESCRIPTION)
                        .on_disabled_hover_text(MATCH_APPLY_DESCRIPTION);
                    navigation::mark_unclaimed(ui, response.id, response.rect);
                    response.clicked()
                });
                if clicked {
                    self.apply_match(&settings);
                }
            }
            Leaf::MatchHeard => {
                let [input, reference] = self.matcher.spectra().heard_seconds();
                let reading = if self.matching {
                    format!("Heard: input {input:.0} s, side chain {reference:.0} s")
                } else {
                    "Heard: nothing yet".to_owned()
                };
                let response = ui.allocate_rect(rect, egui::Sense::hover());
                ui.painter_at(rect).text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    &reading,
                    egui::FontId::proportional(12.0),
                    tokens.text_secondary,
                );
                response
                    .on_hover_text(MATCH_HEARD_DESCRIPTION)
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &reading)
                    });
            }
            Leaf::SideChainView => {
                let mut settings = self.params.view.get();
                let changed = navigation::at(ui, "side chain view", |ui| {
                    mxm_ui::control::toggle_sized(
                        ui,
                        tokens,
                        "Side chain",
                        "Side chain view",
                        &mut settings.side_chain,
                        false,
                        SIDE_CHAIN_VIEW_DESCRIPTION,
                        0.0,
                    )
                });
                if changed {
                    self.set_view(settings);
                }
            }
            Leaf::Moving => {
                if let Some(slot) = selected {
                    let moving = self.moving_db(slot);
                    self.paint_moving(ui, tokens, rect, moving);
                }
            }
            Leaf::Show | Leaf::Resolution | Leaf::Speed | Leaf::Tilt => {
                self.paint_setting(ui, tokens, leaf);
            }
        }
    }

    /// Phase and Resolution (plan §7.7): stepped parameters, not automatable, each change asked
    /// for at once and heard after the host's restart. Resolution is offered only in Linear phase.
    /// Their hovers give the latency and say when a change still waits.
    fn paint_phase(&mut self, ui: &mut Ui, tokens: &Tokens, leaf: Leaf) {
        use mxm_fx_eq_n_dsp::Phase;
        let asked = self.phase_asked();
        let in_force = self.telemetry.phase();
        let rate = self.telemetry.sample_rate();
        let latency_ms = |phase: Phase| match phase {
            Phase::ZeroLatency => 0.0,
            Phase::Linear(resolution) => {
                mxm_fx_eq_n_dsp::linear::Sizes::new(f64::from(rate), resolution).latency() as f32
                    * 1_000.0
                    / rate
            }
        };
        let pending = if asked != in_force {
            PENDING_DESCRIPTION
        } else {
            ""
        };
        let (index, key, label, names, mut current, enabled, description) = match leaf {
            Leaf::PhaseMode => (
                crate::params::PHASE_INDEX,
                "phase",
                "Phase",
                &crate::params::PHASE_NAMES[..],
                usize::from(matches!(asked, Phase::Linear(_))),
                true,
                match asked {
                    Phase::ZeroLatency => format!("{PHASE_ZERO_DESCRIPTION}{pending}"),
                    Phase::Linear(_) => format!(
                        "{PHASE_ZERO_DESCRIPTION} Now the sound is {:.0} ms late; a mix plays it \
                         on time, but monitoring live through it does not.{pending}",
                        latency_ms(asked)
                    ),
                },
            ),
            _ => {
                let resolution = match asked {
                    Phase::Linear(resolution) => resolution,
                    Phase::ZeroLatency => crate::params::resolution_of(
                        self.params
                            .resolution
                            .preview_plain(self.normalised_now(crate::params::RESOLUTION_INDEX)),
                    ),
                };
                (
                    crate::params::RESOLUTION_INDEX,
                    "resolution",
                    "Resolution",
                    &LINEAR_RESOLUTIONS[..],
                    mxm_fx_eq_n_dsp::Resolution::ALL
                        .iter()
                        .position(|other| *other == resolution)
                        .unwrap_or(1),
                    matches!(asked, Phase::Linear(_)),
                    format!("{RESOLUTION_DESCRIPTION}{pending}"),
                )
            }
        };
        let before = current;
        let name = self.params.param(index).name();
        let changed = navigation::at(ui, key, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                mxm_ui::control::selector_named(
                    ui,
                    tokens,
                    label,
                    name,
                    names,
                    &mut current,
                    None,
                    None,
                    &description,
                )
            })
            .inner
        });
        if changed && current != before {
            self.set_phase_value(index, current as i32);
        }
    }

    /// A view-state slider (plan §7.5, §7.6): not a parameter, so the kit's slider with its value
    /// held here while it is dragged, and the settings written — the project marked dirty — once,
    /// when it is let go. A typed value, a step key or a double-click is a gesture that ends at once.
    fn paint_view_slider(&mut self, ui: &mut Ui, tokens: &Tokens, rect: Rect, which: ViewSlider) {
        let mut settings = self.params.view.get();
        let key = which.key();
        let id = egui::Id::new(("mxm-fx-eq-n", key));
        let mut normalised = ui
            .data(|data| data.get_temp::<f64>(id))
            .unwrap_or(which.normalised(&settings));
        let text = which.text(normalised);
        let view = mxm_ui::control::ParamView {
            default: which.default(),
            widest: which.widest(),
            steps: which.steps(),
            ..mxm_ui::control::ParamView::new(which.label(), &text, which.description())
        };
        let entry = self.text.entry(key).or_default();
        // `Escape` abandons an open entry, as on every bound control.
        if entry.is_some()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            *entry = None;
        }
        let outcome = navigation::at(ui, key, |ui| {
            mxm_ui::control::slider(
                ui,
                tokens,
                &view,
                &mut normalised,
                rect.width(),
                entry,
                mxm_ui::control::Wheel::Off,
            )
        });
        let submitted = if outcome.changed && entry.is_some() {
            // A committed text entry, parsed here because this editor wrote the reading.
            let parsed = entry.as_deref().and_then(|typed| which.parse(typed));
            *entry = None;
            parsed
        } else {
            if outcome.changed {
                ui.data_mut(|data| data.insert_temp(id, normalised));
            }
            outcome.gesture_ended.then_some(normalised)
        };
        if outcome.gesture_ended || submitted.is_some() {
            ui.data_mut(|data| data.remove_temp::<f64>(id));
        }
        if let Some(normalised) = submitted {
            which.write(&mut settings, normalised);
            self.set_view(settings);
        }
    }

    /// The moving meter: a track with its middle marked, and a bar from the middle to the band's
    /// movement, left for a cut and right for a lift. Its reading is its name for a screen reader.
    fn paint_moving(&self, ui: &mut Ui, tokens: &Tokens, rect: Rect, moving_db: f32) {
        let response = ui.allocate_rect(rect, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, tokens.surface_2);
        let middle = rect.center().x;
        let half = rect.width() / 2.0;
        let reach = (moving_db / MOVING_SPAN_DB).clamp(-1.0, 1.0) * half;
        if reach != 0.0 {
            let bar = Rect::from_x_y_ranges(
                middle.min(middle + reach)..=middle.max(middle + reach),
                rect.y_range(),
            );
            painter.rect_filled(bar, 2.0, tokens.accent);
        }
        painter.line_segment(
            [
                egui::pos2(middle, rect.top()),
                egui::pos2(middle, rect.bottom()),
            ],
            egui::Stroke::new(1.0, tokens.border_strong),
        );
        let reading = format!("Moving {moving_db:+.1} dB");
        response
            .on_hover_text(MOVING_DESCRIPTION)
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &reading));
    }

    fn paint_setting(&mut self, ui: &mut Ui, tokens: &Tokens, leaf: Leaf) {
        let mut settings = self.params.view.get();
        let (key, label, options, details, mut index): (&str, &str, &[&str], &[&str], usize) =
            match leaf {
                Leaf::Show => (
                    "show",
                    "Show",
                    &SHOW_OPTIONS,
                    &[
                        "The sound coming in, as an outline.",
                        "The sound going out, filled.",
                        "Both: in as an outline, out filled.",
                    ],
                    Show::ALL
                        .iter()
                        .position(|show| *show == settings.show)
                        .unwrap_or(2),
                ),
                Leaf::Resolution => (
                    "resolution",
                    "Resolution",
                    &RESOLUTION_OPTIONS,
                    &[
                        "Quick to follow; the lows are broad.",
                        "Finer lows, a third of a second to settle.",
                        "The finest lows, over a second and a half to settle.",
                    ],
                    Resolution::ALL
                        .iter()
                        .position(|resolution| *resolution == settings.resolution)
                        .unwrap_or(1),
                ),
                Leaf::Speed => (
                    "speed",
                    "Speed",
                    &SPEED_OPTIONS,
                    &[
                        "Levels fall slowly, holding what passed.",
                        "Levels fall at a steady pace.",
                        "Levels fall quickly, following the sound.",
                    ],
                    Speed::ALL
                        .iter()
                        .position(|speed| *speed == settings.speed)
                        .unwrap_or(1),
                ),
                _ => (
                    "tilt",
                    "Tilt",
                    &TILT_OPTIONS,
                    &[
                        "The spectrum as measured.",
                        "Tilted so typical music reads level.",
                        "Tilted further, lifting the highs.",
                        "Tilted furthest, lifting the highs most.",
                    ],
                    Tilt::ALL
                        .iter()
                        .position(|tilt| *tilt == settings.tilt)
                        .unwrap_or(1),
                ),
            };
        let description = details[index.min(details.len() - 1)];
        let changed = navigation::at(ui, key, |ui| {
            mxm_ui::control::selector(
                ui,
                tokens,
                label,
                options,
                &mut index,
                None,
                None,
                description,
            )
        });
        if changed {
            match leaf {
                Leaf::Show => settings.show = Show::ALL[index],
                Leaf::Resolution => settings.resolution = Resolution::ALL[index],
                Leaf::Speed => settings.speed = Speed::ALL[index],
                _ => settings.tilt = Tilt::ALL[index],
            }
            self.set_view(settings);
        }
    }
}
