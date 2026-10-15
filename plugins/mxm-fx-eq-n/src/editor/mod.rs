//! The editor (plan §5): the graph takes every spare point, a fixed-width column of cards sits to
//! its right and never under it, and an Out meter rail stands between them.
//!
//! - **The keyboard is the collection's keyboard language** (`mxm_ui::navigation`, its engine
//!   `mxm-keys`, and the keymap in use in the shared folder), as in every MXM editor: the cursor
//!   walks the column's parameters and the bars, and VIEW moves between the graph and the cards.
//!   **The graph is a surface** (`navigation::surface`): with the cursor on it, the language's
//!   outputs come here ([`EqApp::on_surface`]) and edit the bands.
//! - **Every host write goes through the [`history::Recorder`]**, so undo sees the column's
//!   controls, the graph's drags and the keyboard's gestures alike.
//! - **Solo is a toggle** (the owner, 2026-10-14): the Band card's Solo button latches it on the
//!   selected band, until it is pressed again, another band is selected, the band leaves the sound
//!   or the patch is replaced; a Ctrl/Cmd press on a point solos that point while held. It crosses
//!   `crate::audition`, so the DSP reads no editor state; the shell ends it on reset, deactivation
//!   and restore, and the editor ends it on close.
//! - **Find** (plan §7.5) listens to the input for resonances while it is on, and its proposals are
//!   applied as ordinary bands, each one undo step ([`EqApp::apply_proposals`]).
//! - **Match** (plan §7.6) listens to the side chain and the input and proposes the bands that
//!   bring the input's tone to the reference's, applied together as one undo step
//!   ([`EqApp::apply_match`]). One proposal set at a time: Find, Match and Freeze each end the
//!   others.

pub mod column;
pub mod finder;
pub mod grab;
pub mod graph;
pub mod history;
pub mod matcher;
pub mod panel;
pub mod spectrum;

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Id, Rect};
use mxm_fx_eq_n_dsp::{FREQ_MIN, Q_MAX, Q_MIN, SLOTS, Shape, Slope, max_frequency};
use mxm_keys::{Action, Direction, Output, Step as Size, Verb};
use mxm_params::ErasedParam;
use nice_plug::context::gui::GuiContext;
use nice_plug::params::internals::ParamPtr;
use nice_plug::prelude::*;

use crate::analyser::Block;
use crate::audition::Audition;
use crate::params::{BAND_FIELDS, DEFAULT_SLOPE, EqParams, type_of};
use crate::telemetry::Telemetry;
use crate::view::ViewSettings;

/// The window the editor opens at: inside the quarter-4K budget, as tall as the column's tallest
/// state with no scrolling (`the_opening_size_holds_the_column_without_scrolling`).
pub const REFERENCE: (u32, u32) = (1_532, 1_010);
/// One card wide, and never narrower than the app bar's last compact step
/// (`the_app_bar_holds_in_the_minimum_window`). Narrower than graph and column, the surface scrolls
/// sideways with the column still beside the graph (plan §5.1).
pub const MINIMUM: (u32, u32) = (420, 480);
/// Each column's fixed width.
pub const COLUMN_WIDTH: f32 = 320.0;
/// The two columns of cards beside each other (decision 30), and the gap between them.
pub const COLUMNS_WIDTH: f32 = 2.0 * COLUMN_WIDTH + mxm_ui::space::SPACE_3;
/// Below this the graph stops shrinking and the surface scrolls sideways instead (plan §5.1).
pub const GRAPH_FLOOR: f32 = 320.0;
/// The Out meter's rail: two channels and the gap between them.
pub const METER_RAIL: f32 = 2.0 * mxm_ui::shell::VERTICAL_METER_WIDTH + 4.0;

/// The cursor's cards, every one drawn outside a paging renderer (`navigation::bar_card`), in the
/// order the cursor takes them: the graph first, then the column's.
pub const GRAPH_CARD: u64 = 1;
pub const BAND_CARD: u64 = 2;
pub const OUTPUT_CARD: u64 = 3;
pub const ANALYSER_CARD: u64 = 4;
pub const RESONANCES_CARD: u64 = 5;
pub const MATCH_CARD: u64 = 6;
pub const CARDS: [u64; 6] = [
    GRAPH_CARD,
    BAND_CARD,
    OUTPUT_CARD,
    ANALYSER_CARD,
    RESONANCES_CARD,
    MATCH_CARD,
];
/// The graph's key in the cursor's registry.
pub const GRAPH_KEY: &str = "graph";
/// The Solo button's key: OPEN on it presses it.
pub const SOLO_KEY: &str = "solo";

pub type EqEditor = mxm_editor::Editor<EqApp>;

/// What an editor needs from the plugin instance.
pub struct Handles {
    pub params: Arc<EqParams>,
    pub telemetry: Arc<Telemetry>,
    pub audition: Arc<Audition>,
    pub history: history::Shared,
    pub analyser: Option<rtrb::Consumer<Block>>,
}

/// The window, its setup (`mxm_ui::shell::install`) and the host context's lifecycle are
/// mxm-editor's; this editor is the [`mxm_editor::Panel`] inside.
pub fn create(handles: Handles) -> Option<EqEditor> {
    mxm_editor::create(crate::NAME, REFERENCE, MINIMUM, EqApp::new(handles))
}

/// A slot's field, as its place in [`BAND_FIELDS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Type = 0,
    Bypass = 1,
    Freq = 2,
    Gain = 3,
    Q = 4,
    Slope = 5,
}

/// A host parameter's place in [`EqParams::pointers`] and the history's indices.
pub fn band_index(slot: usize, field: Field) -> usize {
    slot * BAND_FIELDS.len() + field as usize
}

/// Why Find or Match proposes nothing (plan §7.5, §7.6), each with its hover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindRefusal {
    Full,
    NoGain,
}

/// What the editor hands its worker (plan §7.5, §7.6): Find's search or Match's fit, one at a time.
pub enum Task {
    Search(finder::Search),
    Fit(matcher::Fit),
}

/// What the worker gives back.
pub enum Done {
    Found(finder::Found),
    Fitted(matcher::Fitted),
}

impl finder::Job for Task {
    type Done = Done;
    fn run(self) -> Done {
        match self {
            Task::Search(search) => Done::Found(search.run()),
            Task::Fit(fit) => Done::Fitted(fit.run()),
        }
    }
}

/// Why Split cannot act (plan §7.3), each with its hover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitRefusal {
    Mono,
    Placed,
    Full,
}

/// A slot's dynamics fields (plan §7.1), in [`crate::params::DYNAMICS_FIELDS`] order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicsField {
    Range = 0,
    Threshold = 1,
    Attack = 2,
    Release = 3,
}

/// A slot's dynamics parameter's place in [`EqParams::pointers`] and the history's indices.
pub fn dynamics_index(slot: usize, field: DynamicsField) -> usize {
    crate::params::DYNAMICS_BASE + slot * crate::params::DYNAMICS_FIELDS.len() + field as usize
}

/// A slot's side-chain switch's place in [`EqParams::pointers`] and the history's indices.
pub fn key_index(slot: usize) -> usize {
    crate::params::KEY_BASE + slot
}

/// A slot's placement's place in [`EqParams::pointers`] and the history's indices (plan §7.3).
pub fn place_index(slot: usize) -> usize {
    crate::params::PLACE_BASE + slot
}

/// The slot a history index belongs to: a band field's, a dynamics field's or a side-chain
/// switch's, none for a global.
pub fn slot_of(index: usize) -> Option<usize> {
    let dynamics = crate::params::DYNAMICS_BASE;
    if index < OUTPUT_INDEX {
        Some(index / BAND_FIELDS.len())
    } else if index >= crate::params::PLACE_BASE {
        Some(index - crate::params::PLACE_BASE).filter(|&slot| slot < SLOTS)
    } else if index >= crate::params::KEY_BASE {
        Some(index - crate::params::KEY_BASE)
    } else if index >= dynamics {
        Some((index - dynamics) / crate::params::DYNAMICS_FIELDS.len()).filter(|&slot| slot < SLOTS)
    } else {
        None
    }
}
pub const OUTPUT_INDEX: usize = SLOTS * BAND_FIELDS.len();
pub const GAIN_SCALE_INDEX: usize = OUTPUT_INDEX + 1;

/// One of a band's edges, which MICRO + ← → take for MOVE to move on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// Host parameters held open across frames: a drag, a keyboard gesture, a turn of the wheel. Each
/// is begun once and ended once, and its values chain from what was last sent rather than from a
/// readback the host may not have applied yet. What each read when it was opened is kept, so a
/// gesture BACK cancels can be put back.
#[derive(Default)]
pub struct Open {
    params: Vec<(usize, f32)>,
    before: Vec<(usize, f32)>,
}

impl Open {
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    pub fn holds(&self, index: usize) -> bool {
        self.params.iter().any(|(open, _)| *open == index)
    }

    /// The normalised value last sent to `index` in this gesture.
    pub fn sent(&self, index: usize) -> Option<f32> {
        self.params
            .iter()
            .find(|(open, _)| *open == index)
            .map(|(_, value)| *value)
    }
}

pub struct EqApp {
    pub(crate) params: Arc<EqParams>,
    pub(crate) telemetry: Arc<Telemetry>,
    pub(crate) audition: Arc<Audition>,
    pub(crate) history: history::Shared,
    analyser: Option<rtrb::Consumer<Block>>,
    /// The host's own context: undo and redo write through it, unrecorded.
    host: Option<GuiContext>,
    /// The recorded context every edit writes through.
    pub(crate) gui: Option<GuiContext>,
    pointers: Vec<ParamPtr>,
    /// The keyboard language's cursor over the graph, the cards and the bars.
    pub(crate) nav: mxm_ui::navigation::State,
    pub(crate) selected: Option<usize>,
    /// The keyboard's place on the line where no band is selected.
    pub(crate) focus_freq: f32,
    /// The selected band's edge MICRO + ← → took, for MOVE to move alone.
    pub(crate) edge: Option<Edge>,
    /// The slot a DUPLICATE gesture's copy took, and the band it copies.
    copy: Option<(usize, usize)>,
    pub(crate) drag: Option<graph::Drag>,
    pub(crate) open: Open,
    /// When the wheel last turned Q, which ends its gesture after a pause.
    pub(crate) wheel_until: Option<f64>,
    pub(crate) context_slot: Option<usize>,
    /// MENU asked for the selected band's menu (mxm-keys' MENU, the right-click menu of what the
    /// cursor is on); the graph opens it at the band's point.
    pub(crate) menu_asked: bool,
    /// A held solo this frame: Ctrl/Cmd held on a point.
    pub(crate) pointer_solo: Option<usize>,
    /// The display's last two seconds, for a freeze (plan §7.4), and the basis they were read on.
    pub(crate) hold: grab::Hold,
    pub(crate) hold_basis: Option<grab::Basis>,
    /// The freeze in force, and a Freeze asked for this frame (made where the spectrum is drawn).
    pub(crate) frozen: Option<grab::Frozen>,
    pub(crate) freeze_asked: bool,
    /// The keyboard's peak while frozen with no band selected: its place in `frozen.peaks`.
    pub(crate) focus_peak: Option<usize>,
    /// The peak labels drawn last frame, by their place in `frozen.peaks`: a label is part of its
    /// peak for the pointer (plan §7.4).
    pub(crate) peak_labels: Vec<(usize, egui::Rect)>,
    /// Find (plan §7.5): on or off, the editor's moment, never saved; what it has heard; the
    /// candidates found last and when; the proposals shown, what they were made on and when.
    pub(crate) find: bool,
    pub(crate) finder: finder::Finder,
    candidates: Vec<finder::Candidate>,
    found_at: Option<f64>,
    /// The finder's revision the candidates were found at.
    found_revision: u64,
    pub(crate) proposals: Vec<finder::Proposal>,
    proposed_on: Vec<u32>,
    proposed_at: f64,
    /// An apply happened: the next frame proposes again at once.
    proposals_stale: bool,
    /// The thread searches and fits run on, while Find or Match is on; and whether they run here
    /// instead, as a test has them, so what they find is there the same frame.
    worker: Option<finder::Worker<Task>>,
    pub(crate) inline_search: bool,
    /// The finder's listen the shown candidates and proposals belong to.
    listen_seen: u64,
    /// A search that was to find the candidates again could not start: the next one does.
    refind_pending: bool,
    /// Counts each new set of proposals, so the graph's dashed curve is computed again only then.
    pub(crate) proposals_version: u64,
    /// The keyboard's proposal while Find has some and no band is selected, and the labels drawn
    /// last frame, by their place in `proposals`.
    pub(crate) focus_proposal: Option<usize>,
    pub(crate) proposal_labels: Vec<(usize, egui::Rect)>,
    /// The dashed curve the proposals would make: the proposals' version and the frequencies it
    /// was computed at, and its lines.
    pub(crate) proposal_curve: Option<(u64, Vec<f32>, Vec<Vec<f32>>)>,
    /// Match (plan §7.6): on or off, the editor's moment, never saved; what it has heard; the
    /// bands proposed and the basis they were fitted on; the basis and the matcher's revision the
    /// last fit was asked on, and when.
    pub(crate) matching: bool,
    pub(crate) matcher: matcher::Matcher,
    pub(crate) match_bands: Vec<matcher::MatchBand>,
    pub(crate) match_fitted_on: Vec<u32>,
    match_asked_on: Vec<u32>,
    match_asked_at: Option<f64>,
    match_revision: u64,
    match_listen_seen: u64,
    /// Counts each new set of match bands, so the graph's dashed curve is computed again only then.
    pub(crate) match_version: u64,
    pub(crate) match_curve: Option<(u64, Vec<f32>, Vec<Vec<f32>>)>,
    /// Where the primary button last went down on the graph: a click is judged there, not where it
    /// was let go (egui forgets the press's origin in the release's pass).
    pub(crate) press_at: Option<egui::Pos2>,
    /// The band the Solo button latched, until it is pressed again, another band is selected,
    /// the band leaves the sound or the patch is replaced.
    pub(crate) latched: Option<usize>,
    solo_endings: u32,
    /// The shell ended a solo while a press held it: that press may not bring it back.
    solo_blocked: bool,
    pub(crate) spectrum: spectrum::Spectrum,
    /// The analyser's switch last frame: coming back on starts the spectrum from silence.
    analyser_on: bool,
    /// The rate the spectrum and the finder were built for, as the shell published it.
    analysed_rate: f32,
    /// Where the cursor was last frame, so a move scrolls its target into view.
    last_target: (Option<usize>, Option<u64>, Option<String>),
    /// The cursor's new target, to scroll into view when the surface is drawn.
    scroll_to: Option<Rect>,
    /// The window's size and zoom last frame, and the frames left to scroll after a change.
    last_view: (egui::Vec2, f32),
    rescroll: u8,
    pub(crate) text: HashMap<&'static str, Option<String>>,
    pub(crate) presets: mxm_preset::PresetUi,
    pub(crate) meters: column::MeterDisplay,
    pub(crate) graph_rect: Rect,
    /// The column's drawn height and the height it had, last frame: the opening size holds it.
    pub(crate) column_extent: (f32, f32),
}

pub(crate) fn graph_id() -> Id {
    Id::new("mxm-fx-eq-n-graph")
}

impl EqApp {
    pub fn new(handles: Handles) -> Self {
        let presets = mxm_preset::PresetUi::new(handles.params.as_ref());
        let pointers = handles.params.pointers();
        let spectrum = spectrum::Spectrum::new(handles.telemetry.sample_rate());
        let finder = finder::Finder::new(handles.telemetry.sample_rate());
        let matcher = matcher::Matcher::new(handles.telemetry.sample_rate());
        let analysed_rate = handles.telemetry.sample_rate();
        Self {
            params: handles.params,
            telemetry: handles.telemetry,
            audition: handles.audition,
            history: handles.history,
            analyser: handles.analyser,
            host: None,
            gui: None,
            pointers,
            nav: mxm_ui::navigation::State::default(),
            selected: None,
            focus_freq: 1_000.0,
            edge: None,
            copy: None,
            drag: None,
            open: Open::default(),
            wheel_until: None,
            context_slot: None,
            menu_asked: false,
            hold: grab::Hold::default(),
            hold_basis: None,
            frozen: None,
            freeze_asked: false,
            focus_peak: None,
            peak_labels: Vec::new(),
            find: false,
            finder,
            candidates: Vec::new(),
            found_at: None,
            found_revision: u64::MAX,
            proposals: Vec::new(),
            proposed_on: Vec::new(),
            proposed_at: f64::NEG_INFINITY,
            proposals_stale: false,
            worker: None,
            inline_search: false,
            listen_seen: 0,
            refind_pending: false,
            proposals_version: 0,
            focus_proposal: None,
            proposal_labels: Vec::new(),
            proposal_curve: None,
            matching: false,
            matcher,
            match_bands: Vec::new(),
            match_fitted_on: Vec::new(),
            match_asked_on: Vec::new(),
            match_asked_at: None,
            match_revision: u64::MAX,
            match_listen_seen: 0,
            match_version: 0,
            match_curve: None,
            press_at: None,
            pointer_solo: None,
            latched: None,
            solo_endings: 0,
            solo_blocked: false,
            spectrum,
            analyser_on: false,
            analysed_rate,
            last_target: (None, None, None),
            scroll_to: None,
            last_view: (egui::Vec2::ZERO, 1.0),
            rescroll: 0,
            text: HashMap::new(),
            presets,
            meters: column::MeterDisplay::default(),
            graph_rect: Rect::NOTHING,
            column_extent: (0.0, 0.0),
        }
    }

    /// Connects to a host: `host` is the window's own context. Also what a test calls.
    pub fn attach(&mut self, host: GuiContext) {
        let params = self.params.clone();
        self.gui = Some(history::Recorder::context(
            host.clone(),
            self.history.clone(),
            params.pointers(),
        ));
        self.host = Some(host);
        self.solo_endings = self.audition.state().0;
        if let Some(consumer) = &mut self.analyser {
            while consumer.pop().is_ok() {}
        }
        self.spectrum.restart();
        // nice-plug keeps this editor between windows: the display starts from silence too.
        self.meters = column::MeterDisplay::default();
        self.telemetry.connect(true);
    }

    /// The editor closes: every open gesture ends, solo ends, and the analyser stops.
    pub fn detach(&mut self) {
        self.close_open();
        // A shared control held as the window closes never sends its own end: whatever the
        // recorder still holds open is ended through it, kept as one action.
        if let Some(gui) = self.gui.clone() {
            let setter = gui.param_setter();
            let params = self.params.clone();
            let open = history::lock(&self.history).open_params();
            for index in open {
                params.param(index).end(&setter);
            }
        }
        self.copy = None;
        self.end_solo();
        self.drag = None;
        // A freeze is the editor's moment, and so is a listen: closing ends them (plan §7.4,
        // §7.5, §7.6).
        self.unfreeze();
        self.end_find();
        self.end_match();
        self.telemetry.connect(false);
        self.gui = None;
        self.host = None;
    }

    pub(crate) fn sample_rate(&self) -> f64 {
        f64::from(self.telemetry.sample_rate())
    }

    /// The highest frequency a band runs at here.
    pub(crate) fn high(&self) -> f32 {
        max_frequency(self.sample_rate()) as f32
    }

    /// Where a band set at `freq` runs at this rate: a band saved above the rate's limit runs at it.
    pub(crate) fn clamp_freq(&self, freq: f32) -> f32 {
        if freq.is_finite() {
            freq.clamp(FREQ_MIN as f32, self.high())
        } else {
            1_000.0
        }
    }

    pub(crate) fn erased(&self, index: usize) -> &dyn ErasedParam {
        self.params.param(index)
    }

    pub(crate) fn band_in_use(&self, slot: usize) -> bool {
        self.band_now(slot).shape.is_some()
    }

    /// A parameter, normalised, as this editor last set it: the open gesture's last value, else
    /// a value sent that the host has not applied yet ([`history::History::pending`]), else what
    /// it reads. Every chained edit reads through it.
    pub(crate) fn normalised_now(&self, index: usize) -> f32 {
        self.open.sent(index).unwrap_or_else(|| {
            history::lock(&self.history).pending(index, self.erased(index).normalised())
        })
    }

    /// The gain scale as this editor last set it, as the factor the audio applies (1 at 100 %).
    pub(crate) fn gain_scale_now(&self) -> f32 {
        self.params
            .gain_scale
            .preview_plain(self.normalised_now(GAIN_SCALE_INDEX))
    }

    /// How far a dynamic band moves now, as heard: the audio thread's movement under the gain
    /// scale where the audio thread has glided it (the Band card's meter).
    pub(crate) fn moving_db(&self, slot: usize) -> f32 {
        let (drawn, heard) = self.telemetry.gains(slot);
        (heard - drawn) * self.telemetry.heard_scale()
    }

    /// A slot as this editor last set it ([`normalised_now`](Self::normalised_now)'s values).
    pub(crate) fn band_now(&self, slot: usize) -> mxm_fx_eq_n_dsp::Band {
        let band = &self.params.bands[slot];
        let history = history::lock(&self.history);
        let at = |field: Field| {
            let index = band_index(slot, field);
            self.open
                .sent(index)
                .unwrap_or_else(|| history.pending(index, self.erased(index).normalised()))
        };
        mxm_fx_eq_n_dsp::Band {
            shape: crate::params::shape_of(band.kind.preview_plain(at(Field::Type))),
            bypass: band.bypass.preview_plain(at(Field::Bypass)),
            freq: band.freq.preview_plain(at(Field::Freq)),
            gain_db: band.gain.preview_plain(at(Field::Gain)),
            q: band.q.preview_plain(at(Field::Q)),
            slope: crate::params::slope_of(band.slope.preview_plain(at(Field::Slope)))
                .unwrap_or(DEFAULT_SLOPE),
        }
    }

    pub(crate) fn select(&mut self, slot: usize) {
        if self.selected != Some(slot) {
            self.edge = None;
        }
        self.selected = Some(slot);
        self.focus_freq = self.clamp_freq(self.band_now(slot).freq);
    }

    /// Whether the keyboard's cursor is on the graph.
    pub(crate) fn cursor_on_graph(&self) -> bool {
        self.nav.bar().is_none() && self.nav.parameter() == Some(GRAPH_KEY)
    }

    // ---- writing --------------------------------------------------------------------------

    /// Opens `index` in the running gesture if it is not open yet, and sends `normalised`.
    pub(crate) fn send(&mut self, index: usize, normalised: f32) {
        let Some(gui) = self.gui.clone() else {
            return;
        };
        let setter = gui.param_setter();
        let params = self.params.clone();
        let param = params.param(index);
        if !self.open.holds(index) {
            // Where the previous gesture left it, though the host may not have applied that yet.
            let before = self.normalised_now(index);
            param.begin(&setter);
            self.open.params.push((index, before));
            self.open.before.push((index, before));
        }
        param.set(&setter, normalised);
        if let Some(entry) = self.open.params.iter_mut().find(|(open, _)| *open == index) {
            entry.1 = normalised;
        }
    }

    /// Ends every open parameter: the gesture is over and kept.
    pub(crate) fn close_open(&mut self) {
        let open = std::mem::take(&mut self.open.params);
        self.open.before.clear();
        self.wheel_until = None;
        let Some(gui) = self.gui.clone() else {
            return;
        };
        let setter = gui.param_setter();
        let params = self.params.clone();
        for (index, _) in open {
            params.param(index).end(&setter);
        }
    }

    /// BACK: every open parameter goes back to where its gesture began, then the gesture ends. The
    /// record sees a gesture that changed nothing, so no undo step is left behind.
    pub(crate) fn revert_open(&mut self) {
        let before = self.open.before.clone();
        for (index, value) in before {
            self.send(index, value);
        }
        self.close_open();
    }

    /// One complete gesture: every write begun, sent and ended together, as one undo action.
    pub(crate) fn instant(&mut self, writes: &[(usize, f32)]) {
        let Some(gui) = self.gui.clone() else {
            return;
        };
        let setter = gui.param_setter();
        let params = self.params.clone();
        let edits: Vec<(&dyn ErasedParam, f32)> = writes
            .iter()
            .map(|&(index, value)| (params.param(index), value))
            .collect();
        mxm_params::binding::set_together(&setter, &edits);
    }

    /// The normalised value of a host parameter, as `plain` would set it.
    pub(crate) fn normalised(&self, index: usize, plain: f32) -> f32 {
        let params = &self.params;
        if index >= crate::params::PLACE_BASE {
            let place = &params.places[(index - crate::params::PLACE_BASE).min(SLOTS - 1)];
            return place.placement.preview_normalized(plain.round() as i32);
        }
        if index >= crate::params::KEY_BASE {
            let key = &params.keys[(index - crate::params::KEY_BASE).min(SLOTS - 1)];
            return key.side_chain.preview_normalized(plain >= 0.5);
        }
        if index >= crate::params::DYNAMICS_BASE {
            let at = index - crate::params::DYNAMICS_BASE;
            let fields = crate::params::DYNAMICS_FIELDS.len();
            let dynamics = &params.dynamics[(at / fields).min(SLOTS - 1)];
            return match at % fields {
                0 => dynamics.range.preview_normalized(plain),
                1 => dynamics.threshold.preview_normalized(plain),
                2 => dynamics.attack.preview_normalized(plain),
                _ => dynamics.release.preview_normalized(plain),
            };
        }
        if index >= OUTPUT_INDEX {
            return if index == OUTPUT_INDEX {
                params.output.preview_normalized(plain)
            } else {
                params.gain_scale.preview_normalized(plain)
            };
        }
        let band = &params.bands[index / BAND_FIELDS.len()];
        match index % BAND_FIELDS.len() {
            0 => band.kind.preview_normalized(plain as i32),
            1 => {
                if plain >= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            2 => band.freq.preview_normalized(plain),
            3 => band.gain.preview_normalized(plain),
            4 => band.q.preview_normalized(plain),
            _ => band.slope.preview_normalized(plain as i32),
        }
    }

    /// A host parameter's plain value in the running gesture, or as it reads.
    pub(crate) fn current_plain(&self, index: usize) -> f32 {
        let params = &self.params;
        let normalised = self.normalised_now(index);
        if index >= crate::params::PLACE_BASE {
            let place = &self.params.places[(index - crate::params::PLACE_BASE).min(SLOTS - 1)];
            return place.placement.preview_plain(normalised) as f32;
        }
        if index >= crate::params::KEY_BASE {
            return normalised;
        }
        if index >= crate::params::DYNAMICS_BASE {
            let at = index - crate::params::DYNAMICS_BASE;
            let fields = crate::params::DYNAMICS_FIELDS.len();
            let dynamics = &params.dynamics[(at / fields).min(SLOTS - 1)];
            return match at % fields {
                0 => dynamics.range.preview_plain(normalised),
                1 => dynamics.threshold.preview_plain(normalised),
                2 => dynamics.attack.preview_plain(normalised),
                _ => dynamics.release.preview_plain(normalised),
            };
        }
        if index >= OUTPUT_INDEX {
            return if index == OUTPUT_INDEX {
                params.output.preview_plain(normalised)
            } else {
                params.gain_scale.preview_plain(normalised)
            };
        }
        let band = &params.bands[index / BAND_FIELDS.len()];
        match index % BAND_FIELDS.len() {
            0 => band.kind.preview_plain(normalised) as f32,
            1 => normalised,
            2 => band.freq.preview_plain(normalised),
            3 => band.gain.preview_plain(normalised),
            4 => band.q.preview_plain(normalised),
            _ => band.slope.preview_plain(normalised) as f32,
        }
    }

    /// The lowest Off slot, where a new band goes (plan §2.1).
    fn free_slot(&self) -> Option<usize> {
        self.free_slots().next()
    }

    /// The Off slots, lowest first.
    fn free_slots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..SLOTS).filter(|&slot| {
            !self.band_in_use(slot) && !self.open.holds(band_index(slot, Field::Type))
        })
    }

    /// Every field of `slot`, set to a band: what a new band or a copy writes, so nothing a deleted
    /// band left in the slot comes back.
    fn slot_writes(
        &self,
        slot: usize,
        band: mxm_fx_eq_n_dsp::Band,
        dynamics: mxm_fx_eq_n_dsp::Dynamics,
        keyed: bool,
        placement: mxm_fx_eq_n_dsp::Placement,
    ) -> [(usize, f32); 12] {
        let at = |field| band_index(slot, field);
        let dyn_at = |field| dynamics_index(slot, field);
        [
            (
                at(Field::Type),
                self.normalised(at(Field::Type), type_of(band.shape) as f32),
            ),
            (at(Field::Bypass), if band.bypass { 1.0 } else { 0.0 }),
            (at(Field::Freq), self.normalised(at(Field::Freq), band.freq)),
            (
                at(Field::Gain),
                self.normalised(at(Field::Gain), band.gain_db),
            ),
            (at(Field::Q), self.normalised(at(Field::Q), band.q)),
            (
                at(Field::Slope),
                self.normalised(
                    at(Field::Slope),
                    crate::params::slope_value(band.slope) as f32,
                ),
            ),
            (
                dyn_at(DynamicsField::Range),
                self.normalised(dyn_at(DynamicsField::Range), dynamics.range_db),
            ),
            (
                dyn_at(DynamicsField::Threshold),
                self.normalised(dyn_at(DynamicsField::Threshold), dynamics.threshold_db),
            ),
            (
                dyn_at(DynamicsField::Attack),
                self.normalised(dyn_at(DynamicsField::Attack), dynamics.attack_ms),
            ),
            (
                dyn_at(DynamicsField::Release),
                self.normalised(dyn_at(DynamicsField::Release), dynamics.release_ms),
            ),
            (key_index(slot), if keyed { 1.0 } else { 0.0 }),
            (
                place_index(slot),
                self.normalised(
                    place_index(slot),
                    crate::params::placement_value(placement) as f32,
                ),
            ),
        ]
    }

    /// A slot's placement, as this editor last set it.
    pub(crate) fn placement_now(&self, slot: usize) -> mxm_fx_eq_n_dsp::Placement {
        crate::params::placement_of(self.current_plain(place_index(slot)).round() as i32)
    }

    /// Whether the layout hears a slot's placement: in mono only Stereo is (plan §7.3).
    pub(crate) fn placement_heard(&self, slot: usize) -> bool {
        !self.telemetry.mono() || self.placement_now(slot) == mxm_fx_eq_n_dsp::Placement::Stereo
    }

    /// The phase the parameters ask for, as this editor last set them (plan §7.7).
    pub(crate) fn phase_asked(&self) -> mxm_fx_eq_n_dsp::Phase {
        let plain =
            |index: usize, param: &IntParam| param.preview_plain(self.normalised_now(index));
        crate::params::phase_of(
            plain(crate::params::PHASE_INDEX, &self.params.phase),
            plain(crate::params::RESOLUTION_INDEX, &self.params.resolution),
        )
    }

    /// The phase mode (0 Zero latency, 1 Linear phase) or Linear phase's resolution, as one undo
    /// step: asked for, and heard once the host restarts the plugin (plan §7.7).
    pub(crate) fn set_phase_value(&mut self, index: usize, value: i32) {
        self.close_open();
        self.instant(&[(index, self.normalised(index, value as f32))]);
    }

    pub(crate) fn set_placement(&mut self, slot: usize, placement: mxm_fx_eq_n_dsp::Placement) {
        self.close_open();
        let index = place_index(slot);
        self.instant(&[(
            index,
            self.normalised(index, crate::params::placement_value(placement) as f32),
        )]);
    }

    /// Why Split cannot act on `slot` now, or `None` when it can (plan §7.3): only a Stereo band in
    /// a stereo layout splits, into a free slot.
    pub(crate) fn split_refused(&self, slot: usize) -> Option<SplitRefusal> {
        if self.telemetry.mono() {
            Some(SplitRefusal::Mono)
        } else if self.placement_now(slot) != mxm_fx_eq_n_dsp::Placement::Stereo {
            Some(SplitRefusal::Placed)
        } else if self.free_slot().is_none() {
            Some(SplitRefusal::Full)
        } else {
            None
        }
    }

    /// Split: the band becomes Left (or Mid) and a copy of it in the lowest Off slot Right (or
    /// Side), every other field copied, as one undo step; the original stays selected.
    pub(crate) fn split(&mut self, slot: usize, mid_side: bool) {
        if self.split_refused(slot).is_some() {
            return;
        }
        let Some(copy) = self.free_slot() else {
            return;
        };
        use mxm_fx_eq_n_dsp::Placement;
        let (kept, made) = if mid_side {
            (Placement::Mid, Placement::Side)
        } else {
            (Placement::Left, Placement::Right)
        };
        let band = self.band_now(slot);
        let dynamics = self.dynamics_now(slot);
        let keyed = self.key_now(slot);
        self.close_open();
        let mut writes: Vec<(usize, f32)> =
            self.slot_writes(copy, band, dynamics, keyed, made).to_vec();
        let index = place_index(slot);
        writes.push((
            index,
            self.normalised(index, crate::params::placement_value(kept) as f32),
        ));
        self.instant(&writes);
    }

    /// Whether a slot listens to the side chain, as this editor last set it.
    pub(crate) fn key_now(&self, slot: usize) -> bool {
        self.normalised_now(key_index(slot)) >= 0.5
    }

    /// Whether a slot's band hears the side chain: its switch on and its shape a dynamic one, so a
    /// switch kept from an earlier shape means nothing (plan §7.2).
    pub(crate) fn listens_to_key(&self, slot: usize) -> bool {
        self.key_now(slot)
            && self
                .band_now(slot)
                .shape
                .is_some_and(|shape| shape.is_dynamic())
    }

    /// A slot's dynamics as this editor last set them ([`normalised_now`](Self::normalised_now)'s
    /// values).
    pub(crate) fn dynamics_now(&self, slot: usize) -> mxm_fx_eq_n_dsp::Dynamics {
        let params = &self.params.dynamics[slot];
        let now = |field| self.normalised_now(dynamics_index(slot, field));
        mxm_fx_eq_n_dsp::Dynamics {
            range_db: params.range.preview_plain(now(DynamicsField::Range)),
            threshold_db: params
                .threshold
                .preview_plain(now(DynamicsField::Threshold)),
            attack_ms: params.attack.preview_plain(now(DynamicsField::Attack)),
            release_ms: params.release.preview_plain(now(DynamicsField::Release)),
        }
    }

    /// A 0 dB bell in the lowest Off slot, selected; `None` when every slot is in use.
    pub(crate) fn insert_at(&mut self, freq: f32) -> Option<usize> {
        self.insert(freq, false)
    }

    /// Ends a freeze, its hold and its peaks with it.
    pub(crate) fn unfreeze(&mut self) {
        self.frozen = None;
        self.freeze_asked = false;
        self.focus_peak = None;
        self.peak_labels.clear();
        self.hold.clear();
        self.hold_basis = None;
    }

    /// Ends a freeze whose basis has gone (plan §7.4): the analyser off, a setting changed, or the
    /// first audio after a reset or at a new rate. Called before the keyboard and the pointer, so
    /// no input grabs a peak the frame's audio has already made stale; painting checks again.
    pub(crate) fn end_stale_freeze(&mut self, settings: &crate::view::ViewSettings) {
        let basis = grab::Basis {
            view: settings.display(),
            generation: self.spectrum.generation(),
        };
        let stale = !settings.analyser
            || self
                .frozen
                .as_ref()
                .is_some_and(|frozen| frozen.basis != basis);
        if stale && (self.freezing() || self.hold_basis.is_some()) {
            self.unfreeze();
        }
    }

    /// The pre, post and side-chain lines as drawn: a freeze's while frozen, else the live ones.
    pub(crate) fn shown_spectra(&self) -> [&[f32]; 3] {
        match &self.frozen {
            Some(frozen) => [&frozen.pre, &frozen.post, &frozen.key],
            None => [
                &self.spectrum.pre_db,
                &self.spectrum.post_db,
                &self.spectrum.key_db,
            ],
        }
    }

    /// Freeze turned on (plan §7.4): made where the spectrum is drawn next. It ends a listen, as
    /// Find and Match end a freeze (plan §7.5, §7.6).
    pub(crate) fn ask_freeze(&mut self) {
        self.end_find();
        self.end_match();
        self.freeze_asked = true;
    }

    // ---- Find (plan §7.5) -----------------------------------------------------------------

    /// Turns Find on, ending a freeze (the two exclude each other), with a listen from nothing;
    /// or off.
    pub(crate) fn set_find(&mut self, on: bool) {
        if on {
            if !self.find {
                self.unfreeze();
                self.end_match();
                self.finder.restart();
                self.listen_seen = self.finder.listen();
                self.find = true;
            }
        } else {
            self.end_find();
        }
    }

    /// Ends a listen: what it heard and proposed goes with it.
    pub(crate) fn end_find(&mut self) {
        self.find = false;
        self.finder.restart();
        self.listen_seen = self.finder.listen();
        self.candidates.clear();
        self.found_at = None;
        self.found_revision = u64::MAX;
        self.proposals.clear();
        self.proposed_on.clear();
        self.proposals_stale = false;
        self.proposals_version += 1;
        self.focus_proposal = None;
        self.proposal_labels.clear();
        // Its thread ends with it, once any search running has.
        self.worker = None;
    }

    /// The EQ as heard (plan §7.5): every band in the sound, at its drawn gain under the gain
    /// scale, with its placement; a bypassed band and a placed band in mono are not in it.
    pub(crate) fn heard_eq(&self) -> finder::HeardEq {
        let sample_rate = self.sample_rate();
        let scale = self.gain_scale_now();
        let bands = (0..SLOTS)
            .filter_map(|slot| {
                let band = self.band_now(slot);
                let shape = band
                    .shape
                    .filter(|_| !band.bypass && self.placement_heard(slot))?;
                let cascade = mxm_fx_eq_n_dsp::design(
                    mxm_fx_eq_n_dsp::Design {
                        shape,
                        slope: band.slope,
                        freq: f64::from(band.freq),
                        gain_db: f64::from(band.gain_db * scale),
                        q: f64::from(band.q),
                    },
                    sample_rate,
                );
                Some((cascade, self.placement_now(slot)))
            })
            .collect();
        finder::HeardEq {
            bands,
            sample_rate,
            deepest_db: mxm_fx_eq_n_dsp::GAIN_RANGE as f32 * scale,
        }
    }

    /// Why Find proposes nothing although it listens: every slot in use, or the gain scale at 0 %,
    /// where a bell is flat.
    pub(crate) fn find_refusal(&self) -> Option<FindRefusal> {
        if self.free_slots().next().is_none() {
            Some(FindRefusal::Full)
        } else if self.gain_scale_now() <= f32::EPSILON {
            Some(FindRefusal::NoGain)
        } else {
            None
        }
    }

    /// What proposals depend on besides the candidates: every parameter as this editor last set
    /// it, Amount and the layout.
    fn proposal_basis(&self, amount: u8) -> Vec<u32> {
        let mut basis: Vec<u32> = (0..self.pointers.len())
            .map(|index| self.normalised_now(index).to_bits())
            .collect();
        basis.push(u32::from(amount));
        basis.push(u32::from(self.telemetry.mono()));
        basis
    }

    /// Finds the candidates again every [`finder::UPDATE_SECONDS`] when the finder has heard
    /// something since, and proposes again when they or the EQ changed: at once after an apply,
    /// else at most ten times a second while a band moves. The search runs on the finder's
    /// worker, on a copy of what it has heard, so its fits never cost a frame; what it found is
    /// taken when it is done, and one search runs at a time.
    pub(crate) fn update_find(&mut self, now: f64, settings: &ViewSettings) {
        if !self.find {
            return;
        }
        // A reset restarted the listen: what the last one found goes with it.
        if self.finder.listen() != self.listen_seen {
            self.forget_listen();
        }
        if self.worker.as_ref().is_some_and(finder::Worker::busy) {
            return;
        }
        let due = self
            .found_at
            .is_none_or(|at| now - at >= finder::UPDATE_SECONDS || now < at);
        let heard_new = due && self.finder.revision() != self.found_revision;
        if due {
            self.found_at = Some(now);
        }
        let basis = self.proposal_basis(settings.amount);
        let eq_moved = basis != self.proposed_on && now - self.proposed_at >= 0.1;
        if !(heard_new || self.proposals_stale || eq_moved) {
            return;
        }
        if self.find_refusal().is_some() {
            self.proposed_on = basis;
            self.proposed_at = now;
            self.proposals_stale = false;
            self.proposals.clear();
            self.proposals_version += 1;
            self.focus_proposal = None;
            return;
        }
        // Candidates are found again when the finder heard something since, or when the last
        // search that should have found them never ran.
        let refind = heard_new || self.refind_pending;
        let revision = self.finder.revision();
        let search = finder::Search {
            evidence: self.finder.evidence().clone(),
            candidates: (!refind).then(|| self.candidates.clone()),
            heard: self.heard_eq(),
            amount: settings.amount_share(),
            free_slots: self.free_slots().count(),
            shown: self.proposals.clone(),
            listen: self.finder.listen(),
        };
        let started = if self.inline_search {
            self.accept(search.run());
            true
        } else {
            if self.worker.is_none() {
                self.worker = finder::Worker::new();
            }
            // Never in the frame: without a thread to run it on (none could be made, or it has
            // gone), the search waits, and the next frame tries again with a new thread.
            self.worker
                .as_mut()
                .is_some_and(|worker| worker.start(Task::Search(search)).is_ok())
        };
        if started {
            if refind {
                self.found_revision = revision;
            }
            self.refind_pending = false;
            self.proposed_on = basis;
            self.proposed_at = now;
            self.proposals_stale = false;
        } else {
            self.worker = None;
            self.refind_pending |= refind;
            self.proposals_stale = true;
        }
    }

    /// What the worker gave back, if it has: a search's, shown while Find is on, a fit's while
    /// Match is.
    pub(crate) fn take_done(&mut self) {
        match self.worker.as_mut().and_then(finder::Worker::take) {
            Some(Done::Found(found)) if self.find => self.accept(found),
            Some(Done::Fitted(fitted)) if self.matching => self.accept_fit(fitted),
            _ => {}
        }
    }

    /// What a search found, shown, unless an earlier listen made it. The keyboard's proposal
    /// stays the one at its frequency, wherever it now stands in the list, or goes.
    fn accept(&mut self, found: finder::Found) {
        if found.listen != self.finder.listen() {
            return;
        }
        self.candidates = found.candidates;
        self.proposals = found.proposals;
        self.proposals_version += 1;
        let focus = self.focus_freq;
        self.focus_proposal = self.focus_proposal.and_then(|_| {
            self.proposals
                .iter()
                .position(|proposal| (proposal.freq / focus).log2().abs() <= finder::HALF_SEMITONE)
        });
        if let Some(index) = self.focus_proposal {
            self.focus_freq = self.proposals[index].freq;
        }
    }

    /// The listen started again — a reset, a new rate — so what was found and proposed goes, and
    /// a search still running for the old one with it.
    pub(crate) fn forget_listen(&mut self) {
        self.worker = None;
        self.candidates.clear();
        self.proposals.clear();
        self.proposals_version += 1;
        self.focus_proposal = None;
        self.found_at = None;
        self.found_revision = u64::MAX;
        self.proposed_on.clear();
        self.refind_pending = false;
        self.listen_seen = self.finder.listen();
    }

    /// Applies proposals (plan §7.5): each band a static, unkeyed bell at its placement in the
    /// lowest Off slots, every field written, its gain the depth heard under the gain scale. One
    /// undo step; `opened` keeps the fields in the drag's gesture that follows. Returns the slots,
    /// empty when they do not fit.
    pub(crate) fn apply_proposals(&mut self, which: &[usize], opened: bool) -> Vec<usize> {
        let scale = self.gain_scale_now();
        let bands: Vec<finder::ProposedBand> = which
            .iter()
            .filter_map(|&index| self.proposals.get(index))
            .flat_map(|proposal| proposal.bands.iter().copied())
            .collect();
        let slots: Vec<usize> = self.free_slots().take(bands.len()).collect();
        if bands.is_empty() || slots.len() < bands.len() || scale <= f32::EPSILON {
            return Vec::new();
        }
        let range = mxm_fx_eq_n_dsp::GAIN_RANGE as f32;
        let mono = self.telemetry.mono();
        let mut writes: Vec<(usize, f32)> = Vec::new();
        for (&slot, band) in slots.iter().zip(&bands) {
            let placement = if mono {
                mxm_fx_eq_n_dsp::Placement::Stereo
            } else {
                band.domain.placement()
            };
            writes.extend(self.slot_writes(
                slot,
                mxm_fx_eq_n_dsp::Band {
                    shape: Some(Shape::Bell),
                    bypass: false,
                    freq: self.clamp_freq(band.freq),
                    gain_db: (band.gain_db / scale).clamp(-range, range),
                    q: band.q,
                    slope: DEFAULT_SLOPE,
                },
                mxm_fx_eq_n_dsp::Dynamics::STATIC,
                false,
                placement,
            ));
        }
        self.close_open();
        if opened {
            for (index, value) in writes {
                self.send(index, value);
            }
        } else {
            self.instant(&writes);
        }
        self.selected = slots.first().copied();
        self.edge = None;
        if let Some(band) = bands.first() {
            self.focus_freq = self.clamp_freq(band.freq);
        }
        // What was applied leaves the proposals at once, so it cannot be applied twice before the
        // next search shows what is left.
        let mut index = 0;
        self.proposals.retain(|_| {
            index += 1;
            !which.contains(&(index - 1))
        });
        self.proposals_version += 1;
        self.focus_proposal = None;
        self.proposals_stale = true;
        slots
    }

    // ---- Match (plan §7.6) ----------------------------------------------------------------

    /// Whether Match is offered: the analyser on, and a layout with the side-chain port.
    pub(crate) fn match_offered(&self, settings: &ViewSettings) -> bool {
        settings.analyser && self.telemetry.side_chain_port()
    }

    /// Turns Match on, ending Find and a freeze (one proposal set at a time), with a listen from
    /// nothing; or off.
    pub(crate) fn set_match(&mut self, on: bool) {
        if on {
            if !self.matching {
                self.unfreeze();
                self.end_find();
                self.matcher.restart();
                self.match_listen_seen = self.matcher.listen();
                self.matching = true;
            }
        } else {
            self.end_match();
        }
    }

    /// Ends Match: what it heard and proposed goes with it.
    pub(crate) fn end_match(&mut self) {
        if !self.matching && self.match_bands.is_empty() {
            return;
        }
        self.matching = false;
        self.matcher.restart();
        self.forget_match();
    }

    /// The listen started again — Match off, a reset, a new rate — so what was proposed goes, and a
    /// fit still running for the old one with it.
    pub(crate) fn forget_match(&mut self) {
        self.worker = None;
        self.match_bands.clear();
        self.match_version += 1;
        self.match_fitted_on.clear();
        self.match_asked_on.clear();
        self.match_asked_at = None;
        self.match_revision = u64::MAX;
        self.match_listen_seen = self.matcher.listen();
    }

    /// What a fit depends on besides what was heard: every parameter as this editor last set it
    /// (the EQ as heard and the free slots), Amount, Detail and the layout.
    pub(crate) fn match_basis(&self, settings: &ViewSettings) -> Vec<u32> {
        let mut basis: Vec<u32> = (0..self.pointers.len())
            .map(|index| self.normalised_now(index).to_bits())
            .collect();
        basis.push(u32::from(settings.match_amount));
        basis.push(u32::from(settings.match_detail()));
        basis.push(u32::from(self.telemetry.mono()));
        basis
    }

    /// Fits again every [`finder::UPDATE_SECONDS`] while Match hears something new, and whenever the
    /// EQ, Amount or Detail moves; on the worker, the latest winning: while one runs nothing is
    /// asked, and the next is asked on what is current then.
    pub(crate) fn update_match(&mut self, now: f64, settings: &ViewSettings) {
        if !self.matching {
            return;
        }
        if self.matcher.listen() != self.match_listen_seen {
            self.forget_match();
        }
        if self.worker.as_ref().is_some_and(finder::Worker::busy) {
            return;
        }
        let due = self
            .match_asked_at
            .is_none_or(|at| now - at >= finder::UPDATE_SECONDS || now < at);
        let heard_new = due && self.matcher.revision() != self.match_revision;
        let basis = self.match_basis(settings);
        if !(heard_new || basis != self.match_asked_on) || !self.matcher.spectra().listened() {
            return;
        }
        if self.find_refusal().is_some() {
            self.match_asked_on = basis;
            if !self.match_bands.is_empty() {
                self.match_bands.clear();
                self.match_version += 1;
            }
            return;
        }
        let revision = self.matcher.revision();
        let fit = matcher::Fit {
            spectra: self.matcher.spectra().clone(),
            heard: self.heard_eq(),
            amount: settings.match_amount_share(),
            max_bands: usize::from(settings.match_detail()).min(self.free_slots().count()),
            listen: self.matcher.listen(),
            basis: basis.clone(),
        };
        let started = if self.inline_search {
            self.accept_fit(fit.run());
            true
        } else {
            if self.worker.is_none() {
                self.worker = finder::Worker::new();
            }
            // Never in the frame: without a thread, the fit waits for the next frame's.
            self.worker
                .as_mut()
                .is_some_and(|worker| worker.start(Task::Fit(fit)).is_ok())
        };
        if started {
            self.match_revision = revision;
            self.match_asked_on = basis;
            if heard_new {
                self.match_asked_at = Some(now);
            }
        } else {
            self.worker = None;
        }
    }

    /// What a fit proposes, shown, unless an earlier listen made it.
    pub(crate) fn accept_fit(&mut self, fitted: matcher::Fitted) {
        if fitted.listen != self.matcher.listen() {
            return;
        }
        self.match_bands = fitted.bands;
        self.match_fitted_on = fitted.basis;
        self.match_version += 1;
    }

    /// Whether the bands shown were fitted on the EQ, the free slots, Amount and Detail as they
    /// stand: only then does Apply write them (plan §7.6). What was heard moves on, and a fit half a
    /// second old is still the match.
    pub(crate) fn match_current(&self, settings: &ViewSettings) -> bool {
        !self.match_bands.is_empty() && self.match_fitted_on == self.match_basis(settings)
    }

    /// Applies the match (plan §7.6): every band a static, unkeyed Stereo bell or shelf in the
    /// lowest Off slots, every field written, its gain the gain heard under the gain scale. One
    /// undo step. Returns the slots, empty when the bands are not current or do not fit.
    pub(crate) fn apply_match(&mut self, settings: &ViewSettings) -> Vec<usize> {
        let scale = self.gain_scale_now();
        let bands = self.match_bands.clone();
        let slots: Vec<usize> = self.free_slots().take(bands.len()).collect();
        if !self.match_current(settings) || slots.len() < bands.len() || scale <= f32::EPSILON {
            return Vec::new();
        }
        let range = mxm_fx_eq_n_dsp::GAIN_RANGE as f32;
        let mut writes: Vec<(usize, f32)> = Vec::new();
        for (&slot, band) in slots.iter().zip(&bands) {
            writes.extend(self.slot_writes(
                slot,
                mxm_fx_eq_n_dsp::Band {
                    shape: Some(band.shape),
                    bypass: false,
                    freq: self.clamp_freq(band.freq),
                    gain_db: (band.gain_db / scale).clamp(-range, range),
                    q: band.q,
                    slope: DEFAULT_SLOPE,
                },
                mxm_fx_eq_n_dsp::Dynamics::STATIC,
                false,
                mxm_fx_eq_n_dsp::Placement::Stereo,
            ));
        }
        self.close_open();
        self.instant(&writes);
        self.selected = slots.first().copied();
        self.edge = None;
        if let Some(band) = bands.first() {
            self.focus_freq = self.clamp_freq(band.freq);
        }
        // What was applied leaves the proposals at once; the next fit shows what is left.
        self.match_bands.clear();
        self.match_fitted_on.clear();
        self.match_version += 1;
        slots
    }

    /// Whether a band can still be made: an Off slot is free.
    pub(crate) fn free_slot_exists(&self) -> bool {
        self.free_slot().is_some()
    }

    /// Whether Freeze is on or asked for, as its toggle shows.
    pub(crate) fn freezing(&self) -> bool {
        self.frozen.is_some() || self.freeze_asked
    }

    /// A grab (plan §7.4): a static bell at a peak, its Q from the peak's width, in the lowest Off
    /// slot; `opened` keeps its fields in the drag's gesture.
    pub(crate) fn insert_peak(&mut self, peak: grab::Peak, opened: bool) -> Option<usize> {
        self.insert_with(peak.freq, peak.q, opened)
    }

    /// The same bell, its fields opened in the gesture that follows — a drag from the line — so
    /// making it and moving it are one action, and BACK takes it away leaving nothing to undo.
    pub(crate) fn insert_opened(&mut self, freq: f32) -> Option<usize> {
        self.insert(freq, true)
    }

    fn insert(&mut self, freq: f32, opened: bool) -> Option<usize> {
        self.insert_with(freq, 1.0, opened)
    }

    fn insert_with(&mut self, freq: f32, q: f32, opened: bool) -> Option<usize> {
        let slot = self.free_slot()?;
        let freq = self.clamp_freq(freq);
        let writes = self.slot_writes(
            slot,
            mxm_fx_eq_n_dsp::Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq,
                gain_db: 0.0,
                q,
                slope: DEFAULT_SLOPE,
            },
            // A new band is static and hears its own input, whatever an earlier band in the slot
            // left.
            mxm_fx_eq_n_dsp::Dynamics::STATIC,
            false,
            mxm_fx_eq_n_dsp::Placement::Stereo,
        );
        self.close_open();
        if opened {
            for (index, value) in writes {
                self.send(index, value);
            }
        } else {
            self.instant(&writes);
        }
        self.selected = Some(slot);
        self.edge = None;
        self.focus_freq = freq;
        Some(slot)
    }

    pub(crate) fn delete(&mut self, slot: usize) {
        self.close_open();
        let index = band_index(slot, Field::Type);
        self.instant(&[(index, 0.0)]);
        if self.selected == Some(slot) {
            self.selected = None;
            self.edge = None;
            self.focus_freq = self.clamp_freq(self.band_now(slot).freq);
        }
    }

    pub(crate) fn set_shape(&mut self, slot: usize, shape: Shape) {
        self.close_open();
        let index = band_index(slot, Field::Type);
        self.instant(&[(index, self.normalised(index, type_of(Some(shape)) as f32))]);
    }

    pub(crate) fn set_slope(&mut self, slot: usize, slope: Slope) {
        self.close_open();
        let index = band_index(slot, Field::Slope);
        let value = crate::params::slope_value(slope) as f32;
        self.instant(&[(index, self.normalised(index, value))]);
    }

    /// The column's toggle writes bypass through the binding; a test writes it here.
    #[cfg(test)]
    pub(crate) fn set_bypass(&mut self, slot: usize, bypass: bool) {
        self.close_open();
        self.instant(&[(
            band_index(slot, Field::Bypass),
            if bypass { 1.0 } else { 0.0 },
        )]);
    }

    // ---- undo -----------------------------------------------------------------------------

    pub(crate) fn undo(&mut self) {
        self.close_open();
        let writes = history::lock(&self.history).take_undo();
        self.apply_history(writes);
    }

    pub(crate) fn redo(&mut self) {
        self.close_open();
        let writes = history::lock(&self.history).take_redo();
        self.apply_history(writes);
    }

    fn apply_history(&mut self, writes: Option<Vec<(usize, f32)>>) {
        if let (Some(writes), Some(host)) = (writes, self.host.as_ref()) {
            history::write(host, &self.history, &self.pointers, &writes);
            // The band the action touched is shown, so what changed is in front of the player.
            if let Some(slot) = writes.iter().find_map(|&(index, _)| slot_of(index)) {
                let shape_index = band_index(slot, Field::Type);
                let shape = writes
                    .iter()
                    .find(|(index, _)| *index == shape_index)
                    .map(|&(_, value)| value);
                if shape != Some(0.0) {
                    self.selected = Some(slot);
                }
            }
        }
    }

    // ---- solo -----------------------------------------------------------------------------

    pub(crate) fn soloed(&self) -> Option<usize> {
        self.audition.slot()
    }

    /// Solo, from the Solo button's latch or a point held, to the shell. Only a band in the sound
    /// can be soloed: a bypassed band, the tilt and an Off slot are refused here as the DSP refuses
    /// them, and a latch on one is let go.
    pub(crate) fn publish_solo(&mut self) {
        let (endings, stored) = self.audition.state();
        if endings != self.solo_endings {
            // The shell ended a solo (a reset, a deactivation or a restored state): the latch goes,
            // and whatever press holds one now may not bring it back.
            self.solo_endings = endings;
            self.solo_blocked = true;
            self.latched = None;
        }
        // A latch is the selected band's, and only while that band is in the sound.
        if let Some(slot) = self.latched
            && (self.selected != Some(slot) || !self.offers_solo(slot))
        {
            self.latched = None;
        }
        let held = self.pointer_solo;
        if held.is_none() {
            self.solo_blocked = false;
        }
        let wanted = held
            .filter(|_| !self.solo_blocked)
            .or(self.latched)
            .filter(|&slot| self.offers_solo(slot));
        if wanted != stored {
            // Refused only if the shell ended a solo since `endings` was read; the next frame sees it.
            self.audition.request(endings, wanted);
        }
    }

    /// The Solo button, pressed: latches `slot`'s solo, or lets it go.
    pub(crate) fn toggle_solo(&mut self, slot: usize) {
        self.latched = (self.latched != Some(slot)).then_some(slot);
    }

    /// Whether the gain scale changes anything heard: a band in the sound with a gain.
    pub(crate) fn gain_heard(&self) -> bool {
        (0..SLOTS).any(|slot| {
            let band = self.band_now(slot);
            !band.bypass && band.shape.is_some_and(Shape::has_gain) && self.placement_heard(slot)
        })
    }

    /// A placed band in mono is not in the sound, so it offers no solo either (plan §7.3).
    pub(crate) fn offers_solo(&self, slot: usize) -> bool {
        let band = self.band_now(slot);
        band.shape.is_some_and(Shape::offers_solo) && !band.bypass && self.placement_heard(slot)
    }

    fn end_solo(&mut self) {
        let (endings, stored) = self.audition.state();
        if stored.is_some() {
            self.audition.request(endings, None);
        }
        self.pointer_solo = None;
        self.latched = None;
        self.solo_blocked = false;
    }

    // ---- the keyboard language on the graph ----------------------------------------------------

    /// One output of the keyboard language with the cursor on the graph (`navigation::surface`).
    pub(crate) fn on_surface(&mut self, output: Output) {
        match output {
            Output::Navigate { direction, coarse } => {
                self.end_gesture(true);
                self.walk(direction, coarse);
            }
            // The start edge (←) or the end (→); the language gives ↑ ↓ no edge.
            Output::Within { direction } => {
                let edge = match direction {
                    Direction::Left => Some(Edge::Start),
                    Direction::Right => Some(Edge::End),
                    Direction::Up | Direction::Down => None,
                };
                if let Some(edge) = edge
                    && self.selected.is_some_and(|slot| self.has_q(slot))
                {
                    self.edge = Some(edge);
                }
            }
            Output::Step {
                verb,
                step,
                direction,
            } => self.step(verb, step, direction),
            Output::Finish => self.end_gesture(true),
            Output::Cancel => self.end_gesture(false),
            Output::Action(Action::Add) => {
                self.end_gesture(true);
                // On a proposal, ADD applies it (plan §7.5).
                let proposal = self
                    .focus_proposal
                    .filter(|_| self.find && self.selected.is_none())
                    .filter(|&index| {
                        self.proposals.get(index).is_some_and(|proposal| {
                            (proposal.freq - self.focus_freq).abs() < 1.0e-3
                        })
                    });
                if let Some(index) = proposal {
                    self.apply_proposals(&[index], false);
                    return;
                }
                // On a frozen peak, ADD grabs it: its bell, its Q (plan §7.4).
                let peak = self
                    .focus_peak
                    .filter(|_| self.selected.is_none())
                    .and_then(|index| self.frozen.as_ref()?.peaks.get(index).copied())
                    .filter(|peak| (peak.freq - self.focus_freq).abs() < 1.0e-3);
                match peak {
                    Some(peak) => {
                        self.insert_peak(peak, false);
                    }
                    None => {
                        self.insert_at(self.focus_freq);
                    }
                }
            }
            // The selected band's right-click menu; on the line, with no band, there is none.
            Output::Action(Action::Menu) => {
                self.end_gesture(true);
                if let Some(slot) = self.selected {
                    self.context_slot = Some(slot);
                    self.menu_asked = true;
                }
            }
            Output::Action(Action::Remove | Action::Delete | Action::Ripple) => {
                self.end_gesture(true);
                if let Some(slot) = self.selected {
                    self.delete(slot);
                }
            }
            // BACK with nothing to cancel goes back a level: from an edge to its band, from a band
            // to the line.
            Output::Back => {
                let had_edge = self.edge.take().is_some();
                if !had_edge {
                    self.selected = None;
                }
            }
            _ => {}
        }
    }

    /// The patch was replaced under the editor — a host restore, a preset load — or the band a
    /// gesture was on went Off: every gesture open, a column control's too, ends where the host now
    /// has it, writing nothing ([`history::History::abandon_open`]), so BACK cannot put back a value
    /// from before. The drag, the copy and egui's own drag end with it.
    pub(crate) fn abandon_gesture(&mut self, ctx: &egui::Context) {
        let ended = {
            let mut history = history::lock(&self.history);
            history.abandon_open();
            history.take_to_end()
        };
        if let Some(host) = self.host.clone() {
            let setter = host.param_setter();
            let params = self.params.clone();
            for index in ended {
                params.param(index).end(&setter);
            }
        }
        self.open = Open::default();
        self.wheel_until = None;
        self.copy = None;
        self.drag = None;
        // A solo latched on the patch replaced is let go with it.
        self.latched = None;
        ctx.stop_dragging();
    }

    /// The gesture is over: kept as one undo step, or put back.
    fn end_gesture(&mut self, keep: bool) {
        if keep {
            self.close_open();
        } else {
            self.revert_open();
            if let Some((_, original)) = self.copy {
                self.selected = Some(original);
            }
        }
        self.copy = None;
    }

    fn has_q(&self, slot: usize) -> bool {
        let band = self.band_now(slot);
        band.shape.is_some_and(|shape| shape.has_q(band.slope))
    }

    /// The bands in use, by where they run then slot.
    pub(crate) fn by_frequency(&self) -> Vec<(f32, usize)> {
        let mut bands: Vec<(f32, usize)> = (0..SLOTS)
            .filter(|&slot| self.band_in_use(slot))
            .map(|slot| (self.clamp_freq(self.band_now(slot).freq), slot))
            .collect();
        bands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        bands
    }

    /// ← → go to the previous or next band by frequency. The keyboard's place is on the line, so
    /// with no band that way it steps along it: a semitone, or an octave with COARSE.
    fn walk(&mut self, direction: Direction, coarse: bool) {
        let right = match direction {
            Direction::Right => true,
            Direction::Left => false,
            Direction::Up | Direction::Down => return,
        };
        // With Find's proposals and no band selected, ← → walk the proposals (§7.5).
        if self.selected.is_none() && !coarse && self.find && !self.proposals.is_empty() {
            let mut by_freq: Vec<(f32, usize)> = self
                .proposals
                .iter()
                .enumerate()
                .map(|(index, proposal)| (proposal.freq, index))
                .collect();
            by_freq.sort_by(|a, b| a.0.total_cmp(&b.0));
            let from = self.focus_freq;
            let next = if right {
                by_freq.iter().find(|&&(freq, _)| freq > from * 1.0001)
            } else {
                by_freq
                    .iter()
                    .rev()
                    .find(|&&(freq, _)| freq < from / 1.0001)
            };
            if let Some(&(freq, index)) = next {
                self.focus_proposal = Some(index);
                self.focus_freq = freq;
            }
            return;
        }
        // Frozen with no band selected, ← → walk the labelled peaks instead of the line (§7.4).
        if self.selected.is_none()
            && !coarse
            && let Some(frozen) = &self.frozen
            && !frozen.peaks.is_empty()
        {
            let mut by_freq: Vec<(f32, usize)> = frozen
                .peaks
                .iter()
                .enumerate()
                .map(|(index, peak)| (peak.freq, index))
                .collect();
            by_freq.sort_by(|a, b| a.0.total_cmp(&b.0));
            let from = self.focus_freq;
            let next = if right {
                by_freq.iter().find(|&&(freq, _)| freq > from * 1.0001)
            } else {
                by_freq
                    .iter()
                    .rev()
                    .find(|&&(freq, _)| freq < from / 1.0001)
            };
            if let Some(&(freq, index)) = next {
                self.focus_peak = Some(index);
                self.focus_freq = freq;
            }
            return;
        }
        let bands = self.by_frequency();
        let index = self
            .selected
            .and_then(|slot| bands.iter().position(|&(_, other)| other == slot));
        let next = match index {
            Some(index) => if right {
                index.checked_add(1)
            } else {
                index.checked_sub(1)
            }
            .and_then(|next| bands.get(next))
            .map(|&(_, slot)| slot),
            None if right => bands
                .iter()
                .find(|&&(freq, _)| freq > self.focus_freq)
                .map(|&(_, slot)| slot),
            None => bands
                .iter()
                .rev()
                .find(|&&(freq, _)| freq < self.focus_freq)
                .map(|&(_, slot)| slot),
        };
        match next.filter(|_| !coarse) {
            Some(slot) => self.select(slot),
            None => {
                let from = index.map_or(self.focus_freq, |index| bands[index].0);
                self.selected = None;
                self.edge = None;
                let factor = if coarse { 2.0 } else { 2f32.powf(1.0 / 12.0) };
                self.focus_freq =
                    self.clamp_freq(if right { from * factor } else { from / factor });
            }
        }
    }

    /// An octave, a semitone or a cent.
    pub(crate) fn freq_factor(step: Size) -> f32 {
        match step {
            Size::Coarse => 2.0,
            Size::Fine => 2f32.powf(1.0 / 12.0),
            Size::Micro => 2f32.powf(1.0 / 1_200.0),
        }
    }

    /// 3, 0.5 and 0.1 dB.
    pub(crate) fn gain_step(step: Size) -> f32 {
        match step {
            Size::Coarse => 3.0,
            Size::Fine => 0.5,
            Size::Micro => 0.1,
        }
    }

    /// Q moves by a share of its logarithmic travel: 10, 1 and 0.1 %.
    pub(crate) fn q_factor(sign: f32, step: Size) -> f32 {
        let fraction = match step {
            Size::Coarse => 0.10,
            Size::Fine => 0.01,
            Size::Micro => 0.001,
        };
        let travel = (Q_MAX / Q_MIN).ln() as f32;
        (sign * fraction * travel).exp()
    }

    fn send_plain(&mut self, index: usize, plain: f32) {
        let normalised = self.normalised(index, plain);
        self.send(index, normalised);
    }

    /// One arrow of a verb's gesture on the selected band.
    fn step(&mut self, verb: Verb, step: Size, direction: Direction) {
        // A copy that cannot be made, with every slot in use, moves nothing either.
        if verb == Verb::Duplicate && self.copy.is_none() && !self.start_copy() {
            return;
        }
        let Some(slot) = self.selected else {
            // No band: MOVE walks the keyboard's place along the line.
            if verb == Verb::Move && matches!(direction, Direction::Left | Direction::Right) {
                let factor = Self::freq_factor(step);
                self.focus_freq = self.clamp_freq(if direction == Direction::Right {
                    self.focus_freq * factor
                } else {
                    self.focus_freq / factor
                });
            }
            return;
        };
        let up = matches!(direction, Direction::Right | Direction::Up);
        let sign = if up { 1.0 } else { -1.0 };
        let horizontal = matches!(direction, Direction::Left | Direction::Right);
        let band = self.band_now(slot);
        let Some(shape) = band.shape else {
            return;
        };
        let freq_index = band_index(slot, Field::Freq);
        let gain_index = band_index(slot, Field::Gain);
        let q_index = band_index(slot, Field::Q);
        let range = mxm_fx_eq_n_dsp::GAIN_RANGE as f32;
        match verb {
            Verb::Move if horizontal && self.edge.is_some() => {
                self.move_edge(slot, shape, step, up);
            }
            Verb::Move | Verb::Duplicate if horizontal => {
                // From where it runs, so a band held at this rate's limit moves at once.
                let freq = self.clamp_freq(self.current_plain(freq_index))
                    * Self::freq_factor(step).powf(sign);
                let freq = freq.clamp(FREQ_MIN as f32, self.high());
                self.send_plain(freq_index, freq);
                self.focus_freq = freq;
            }
            Verb::Move | Verb::Duplicate | Verb::Value if shape.has_gain() && !horizontal => {
                let gain = self.current_plain(gain_index) + sign * Self::gain_step(step);
                self.send_plain(gain_index, gain.clamp(-range, range));
            }
            Verb::Value if shape.has_gain() => {
                // ← → to the next round value of the step, as on any value.
                let size = Self::gain_step(step);
                let gain = self.current_plain(gain_index) / size;
                let next = if up {
                    (gain + 1e-4).floor() + 1.0
                } else {
                    (gain - 1e-4).ceil() - 1.0
                };
                self.send_plain(gain_index, (next * size).clamp(-range, range));
            }
            Verb::Extent if horizontal && shape.has_q(band.slope) => {
                // Wider is a lower Q.
                let q = self.current_plain(q_index) * Self::q_factor(-sign, step);
                self.send_plain(q_index, q.clamp(Q_MIN as f32, Q_MAX as f32));
            }
            _ => {}
        }
    }

    /// MOVE on an edge: that edge moves and the other stays where it is, so a bell, notch or band
    /// pass moves its centre too. A shelf's or a cut's edges are not symmetric, so there Q alone
    /// changes. Outward widens.
    fn move_edge(&mut self, slot: usize, shape: Shape, step: Size, right: bool) {
        let Some(edge) = self.edge else {
            return;
        };
        if !self.has_q(slot) {
            return;
        }
        let outward = matches!((edge, right), (Edge::Start, false) | (Edge::End, true));
        let q_index = band_index(slot, Field::Q);
        let freq_index = band_index(slot, Field::Freq);
        let old_q = self.current_plain(q_index);
        let q = (old_q * Self::q_factor(if outward { -1.0 } else { 1.0 }, step))
            .clamp(Q_MIN as f32, Q_MAX as f32);
        if matches!(shape, Shape::Bell | Shape::Notch | Shape::BandPass) {
            let centre = self.clamp_freq(self.current_plain(freq_index));
            let moved = match edge {
                Edge::Start => centre * graph::edge_ratio(old_q) / graph::edge_ratio(q),
                Edge::End => centre / graph::edge_ratio(old_q) * graph::edge_ratio(q),
            };
            let moved = moved.clamp(FREQ_MIN as f32, self.high());
            self.send_plain(freq_index, moved);
            self.focus_freq = self.clamp_freq(moved);
        }
        self.send_plain(q_index, q);
    }

    /// DUPLICATE's first arrow: the selected band, copied into the lowest Off slot inside the
    /// gesture, and the copy selected so the arrows take it. Its end keeps the copy; BACK takes it
    /// away again. Whether a copy was made.
    fn start_copy(&mut self) -> bool {
        let Some(original) = self.selected else {
            return false;
        };
        let Some(slot) = self.free_slot() else {
            return false;
        };
        let band = self.band_now(original);
        let dynamics = self.dynamics_now(original);
        let keyed = self.key_now(original);
        let placement = self.placement_now(original);
        self.close_open();
        for (index, value) in self.slot_writes(slot, band, dynamics, keyed, placement) {
            self.send(index, value);
        }
        self.copy = Some((slot, original));
        self.selected = Some(slot);
        self.edge = None;
        true
    }

    /// The analyser's settings change: saved with the project, which is marked dirty.
    pub(crate) fn set_view(&mut self, settings: ViewSettings) {
        if settings == self.params.view.get() {
            return;
        }
        self.params.view.edit(settings);
        if let Some(host) = &self.host {
            // The parameter-free transaction: the wrapper marks the project dirty and restores
            // nothing (nice-plug's PATCHES.md, *GUI-authored state dirty*).
            host.set_state(PluginState {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                params: Default::default(),
                fields: self.params.serialize_fields(),
            });
        }
    }
}

#[cfg(test)]
mod tests;
