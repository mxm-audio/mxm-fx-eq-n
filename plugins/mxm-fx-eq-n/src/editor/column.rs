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
}

const SHAPE_DESCRIPTION: &str = "What the band does to the sound around its frequency.";
const SLOPE_DESCRIPTION: &str = "How steeply the cut falls away past its frequency.";
const BYPASS_DESCRIPTION: &str = "Takes the band out of the sound and keeps its settings.";
const SOLO_DESCRIPTION: &str = "Plays only what this band works on, until it is pressed again.";
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
    match slider {
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
fn band_body(params: &EqParams, slot: usize, shape: Shape, slope: Slope) -> Node<Leaf> {
    let mut rows = vec![
        leaf(
            Leaf::Bypass,
            Kind::Toggle {
                label: "Bypass".to_owned(),
            },
        ),
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
    let mut actions = Vec::new();
    if shape.offers_solo() {
        actions.push(button_leaf(Leaf::Solo, "Bypassed"));
    }
    actions.push(button_leaf(Leaf::Delete, "Delete band"));
    rows.push(row_gap(SPACE_3, actions));
    stack(rows)
}

/// The Band card's body: the selected band's, reserved to the tallest shape's.
pub fn band_tree(app: &EqApp) -> Node<Leaf> {
    let params = &app.params;
    let alternatives = vec![
        band_body(params, 0, Shape::Bell, Slope::Db12),
        band_body(params, 0, Shape::LowCut, Slope::Db12),
    ];
    let shown = app
        .selected
        .and_then(|slot| {
            let band = app.band_now(slot);
            band.shape
                .map(|shape| band_body(params, slot, shape, band.slope))
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

    pub(crate) fn paint_leaf(&mut self, ui: &mut Ui, tokens: &Tokens, leaf: Leaf, _rect: Rect) {
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
                let band_slider = matches!(slider, Slider::Frequency | Slider::Gain | Slider::Q);
                let slot = if band_slider { selected } else { Some(0) };
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
                    let soloed = self.soloed() == Some(slot);
                    let text = if bypassed { "Bypassed" } else { "Solo" };
                    let response = navigation::at(ui, SOLO_KEY, |ui| {
                        let response = ui
                            .add_enabled(
                                !bypassed,
                                egui::Button::new(text)
                                    .selected(soloed)
                                    .min_size(vec2(0.0, mxm_ui::space::MIN_TARGET)),
                            )
                            .on_hover_text(SOLO_DESCRIPTION);
                        navigation::mark_unclaimed(ui, response.id, response.rect);
                        response
                    });
                    // A toggle (the owner, 2026-10-14: held, the band could not be edited while it
                    // played). OPEN presses it as a click does.
                    if response.clicked() {
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
            Leaf::Show | Leaf::Resolution | Leaf::Speed | Leaf::Tilt => {
                self.paint_setting(ui, tokens, leaf);
            }
        }
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
