//! The column to the graph's right (plan §5.1): the Band card for the selected band, then Output
//! and Analyser. Each body is a `mxm_ui::tree`, measured and drawn from one description, and every
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

use super::{ANALYSER_CARD, BAND_CARD, EqApp, Field, OUTPUT_CARD, SOLO_KEY};
use crate::params::{EqParams, SHAPES, band_ids, shape_name};
use crate::view::{Resolution, Show, Speed, Tilt};

/// The cards' titles, top to bottom.
pub const OUTPUT_TITLE: &str = "Output";
pub const ANALYSER_TITLE: &str = "Analyser";

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
    stack(vec![
        slider_leaf(&app.params, 0, Slider::Output),
        slider_leaf(&app.params, 0, Slider::GainScale),
    ])
}

const SHOW_OPTIONS: [&str; 3] = ["Pre", "Post", "Both"];
const RESOLUTION_OPTIONS: [&str; 3] = ["Low", "Medium", "High"];
const SPEED_OPTIONS: [&str; 3] = ["Slow", "Medium", "Fast"];
const TILT_OPTIONS: [&str; 4] = ["0 dB/oct", "1.5 dB/oct", "3 dB/oct", "4.5 dB/oct"];

pub fn analyser_tree() -> Node<Leaf> {
    let options = |all: &[&str]| all.iter().map(|option| (*option).to_owned()).collect();
    stack(vec![
        leaf(
            Leaf::Analyser,
            Kind::Toggle {
                label: "Analyser".to_owned(),
            },
        ),
        selector_leaf(Leaf::Show, "Show", options(&SHOW_OPTIONS)),
        selector_leaf(Leaf::Resolution, "Resolution", options(&RESOLUTION_OPTIONS)),
        selector_leaf(Leaf::Speed, "Speed", options(&SPEED_OPTIONS)),
        selector_leaf(Leaf::Tilt, "Tilt", options(&TILT_OPTIONS)),
    ])
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
    /// The column's three cards, top to bottom, each one of the cursor's.
    pub(crate) fn column(&mut self, ui: &mut Ui, tokens: &Tokens) {
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
        ui.add_space(SPACE_3);
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
            |_, _| analyser_tree(),
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
                        ui.add_enabled_ui(!bypassed, |ui| {
                            mxm_ui::control::toggle_sized(
                                ui,
                                tokens,
                                label,
                                "Solo",
                                &mut on,
                                false,
                                if self.listens_to_key(slot) {
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
