//! The editor against plan §5 and §8: what it paints, what the keyboard language reaches and does
//! on the graph, what undo reverses, and the checks every editor carries.

use super::*;
use egui::Key;
use mxm_keys::Job;
use mxm_plugin_test::tree_checks;
use std::sync::Mutex;

/// A host that keeps a record of the gestures and applies every write at once, or, `delayed`,
/// only when told ([`Host::flush`]): a CLAP host changes a parameter only when nice-plug writes its
/// queued event, at the end of a process cycle or a flush.
#[derive(Default)]
pub struct Host {
    pub log: Mutex<Vec<(char, ParamPtr)>>,
    pub states: Mutex<usize>,
    pub delayed: std::sync::atomic::AtomicBool,
    queued: Mutex<Vec<(ParamPtr, f32)>>,
}

impl Host {
    pub fn delay(&self, delayed: bool) {
        self.delayed
            .store(delayed, std::sync::atomic::Ordering::Relaxed);
    }

    /// The host takes the queued writes.
    pub fn flush(&self) {
        for (param, normalized) in self.queued.lock().unwrap().drain(..) {
            // SAFETY: the test owns the parameter for the whole call.
            unsafe { param._internal_set_normalized_value(normalized) };
        }
    }
}

// SAFETY: the pointers are only compared, never dereferenced, outside the calls that hand them.
unsafe impl Send for Host {}
unsafe impl Sync for Host {}

impl nice_plug::context::gui::GuiContextInner for Host {
    fn plugin_api(&self) -> nice_plug::context::PluginApi {
        nice_plug::context::PluginApi::Clap
    }
    fn request_restart(&self) {}
    unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
        self.log.lock().unwrap().push(('b', param));
    }
    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
        self.log.lock().unwrap().push(('s', param));
        if self.delayed.load(std::sync::atomic::Ordering::Relaxed) {
            self.queued.lock().unwrap().push((param, normalized));
            return;
        }
        // SAFETY: the test owns the parameter for the whole call.
        unsafe { param._internal_set_normalized_value(normalized) };
    }
    unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
        self.log.lock().unwrap().push(('e', param));
    }
    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }
    fn set_state(&self, _state: PluginState) {
        *self.states.lock().unwrap() += 1;
    }
}

pub struct Rig {
    pub app: EqApp,
    pub host: Arc<Host>,
    pub ctx: egui::Context,
    pub telemetry: Arc<Telemetry>,
    pub audition: Arc<Audition>,
    /// egui's clock, a 60th of a second a frame, so animations and ballistics move.
    pub time: f64,
}

impl Rig {
    pub fn new() -> Rig {
        let params = Arc::new(EqParams::default());
        let telemetry = Arc::new(Telemetry::default());
        telemetry.publish_sample_rate(48_000.0);
        let audition = Arc::new(Audition::default());
        let mut app = EqApp::new(Handles {
            params,
            telemetry: telemetry.clone(),
            audition: audition.clone(),
            history: history::Shared::default(),
            analyser: None,
        });
        let host = Arc::new(Host::default());
        app.attach(GuiContext::new(host.clone()));
        let ctx = tree_checks::context(&|_| {});
        Rig {
            app,
            host,
            ctx,
            telemetry,
            audition,
            time: 0.0,
        }
    }

    /// One frame at `size` with `events`.
    pub fn frame(&mut self, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let modifiers = events
            .iter()
            .rev()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut events = events;
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            focused: true,
            time: Some(self.time),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| app.panel(ui));
        output.textures_delta.clear();
        output
    }

    pub fn opening() -> egui::Vec2 {
        egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32)
    }

    fn press(&mut self, events: Vec<egui::Event>) {
        self.frame(Rig::opening(), events);
    }

    /// Jobs and arrows pressed and let go, one after another, as somebody typing them slowly.
    fn type_keys(&mut self, keys: &[Key]) {
        let events = keys
            .iter()
            .flat_map(|&k| [key(k, true, NONE), key(k, false, NONE)])
            .collect();
        self.press(events);
        self.press(Vec::new());
    }

    /// A job held while arrows are pressed, then let go: one gesture.
    fn hold(&mut self, held: &[Job], arrows: &[Key]) {
        let mut events: Vec<egui::Event> = held
            .iter()
            .map(|&job| key(job_key(job), true, NONE))
            .collect();
        for &arrow in arrows {
            events.push(key(arrow, true, NONE));
            events.push(key(arrow, false, NONE));
        }
        for &job in held.iter().rev() {
            events.push(key(job_key(job), false, NONE));
        }
        self.press(events);
        self.press(Vec::new());
    }

    /// VIEW + ↑ up the column until the cursor is on the Band card, the top one: one more would
    /// take it on into the app bar.
    fn up_to_the_band_card(&mut self) {
        for _ in 0..3 {
            if self.app.nav.card() == Some(BAND_CARD) {
                break;
            }
            self.view(Key::ArrowUp);
        }
        assert_eq!(self.app.nav.card(), Some(BAND_CARD));
        assert_eq!(self.app.nav.bar(), None);
    }

    /// VIEW + an arrow: to the next card that way.
    fn view(&mut self, arrow: Key) {
        self.press(vec![
            key(job_key(VIEW), true, NONE),
            key(arrow, true, NONE),
            key(arrow, false, NONE),
            key(job_key(VIEW), false, NONE),
        ]);
        self.press(Vec::new());
    }

    fn band(&self, slot: usize) -> mxm_fx_eq_n_dsp::Band {
        self.app.params.bands[slot].band()
    }

    fn depth(&self) -> usize {
        history::lock(&self.app.history).depth()
    }

    /// Every parameter's gestures are balanced: as many ends as begins, each set inside one.
    fn gestures_balanced(&self) -> bool {
        let log = self.host.log.lock().unwrap();
        let mut open: Vec<ParamPtr> = Vec::new();
        for (what, param) in log.iter() {
            match what {
                'b' => open.push(*param),
                'e' => {
                    let Some(at) = open.iter().position(|other| other == param) else {
                        return false;
                    };
                    open.remove(at);
                }
                _ => {
                    if !open.contains(param) {
                        return false;
                    }
                }
            }
        }
        open.is_empty()
    }
}

fn key(key: Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

const NONE: egui::Modifiers = egui::Modifiers::NONE;
const MOVE: Job = Job::Verb(mxm_keys::Verb::Move);
const EXTENT: Job = Job::Verb(mxm_keys::Verb::Extent);
const DUPLICATE: Job = Job::Verb(mxm_keys::Verb::Duplicate);
const COARSE: Job = Job::Step(mxm_keys::Step::Coarse);
const MICRO: Job = Job::Step(mxm_keys::Step::Micro);
const ADD: Job = Job::Action(mxm_keys::Action::Add);
const DELETE: Job = Job::Action(mxm_keys::Action::Delete);
const OUT: Job = Job::Out;
const BACK: Job = Job::Back;
const VIEW: Job = Job::View;

/// The key the default keymap gives a job: tests press jobs, so a remap changes no test here.
fn job_key(job: Job) -> Key {
    mxm_ui::navigation::default_key(job)
        .unwrap_or_else(|| panic!("the default keymap binds {}", job.name()))
}

/// Renders the editor in its states, light and dark, into `target/layout-tree/mxm-fx-eq-n/<tag>`
/// for review (`MXM_PICTURES` names the tag).
#[test]
fn pictures() {
    let tag = std::env::var("MXM_PICTURES").unwrap_or_else(|_| "after".to_owned());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/layout-tree/mxm-fx-eq-n")
        .join(tag);
    for (sub, bands) in [("empty", 0usize), ("bands", 4)] {
        let mut rig = Rig::new();
        rig.frame(Rig::opening(), Vec::new());
        for (index, (freq, shape)) in [
            (80.0, Shape::LowCut),
            (250.0, Shape::Bell),
            (2_500.0, Shape::Bell),
            (9_000.0, Shape::HighShelf),
        ]
        .into_iter()
        .take(bands)
        .enumerate()
        {
            let slot = rig.app.insert_at(freq).unwrap();
            rig.app.set_shape(slot, shape);
            if shape == Shape::Bell {
                let gain = band_index(slot, Field::Gain);
                let value = if index == 1 { 6.0 } else { -4.5 };
                rig.app.instant(&[(gain, rig.app.normalised(gain, value))]);
            }
        }
        let mut app = rig.app;
        tree_checks::pictures(&|_| {}, Rig::opening(), &dir.join(sub), &mut |ui| {
            app.panel(ui)
        });
    }
}

#[test]
fn on_the_graph_the_language_makes_moves_widens_and_deletes_bands() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    assert!(rig.app.cursor_on_graph(), "the cursor lands on the graph");
    assert!(
        (0..SLOTS).all(|slot| rig.band(slot).shape.is_none()),
        "Init is empty"
    );

    // ADD: a 0 dB bell at the keyboard's place, in slot 1.
    rig.type_keys(&[job_key(ADD)]);
    assert_eq!(rig.app.selected, Some(0));
    let band = rig.band(0);
    assert_eq!((band.shape, band.gain_db), (Some(Shape::Bell), 0.0));
    assert!((band.freq - 1_000.0).abs() < 0.5);

    // MOVE + → a semitone, as one undo step; MOVE + COARSE + ↑ 3 dB.
    let depth = rig.depth();
    rig.hold(&[MOVE], &[Key::ArrowRight]);
    assert!((rig.band(0).freq / 1_000.0 - 2f32.powf(1.0 / 12.0)).abs() < 1e-3);
    assert_eq!(rig.depth(), depth + 1, "one gesture, one undo step");
    rig.hold(&[MOVE, COARSE], &[Key::ArrowUp]);
    assert!(
        (rig.band(0).gain_db - 3.0).abs() < 1e-4,
        "{}",
        rig.band(0).gain_db
    );

    // EXTENT + → wider: a lower Q, about the same centre.
    let before = rig.band(0);
    rig.hold(&[EXTENT], &[Key::ArrowRight]);
    let after = rig.band(0);
    assert!(after.q < before.q);
    assert_eq!(after.freq, before.freq);

    // MICRO + → takes the end edge; MOVE then moves it alone, the start edge where it was.
    let start = after.freq / graph::edge_ratio(after.q);
    rig.hold(&[MICRO], &[Key::ArrowRight]);
    assert_eq!(rig.app.edge, Some(Edge::End));
    rig.hold(&[MOVE], &[Key::ArrowRight]);
    let moved = rig.band(0);
    assert!(moved.q < after.q, "outward widens");
    assert!(
        (moved.freq / graph::edge_ratio(moved.q) / start - 1.0).abs() < 1e-3,
        "the start edge stayed"
    );
    // BACK lets go of the edge, then of the band.
    rig.type_keys(&[job_key(BACK)]);
    assert_eq!(rig.app.edge, None);
    assert_eq!(rig.app.selected, Some(0));

    // ← → walk the bands by frequency.
    rig.type_keys(&[job_key(ADD)]);
    assert_eq!(rig.app.selected, Some(1));
    rig.app.select(1);
    let freq = rig.app.focus_freq;
    rig.hold(&[MOVE, COARSE], &[Key::ArrowRight]);
    assert!((rig.band(1).freq / freq - 2.0).abs() < 1e-3, "an octave up");
    rig.type_keys(&[Key::ArrowLeft]);
    assert_eq!(rig.app.selected, Some(0), "the band below");
    rig.type_keys(&[Key::ArrowRight]);
    assert_eq!(rig.app.selected, Some(1), "and back up");

    // DELETE removes the band.
    rig.type_keys(&[job_key(DELETE)]);
    assert_eq!(rig.band(1).shape, None);
    assert!(rig.gestures_balanced());
}

#[test]
fn back_cancels_a_gesture_and_leaves_no_undo_step() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    let depth = rig.depth();
    // MOVE held over several frames, then BACK before it is let go.
    rig.press(vec![
        key(job_key(MOVE), true, NONE),
        key(Key::ArrowRight, true, NONE),
    ]);
    rig.press(vec![
        key(Key::ArrowRight, false, NONE),
        key(Key::ArrowRight, true, NONE),
    ]);
    assert!(rig.band(0).freq > 1_050.0);
    rig.press(vec![
        key(job_key(BACK), true, NONE),
        key(job_key(BACK), false, NONE),
    ]);
    rig.press(vec![key(job_key(MOVE), false, NONE)]);
    rig.press(Vec::new());
    assert!(
        (rig.band(0).freq - 1_000.0).abs() < 0.5,
        "put back: {}",
        rig.band(0).freq
    );
    assert_eq!(rig.depth(), depth, "nothing to undo");
    assert!(rig.gestures_balanced());
}

#[test]
fn duplicate_carries_a_copy_and_back_takes_it_away() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.hold(&[MOVE, COARSE], &[Key::ArrowUp]);

    // DUPLICATE + → a semitone, kept with OUT: a second band, the first unchanged.
    rig.press(vec![
        key(job_key(DUPLICATE), true, NONE),
        key(Key::ArrowRight, true, NONE),
        key(Key::ArrowRight, false, NONE),
    ]);
    rig.press(vec![
        key(job_key(DUPLICATE), false, NONE),
        key(job_key(OUT), true, NONE),
        key(job_key(OUT), false, NONE),
    ]);
    rig.press(Vec::new());
    let (original, copy) = (rig.band(0), rig.band(1));
    assert_eq!(rig.app.selected, Some(1), "the copy is selected");
    assert_eq!(copy.shape, Some(Shape::Bell));
    assert_eq!(copy.gain_db, original.gain_db);
    assert!((copy.freq / original.freq - 2f32.powf(1.0 / 12.0)).abs() < 1e-3);
    assert!((original.freq - 1_000.0).abs() < 0.5);

    // Again, cancelled: no third band, and the selection is back on the band copied.
    rig.press(vec![
        key(job_key(DUPLICATE), true, NONE),
        key(Key::ArrowRight, true, NONE),
        key(Key::ArrowRight, false, NONE),
    ]);
    assert_eq!(
        rig.band(2).shape,
        Some(Shape::Bell),
        "the copy is made at once"
    );
    rig.press(vec![
        key(job_key(BACK), true, NONE),
        key(job_key(BACK), false, NONE),
    ]);
    rig.press(vec![key(job_key(DUPLICATE), false, NONE)]);
    rig.press(Vec::new());
    assert_eq!(rig.band(2).shape, None, "BACK took it away");
    assert_eq!(rig.app.selected, Some(1));
    assert!(rig.gestures_balanced());
}

#[test]
fn view_goes_between_the_graph_and_the_column_and_the_column_is_the_cursors() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.view(Key::ArrowRight);
    assert!(!rig.app.cursor_on_graph());
    assert!(
        [BAND_CARD, OUTPUT_CARD, ANALYSER_CARD].contains(&rig.app.nav.card().unwrap()),
        "a card of the column, to the graph's right"
    );
    // VIEW + ↑ walks up the column to the Band card, by the cards' painted places.
    rig.up_to_the_band_card();
    rig.view(Key::ArrowLeft);
    assert!(rig.app.cursor_on_graph(), "and back");
}

/// The shared check, in a bell's state and a cut's: the cursor lands, edits Output as one balanced
/// gesture, selects cards with VIEW, and reaches exactly every control the column draws.
#[test]
fn the_cursor_reaches_and_operates_every_control() {
    for (shape, extra) in [
        (Shape::Bell, &["gain_1", "q_1"][..]),
        (Shape::LowCut, &["q_1", "slope_1"][..]),
    ] {
        let params = Arc::new(EqParams::default());
        let recorder = Arc::new(mxm_plugin_test::keyboard_checks::Recorder::default());
        let mut app = EqApp::new(Handles {
            params,
            telemetry: Arc::new(Telemetry::default()),
            audition: Arc::new(Audition::default()),
            history: history::Shared::default(),
            analyser: None,
        });
        // The band is written directly: the check's host records and applies nothing.
        let band = &app.params.bands[0];
        // SAFETY: the test owns the parameters.
        unsafe {
            let _ = band
                .kind
                .as_ptr()
                ._internal_set_normalized_value(band.kind.preview_normalized(type_of(Some(shape))));
        }
        app.selected = Some(0);
        app.attach(GuiContext::new(recorder.clone()));
        let mut ids = vec![
            GRAPH_KEY,
            "type_1",
            "bypass_1",
            "freq_1",
            SOLO_KEY,
            "delete",
            "output",
            "gainscale",
            "analyser",
            "show",
            "resolution",
            "speed",
            "tilt",
        ];
        ids.extend(extra);
        mxm_plugin_test::keyboard_checks::the_cursor_reaches_and_operates_from(
            Rig::opening(),
            &[],
            mxm_plugin_test::keyboard_checks::Coverage::Exactly(&ids),
            "output",
            &|_| {},
            &recorder,
            &mut |ui| app.panel(ui),
        );
    }
}

#[test]
fn undo_and_redo_reverse_whole_actions_in_order() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.hold(&[MOVE, COARSE], &[Key::ArrowUp]);
    rig.hold(&[MOVE, COARSE], &[Key::ArrowRight]);
    let edited = rig.band(0);
    assert!((edited.gain_db - 3.0).abs() < 1e-4);
    let ctrl = egui::Modifiers::COMMAND;
    let chord = |which: Key, modifiers: egui::Modifiers| {
        vec![key(which, true, modifiers), key(which, false, modifiers)]
    };
    rig.press(chord(Key::Z, ctrl));
    assert!(
        (rig.band(0).freq - 1_000.0).abs() < 0.5,
        "the octave is taken back"
    );
    assert!((rig.band(0).gain_db - 3.0).abs() < 1e-4, "and only it");
    rig.press(chord(Key::Z, ctrl));
    assert_eq!(rig.band(0).gain_db, 0.0);
    rig.press(chord(Key::Z, ctrl));
    assert_eq!(
        rig.band(0).shape,
        None,
        "the insert is taken back: the band is gone"
    );
    rig.press(chord(Key::Z, ctrl | egui::Modifiers::SHIFT));
    assert_eq!(rig.band(0).shape, Some(Shape::Bell));
    rig.press(chord(Key::Y, ctrl));
    rig.press(chord(Key::Y, ctrl));
    let redone = rig.band(0);
    assert_eq!((redone.freq, redone.gain_db), (edited.freq, edited.gain_db));
    assert!(rig.gestures_balanced());
}

#[test]
fn a_restore_or_a_preset_load_starts_an_epoch() {
    use nice_plug::params::persist::PersistentField;
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    assert!(history::lock(&rig.app.history).can_undo());
    // A host restores the project: the persisted view field is set.
    PersistentField::set(&rig.app.params.view, ViewSettings::default());
    rig.press(Vec::new());
    assert!(!history::lock(&rig.app.history).can_undo());

    rig.app.insert_at(2_000.0);
    // Whatever the preset bar writes replaces the patch.
    history::lock(&rig.app.history).start_replacing();
    let index = band_index(2, Field::Type);
    rig.app.instant(&[(index, rig.app.normalised(index, 3.0))]);
    assert!(history::lock(&rig.app.history).finish_replacing());
    assert_eq!(rig.depth(), 0);
}

/// The history is the instance's, and so is the restore count it last saw: a restore while the
/// editor is closed starts an epoch when it opens again.
#[test]
fn a_restore_while_the_editor_is_closed_starts_an_epoch_on_reopening() {
    use nice_plug::params::persist::PersistentField;
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    assert!(history::lock(&rig.app.history).can_undo());
    rig.app.detach();

    PersistentField::set(&rig.app.params.view, ViewSettings::default());

    let mut reopened = EqApp::new(Handles {
        params: rig.app.params.clone(),
        telemetry: rig.telemetry.clone(),
        audition: rig.audition.clone(),
        history: rig.app.history.clone(),
        analyser: None,
    });
    reopened.attach(GuiContext::new(rig.host.clone()));
    rig.app = reopened;
    rig.press(Vec::new());
    assert!(
        !history::lock(&rig.app.history).can_undo(),
        "nothing from before the restore"
    );
}

/// A drag that starts on the line makes the band and moves it as one action: one undo takes it
/// away, and BACK during the drag leaves nothing to undo.
#[test]
fn a_drag_from_the_line_makes_and_moves_the_band_as_one_action() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let from = egui::pos2(axes.x(500.0), axes.y(0.0));
    let to = egui::pos2(axes.x(2_000.0), axes.y(6.0));
    let drag = |rig: &mut Rig, release: bool| {
        rig.press(pointer(from, Some(true)));
        for step in 1..=8 {
            rig.press(pointer(from + (to - from) * (step as f32 / 8.0), None));
        }
        if !release {
            rig.press(vec![
                key(job_key(BACK), true, NONE),
                key(job_key(BACK), false, NONE),
            ]);
        }
        rig.press(pointer(to, Some(false)));
        rig.press(Vec::new());
    };

    drag(&mut rig, true);
    let band = rig.band(0);
    assert_eq!(band.shape, Some(Shape::Bell));
    assert!((band.freq / 2_000.0 - 1.0).abs() < 0.05, "{}", band.freq);
    assert_eq!(rig.depth(), 1, "made and moved: one action");
    rig.app.undo();
    rig.press(Vec::new());
    assert_eq!(rig.band(0).shape, None, "one undo takes the band away");

    drag(&mut rig, false);
    assert_eq!(rig.band(0).shape, None, "BACK took it away");
    assert!(!history::lock(&rig.app.history).can_undo());
    assert!(rig.gestures_balanced());
}

/// COARSE + ← → leaves the bands for the line, an octave a press; ← → alone visit the bands.
#[test]
fn coarse_walks_the_line_past_the_bands_and_the_arrows_visit_them() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.app.insert_at(1_200.0);
    rig.app.select(0);
    rig.press(Vec::new());
    rig.hold(&[COARSE], &[Key::ArrowRight]);
    assert_eq!(rig.app.selected, None);
    assert!(
        (rig.app.focus_freq / 2_000.0 - 1.0).abs() < 1e-3,
        "{}",
        rig.app.focus_freq
    );

    rig.app.select(0);
    rig.type_keys(&[Key::ArrowRight]);
    assert_eq!(rig.app.selected, Some(1), "the 1.2 kHz band");
}

/// MICRO + ← → take the start and end edges; MICRO + ↑ ↓ take none.
#[test]
fn only_micro_left_and_right_take_an_edge() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    for (arrow, edge) in [
        (Key::ArrowUp, None),
        (Key::ArrowDown, None),
        (Key::ArrowLeft, Some(Edge::Start)),
        (Key::ArrowRight, Some(Edge::End)),
    ] {
        rig.app.edge = None;
        rig.hold(&[MICRO], &[arrow]);
        assert_eq!(rig.app.edge, edge, "{arrow:?}");
    }
}

/// A band automation turns Off during a held gesture takes the gesture with it: the host's
/// gestures balance at once, and BACK then writes nothing.
#[test]
fn a_band_turned_off_outside_the_editor_ends_its_gesture() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    open_a_move(&mut rig);
    let moved = rig.band(0).freq;
    let kind = band_index(0, Field::Type);
    // SAFETY: the test owns the parameter for the whole call.
    unsafe { rig.app.pointers[kind]._internal_set_normalized_value(0.0) };
    rig.press(Vec::new());
    assert!(rig.app.open.is_empty());
    assert!(rig.gestures_balanced());
    assert_eq!(rig.app.selected, None);
    back_and_let_go(&mut rig);
    assert_eq!(rig.band(0).shape, None);
    assert_eq!(rig.band(0).freq, moved, "BACK wrote nothing");
    assert!(rig.gestures_balanced());
}

/// A rate change of any size rebuilds the spectrum, so its blocks are analysed.
#[test]
fn any_change_of_rate_rebuilds_the_spectrum() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.telemetry.publish_sample_rate(48_000.25);
    rig.press(Vec::new());
    assert_eq!(rig.app.spectrum.sample_rate(), 48_000.25);
}

/// nice-plug keeps the editor between windows: a reopened one paints its meter from silence.
#[test]
fn a_reopened_window_paints_its_meter_from_silence() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.meters.update(rig.time, [1.0, 1.0]);
    assert_eq!(rig.app.meters.output, [1.0, 1.0]);
    rig.app.detach();
    rig.app.attach(GuiContext::new(rig.host.clone()));
    rig.press(Vec::new());
    assert_eq!(rig.app.meters.output, [0.0, 0.0]);
}

/// Until the host applies a write the parameter reads as before, so the editor chains on what it
/// sent: an ADDed band stays selected, MOVE moves it from where it was made, DUPLICATE's first
/// arrow carries the copy, two ADDs take two slots, and a drag from the line sets gain at once.
#[test]
fn the_editor_chains_on_what_it_sent_before_the_host_applies_it() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.host.delay(true);
    rig.hold(&[COARSE], &[Key::ArrowRight]);
    rig.type_keys(&[job_key(ADD)]);
    assert_eq!(rig.band(0).shape, None, "the host has not applied it");
    assert_eq!(rig.app.selected, Some(0), "still selected");
    assert!((rig.app.band_now(0).freq - 2_000.0).abs() < 0.5);

    rig.hold(&[MOVE], &[Key::ArrowRight]);
    let moved = rig.app.band_now(0).freq;
    assert!(
        (moved / 2_000.0 - 2f32.powf(1.0 / 12.0)).abs() < 1e-3,
        "{moved}"
    );

    rig.press(vec![
        key(job_key(DUPLICATE), true, NONE),
        key(Key::ArrowRight, true, NONE),
        key(Key::ArrowRight, false, NONE),
    ]);
    rig.press(vec![
        key(job_key(DUPLICATE), false, NONE),
        key(job_key(OUT), true, NONE),
        key(job_key(OUT), false, NONE),
    ]);
    rig.press(Vec::new());
    let copy = rig.app.band_now(1);
    assert_eq!(copy.shape, Some(Shape::Bell));
    assert!(
        (copy.freq / moved - 2f32.powf(1.0 / 12.0)).abs() < 1e-3,
        "{}",
        copy.freq
    );
    assert!(
        (rig.app.band_now(0).freq - moved).abs() < 0.01,
        "the original stays"
    );

    assert_eq!(rig.app.insert_at(300.0), Some(2));
    assert_eq!(
        rig.app.insert_at(400.0),
        Some(3),
        "a slot not yet applied is taken"
    );

    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let from = egui::pos2(axes.x(150.0), axes.y(0.0));
    let to = egui::pos2(axes.x(150.0), axes.y(6.0));
    rig.press(pointer(from, Some(true)));
    for step in 1..=4 {
        rig.press(pointer(from + (to - from) * (step as f32 / 4.0), None));
    }
    let made = rig.app.band_now(4);
    assert_eq!(made.shape, Some(Shape::Bell));
    assert!(
        made.gain_db > 3.0,
        "gain from the drag's first frames: {}",
        made.gain_db
    );
    rig.press(pointer(to, Some(false)));

    rig.host.flush();
    rig.press(Vec::new());
    assert_eq!(rig.band(1).shape, Some(Shape::Bell));
    assert_eq!(rig.band(3).shape, Some(Shape::Bell));
    assert!(rig.gestures_balanced());
}

/// A shared control's gesture is the history's too: a restore, or its band going Off, ends it at
/// the host, and what the still-held control sends after is dropped.
#[test]
fn a_column_control_s_gesture_ends_with_a_restore_or_its_band() {
    use nice_plug::params::persist::PersistentField;
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    let gui = rig.app.gui.clone().unwrap();
    let setter = gui.param_setter();
    let params = rig.app.params.clone();
    let output = params.erased()[OUTPUT_INDEX];

    // The Output control mid-drag, as the shared binding writes it; then the host restores.
    output.begin(&setter);
    output.set(&setter, 0.3);
    PersistentField::set(&rig.app.params.view, ViewSettings::default());
    rig.press(Vec::new());
    assert!(rig.gestures_balanced(), "ended at the host");
    let restored = output.normalised();
    output.set(&setter, 0.9);
    output.end(&setter);
    assert_eq!(
        output.normalised(),
        restored,
        "the held control's write is dropped"
    );
    assert!(rig.gestures_balanced());

    // The band's frequency control mid-drag, and the band turned Off outside the editor.
    rig.app.select(0);
    let freq = params.erased()[band_index(0, Field::Freq)];
    freq.begin(&setter);
    freq.set(&setter, 0.7);
    // SAFETY: the test owns the parameter for the whole call.
    unsafe { rig.app.pointers[band_index(0, Field::Type)]._internal_set_normalized_value(0.0) };
    rig.press(Vec::new());
    assert!(rig.gestures_balanced());
    freq.end(&setter);
    assert!(rig.gestures_balanced());

    // A new gesture on it is heard again.
    output.begin(&setter);
    output.set(&setter, 0.6);
    output.end(&setter);
    assert_eq!(output.normalised(), 0.6);
    assert!(rig.gestures_balanced());
}

/// EXTENT is ← → on the graph: ↑ ↓ change nothing.
#[test]
fn vertical_extent_changes_nothing() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    let before = rig.band(0);
    let depth = rig.depth();
    rig.hold(&[EXTENT], &[Key::ArrowUp]);
    rig.hold(&[EXTENT], &[Key::ArrowDown]);
    assert_eq!(rig.band(0), before);
    assert_eq!(rig.depth(), depth);
}

/// BACK puts back only its own gesture, though the host has not applied the one before it.
#[test]
fn back_cancels_only_its_own_gesture_before_the_host_applies_the_last() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.host.delay(true);
    rig.hold(&[MOVE, COARSE], &[Key::ArrowRight]);
    let kept = rig.app.band_now(0).freq;
    assert!((kept - 2_000.0).abs() < 1.0, "{kept}");
    open_a_move(&mut rig);
    back_and_let_go(&mut rig);
    assert!(
        (rig.app.band_now(0).freq - kept).abs() < 0.01,
        "{}",
        rig.app.band_now(0).freq
    );
    rig.host.flush();
    rig.press(Vec::new());
    assert!(
        (rig.band(0).freq - kept).abs() < 0.01,
        "{}",
        rig.band(0).freq
    );
}

/// A write applied and put back by automation between two frames reads as pending only up to
/// the bound, then as what the host has.
#[test]
fn a_write_put_back_between_frames_is_pending_only_up_to_the_bound() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.host.delay(true);
    rig.type_keys(&[job_key(ADD)]);
    rig.host.flush();
    // SAFETY: the test owns the parameter for the whole call.
    unsafe { rig.app.pointers[band_index(0, Field::Type)]._internal_set_normalized_value(0.0) };
    for _ in 0..=history::PENDING_FRAMES {
        rig.press(Vec::new());
    }
    assert_eq!(rig.app.band_now(0).shape, None);
    assert_eq!(rig.app.selected, None);
}

/// A shared control held as the window closes is ended through the recorder, kept.
#[test]
fn closing_the_editor_ends_a_column_control_s_gesture() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let depth = rig.depth();
    let gui = rig.app.gui.clone().unwrap();
    let setter = gui.param_setter();
    let params = rig.app.params.clone();
    let output = params.erased()[OUTPUT_INDEX];
    output.begin(&setter);
    output.set(&setter, 0.3);
    rig.app.detach();
    assert!(rig.gestures_balanced());
    assert_eq!(rig.depth(), depth + 1, "kept, as one action");
}

/// At the minimum window the surface scrolls sideways and the column down: the cursor's target is
/// brought into view as it moves there.
#[test]
fn the_cursor_s_target_is_scrolled_into_view() {
    let mut rig = Rig::new();
    let size = egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32);
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    let frame = |rig: &mut Rig, events: Vec<egui::Event>| {
        rig.frame(size, events);
    };
    for _ in 0..3 {
        frame(&mut rig, Vec::new());
    }
    assert!(rig.app.cursor_on_graph());
    let target = |rig: &Rig| {
        mxm_ui::navigation::spots(&rig.ctx)
            .into_iter()
            .find(|spot| {
                Some(spot.card) == rig.app.nav.card()
                    && Some(spot.key.as_str()) == rig.app.nav.parameter()
            })
            .map(|spot| spot.rect)
    };
    frame(
        &mut rig,
        vec![
            key(job_key(VIEW), true, NONE),
            key(Key::ArrowRight, true, NONE),
            key(Key::ArrowRight, false, NONE),
            key(job_key(VIEW), false, NONE),
        ],
    );
    // egui animates a scroll; half a second covers it.
    for _ in 0..30 {
        frame(&mut rig, Vec::new());
    }
    assert!(!rig.app.cursor_on_graph(), "on a column card");
    let rect = target(&rig).expect("drawn");
    assert!(
        rect.min.x >= window.min.x - 0.5 && rect.max.x <= window.max.x + 0.5,
        "{rect:?} inside {window:?}"
    );
    for _ in 0..12 {
        frame(
            &mut rig,
            vec![
                key(Key::ArrowDown, true, NONE),
                key(Key::ArrowDown, false, NONE),
            ],
        );
        for _ in 0..30 {
            frame(&mut rig, Vec::new());
        }
        if rig.app.nav.bar().is_some() {
            break;
        }
        let rect = target(&rig).expect("drawn");
        assert!(
            rect.max.y <= window.max.y + 0.5 && rect.min.y >= window.min.y - 0.5,
            "{:?}: {rect:?} inside {window:?}",
            rig.app.nav.parameter()
        );
    }
}

/// MOVE held and stepped once on band 1: a keyboard gesture left open.
fn open_a_move(rig: &mut Rig) {
    rig.press(vec![
        key(job_key(MOVE), true, NONE),
        key(Key::ArrowRight, true, NONE),
        key(Key::ArrowRight, false, NONE),
    ]);
    assert!(!rig.app.open.is_empty(), "the gesture is open");
}

/// BACK, then MOVE let go.
fn back_and_let_go(rig: &mut Rig) {
    rig.press(vec![
        key(job_key(BACK), true, NONE),
        key(job_key(BACK), false, NONE),
    ]);
    rig.press(vec![key(job_key(MOVE), false, NONE)]);
    rig.press(Vec::new());
}

/// A host restore during a held gesture ends it, writing nothing: the next arrow starts from the
/// restored value, and BACK puts back only what that new gesture changed.
#[test]
fn a_restore_during_a_gesture_ends_it_and_back_keeps_the_restored_patch() {
    use nice_plug::params::persist::PersistentField;
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    open_a_move(&mut rig);

    // The host restores a project with the band at 500 Hz.
    let freq = band_index(0, Field::Freq);
    let restored = rig.app.normalised(freq, 500.0);
    // SAFETY: the test owns the parameter for the whole call.
    unsafe { rig.app.pointers[freq]._internal_set_normalized_value(restored) };
    PersistentField::set(&rig.app.params.view, ViewSettings::default());
    rig.press(Vec::new());
    assert!(rig.app.open.is_empty(), "the gesture ended");
    assert!(rig.gestures_balanced());

    rig.press(vec![
        key(Key::ArrowRight, true, NONE),
        key(Key::ArrowRight, false, NONE),
    ]);
    assert!(
        (rig.band(0).freq / 500.0 - 2f32.powf(1.0 / 12.0)).abs() < 1e-3,
        "from the restored value: {}",
        rig.band(0).freq
    );
    back_and_let_go(&mut rig);
    assert!(
        (rig.band(0).freq - 500.0).abs() < 0.5,
        "{}",
        rig.band(0).freq
    );
    assert_eq!(rig.depth(), 0);
    assert!(rig.gestures_balanced());
}

/// What the preset bar writes during a held gesture ends it as a restore does: BACK then has
/// nothing from before the load to put back.
#[test]
fn a_preset_load_during_a_gesture_ends_it_and_back_puts_nothing_back() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    open_a_move(&mut rig);
    let moved = rig.band(0).freq;

    // The bar's write, as the bar's calls are wrapped.
    rig.app.start_replacing();
    let kind = band_index(2, Field::Type);
    rig.app.instant(&[(kind, rig.app.normalised(kind, 1.0))]);
    rig.app.finish_replacing(&rig.ctx.clone());
    assert!(rig.app.open.is_empty(), "the gesture ended");

    back_and_let_go(&mut rig);
    assert_eq!(rig.band(0).freq, moved);
    assert_eq!(rig.band(2).shape, Some(Shape::Bell), "the load stands");
    assert_eq!(rig.depth(), 0);
    assert!(rig.gestures_balanced());
}

/// The pointer and the keyboard share no gesture: a press ends a held keyboard gesture, kept as
/// one undo step, before the press reaches anything.
#[test]
fn a_pointer_press_ends_a_keyboard_gesture_and_keeps_it() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    let depth = rig.depth();
    open_a_move(&mut rig);
    let moved = rig.band(0).freq;

    let corner = egui::pos2(2.0, 2.0);
    rig.press(pointer(corner, Some(true)));
    rig.press(pointer(corner, Some(false)));
    assert!(rig.app.open.is_empty());
    assert_eq!(rig.depth(), depth + 1, "kept, as one action");

    back_and_let_go(&mut rig);
    assert_eq!(rig.band(0).freq, moved, "BACK cancels nothing now");
    assert!(rig.gestures_balanced());
}

/// A press ends a wheel gesture at once, kept, as a pause does.
#[test]
fn a_pointer_press_ends_a_wheel_gesture_and_keeps_it() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let point = egui::pos2(axes.x(rig.band(0).freq), axes.y(0.0));
    let depth = rig.depth();
    let q = rig.band(0).q;
    rig.press(vec![
        egui::Event::PointerMoved(point),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: NONE,
        },
    ]);
    assert_ne!(rig.band(0).q, q, "the wheel turned Q");
    assert!(!rig.app.open.is_empty(), "inside the wheel's pause");

    let corner = egui::pos2(2.0, 2.0);
    rig.press(pointer(corner, Some(true)));
    assert!(rig.app.open.is_empty());
    rig.press(pointer(corner, Some(false)));
    assert_eq!(rig.depth(), depth + 1, "kept, as one action");
    assert!(rig.gestures_balanced());
}

/// With every slot in use DUPLICATE has nowhere to put a copy, and moves nothing either.
#[test]
fn duplicate_with_every_slot_in_use_changes_nothing() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    for slot in 0..SLOTS {
        rig.app.insert_at(50.0 * 1.25f32.powi(slot as i32));
    }
    rig.press(Vec::new());
    rig.app.select(0);
    let before = rig.band(0);
    let depth = rig.depth();
    for arrow in [Key::ArrowRight, Key::ArrowUp] {
        rig.press(vec![
            key(job_key(DUPLICATE), true, NONE),
            key(arrow, true, NONE),
            key(arrow, false, NONE),
        ]);
        rig.press(vec![
            key(job_key(DUPLICATE), false, NONE),
            key(job_key(OUT), true, NONE),
            key(job_key(OUT), false, NONE),
        ]);
        rig.press(Vec::new());
    }
    assert_eq!(rig.band(0), before);
    assert_eq!(rig.depth(), depth);
    assert!(rig.gestures_balanced());
}

/// The analyser coming back on starts from silence: nothing drawn before it went off stays.
#[test]
fn the_analyser_coming_back_on_starts_from_silence() {
    use super::spectrum::FLOOR_DB;
    use crate::analyser::{BLOCK_FRAMES, Block};
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let rate = rig.app.spectrum.sample_rate();
    for seq in 0..(rate as usize / 2 / BLOCK_FRAMES) as u64 {
        let mut block = Block {
            seq,
            generation: 0,
            sample_rate: rate,
            channels: 2,
            pre: [[0.0; BLOCK_FRAMES]; 2],
            post: [[0.0; BLOCK_FRAMES]; 2],
        };
        for frame in 0..BLOCK_FRAMES {
            let n = seq as usize * BLOCK_FRAMES + frame;
            let value = (std::f32::consts::TAU * 1_000.0 * n as f32 / rate).sin();
            for channel in 0..2 {
                block.pre[channel][frame] = value;
                block.post[channel][frame] = value;
            }
        }
        rig.app.spectrum.ingest(&block);
    }
    rig.press(Vec::new());
    assert!(
        rig.app
            .spectrum
            .post_db
            .iter()
            .any(|db| *db > FLOOR_DB + 20.0),
        "drawn while on"
    );

    let on = rig.app.params.view.get();
    rig.app.set_view(ViewSettings {
        analyser: false,
        ..on
    });
    rig.press(Vec::new());
    rig.app.set_view(on);
    rig.press(Vec::new());
    assert!(
        rig.app.spectrum.post_db.iter().all(|db| *db <= FLOOR_DB),
        "nothing has played since it came back on"
    );
}

#[test]
fn solo_is_held_never_latched_refused_where_nothing_is_heard_and_ends_on_close() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    // Held: the pointer on the Hold button is the editor's own record of it.
    rig.app.publish_solo(true);
    assert_eq!(rig.audition.slot(), Some(0), "held");
    rig.app.publish_solo(true);
    assert_eq!(rig.audition.slot(), Some(0), "still held");
    rig.app.publish_solo(false);
    assert_eq!(rig.audition.slot(), None, "released");

    // The shell ends a solo while it is held: the same press cannot bring it back.
    rig.app.publish_solo(true);
    rig.audition.end();
    rig.app.publish_solo(true);
    rig.app.publish_solo(true);
    assert_eq!(rig.audition.slot(), None);
    rig.app.publish_solo(false);

    // A bypassed band is not in the sound, so it cannot be soloed.
    rig.app.set_bypass(0, true);
    rig.app.publish_solo(true);
    assert_eq!(rig.audition.slot(), None);
    rig.app.publish_solo(false);
    rig.app.set_bypass(0, false);

    // Closing the editor while it is held ends it.
    rig.app.publish_solo(true);
    assert_eq!(rig.audition.slot(), Some(0));
    rig.app.detach();
    assert_eq!(rig.audition.slot(), None);
    assert!(!rig.telemetry.connected());
}

/// OPEN held with the cursor on Hold holds solo, as the button held does.
#[test]
fn open_held_on_hold_solos_the_band() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.view(Key::ArrowRight);
    rig.up_to_the_band_card();
    for _ in 0..12 {
        match rig.app.nav.parameter() {
            Some(SOLO_KEY) => break,
            // Hold and Delete share the bottom row: ← along it.
            Some("delete") => rig.type_keys(&[Key::ArrowLeft]),
            _ => rig.type_keys(&[Key::ArrowDown]),
        }
    }
    assert_eq!(
        rig.app.nav.parameter(),
        Some(SOLO_KEY),
        "the arrows reach Hold"
    );
    rig.press(vec![key(Key::Enter, true, NONE)]);
    rig.press(Vec::new());
    assert_eq!(rig.audition.slot(), Some(0), "held");
    rig.press(vec![key(Key::Enter, false, NONE)]);
    rig.press(Vec::new());
    assert_eq!(rig.audition.slot(), None, "let go");
}

fn pointer(at: egui::Pos2, pressed: Option<bool>) -> Vec<egui::Event> {
    let mut events = vec![egui::Event::PointerMoved(at)];
    if let Some(pressed) = pressed {
        events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: NONE,
        });
    }
    events
}

#[test]
fn clicking_the_line_adds_a_neutral_bell_in_the_lowest_free_slot_and_a_drag_moves_it() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let on_line = |freq: f32| egui::pos2(axes.x(freq), axes.y(0.0));
    for (freq, slot) in [(200.0, 0), (2_000.0, 1)] {
        rig.press(pointer(on_line(freq), Some(true)));
        rig.press(pointer(on_line(freq), Some(false)));
        assert_eq!(rig.app.selected, Some(slot));
        let band = rig.band(slot);
        assert_eq!((band.shape, band.gain_db), (Some(Shape::Bell), 0.0));
        assert!((band.freq / freq - 1.0).abs() < 0.02, "{}", band.freq);
    }
    rig.app.delete(0);
    rig.press(Vec::new());
    rig.press(pointer(on_line(5_000.0), Some(true)));
    rig.press(pointer(on_line(5_000.0), Some(false)));
    assert_eq!(rig.app.selected, Some(0), "the lowest Off slot, reused");

    // A drag of point 2 sets its frequency and gain, as one action.
    let depth = rig.depth();
    let from = egui::pos2(axes.x(rig.band(1).freq), axes.y(0.0));
    let to = egui::pos2(axes.x(1_000.0), axes.y(6.0));
    rig.press(pointer(from, Some(true)));
    for step in 1..=8 {
        rig.press(pointer(from + (to - from) * (step as f32 / 8.0), None));
    }
    rig.press(pointer(to, Some(false)));
    let band = rig.band(1);
    assert!((band.freq / 1_000.0 - 1.0).abs() < 0.03, "{}", band.freq);
    assert!((band.gain_db - 6.0).abs() < 0.3, "{}", band.gain_db);
    assert_eq!(rig.depth(), depth + 1);
    assert!(rig.gestures_balanced());
}

/// BACK during a drag puts the band back where the drag began.
#[test]
fn back_during_a_drag_cancels_it() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let from = egui::pos2(axes.x(1_000.0), axes.y(0.0));
    let to = egui::pos2(axes.x(3_000.0), axes.y(9.0));
    rig.press(pointer(from, Some(true)));
    for step in 1..=6 {
        rig.press(pointer(from + (to - from) * (step as f32 / 6.0), None));
    }
    assert!(rig.band(0).freq > 2_000.0);
    rig.press(vec![
        key(Key::Escape, true, NONE),
        key(Key::Escape, false, NONE),
    ]);
    rig.press(pointer(to, Some(false)));
    rig.press(Vec::new());
    let band = rig.band(0);
    assert!((band.freq - 1_000.0).abs() < 0.5, "{}", band.freq);
    assert_eq!(band.gain_db, 0.0);
    assert!(rig.gestures_balanced());
}

/// Every point carries its number and the selected one a ring: identity never rests on hue.
#[test]
fn band_identity_is_painted_without_hue() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    for freq in [100.0, 1_000.0, 10_000.0] {
        rig.app.insert_at(freq);
    }
    rig.app.set_bypass(2, true);
    let output = rig.frame(Rig::opening(), Vec::new());
    let texts: Vec<String> = output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    for number in ["1", "2", "3"] {
        assert!(
            texts.iter().any(|text| text == number),
            "point {number} is numbered"
        );
    }
    let rings = output
        .shapes
        .iter()
        .filter(|clipped| match &clipped.shape {
            egui::epaint::Shape::Circle(circle) => {
                circle.fill == egui::Color32::TRANSPARENT && circle.stroke.width > 0.0
            }
            _ => false,
        })
        .count();
    assert!(
        rings >= 2,
        "the selected ring and the bypassed band's hollow point: {rings}"
    );
}

#[test]
fn the_opening_size_is_inside_the_budget_and_holds_the_column_without_scrolling() {
    let budget = mxm_plugin_test::opening_size::BUDGET;
    assert!(REFERENCE.0 as f32 <= budget.x && REFERENCE.1 as f32 <= budget.y);
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    for _ in 0..3 {
        rig.press(Vec::new());
    }
    let (drawn, room) = rig.app.column_extent;
    assert!(drawn <= room, "the column needs {drawn} of {room}");
    assert!(
        room - drawn < 24.0,
        "and the opening is hugged to it: {room} for {drawn}"
    );
}

#[test]
fn the_graph_takes_the_slack_and_the_column_stays_beside_it() {
    let mut sizes = Vec::new();
    for size in [egui::vec2(1_180.0, 880.0), egui::vec2(1_600.0, 1_000.0)] {
        let mut rig = Rig::new();
        for _ in 0..2 {
            rig.frame(size, Vec::new());
        }
        sizes.push(rig.app.graph_rect);
    }
    assert!(sizes[1].width() - sizes[0].width() > 400.0, "{sizes:?}");
    assert!(sizes[1].height() - sizes[0].height() > 100.0);
}

#[test]
fn a_narrow_window_keeps_the_graph_s_floor_and_scrolls_sideways() {
    let minimum = egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32);
    let mut rig = Rig::new();
    rig.frame(minimum, Vec::new());
    rig.app.insert_at(1_000.0);
    rig.frame(minimum, Vec::new());
    assert!(
        rig.app.graph_rect.width() >= GRAPH_FLOOR - 0.5,
        "the graph keeps its floor"
    );
    // The column is beside the graph, beyond the window, until the surface scrolls.
    let output = mxm_ui::navigation::spots(&rig.ctx)
        .into_iter()
        .find(|spot| spot.key == "output")
        .expect("Output is drawn");
    assert!(
        output.rect.left() > rig.app.graph_rect.right(),
        "beside, never under"
    );
}

#[test]
fn the_app_bar_holds_in_the_minimum_window() {
    let mut app = Rig::new().app;
    mxm_plugin_test::opening_size::bar_holds_from_the_minimum(
        egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
        &mut |ui| app.panel(ui),
    );
}

/// Two apps over the same parameters and one host: one builds a card's tree, the other paints it,
/// so a check can hold both at once.
fn pair(select: Option<(Shape, Slope)>) -> (EqApp, EqApp) {
    let params = Arc::new(EqParams::default());
    let telemetry = Arc::new(Telemetry::default());
    telemetry.publish_sample_rate(48_000.0);
    let make = || {
        EqApp::new(Handles {
            params: params.clone(),
            telemetry: telemetry.clone(),
            audition: Arc::new(Audition::default()),
            history: history::Shared::default(),
            analyser: None,
        })
    };
    let (mut build, mut paint) = (make(), make());
    let host = Arc::new(Host::default());
    build.attach(GuiContext::new(host.clone()));
    paint.attach(GuiContext::new(host));
    if let Some((shape, slope)) = select {
        let slot = paint.insert_at(1_000.0).unwrap();
        paint.set_shape(slot, shape);
        paint.set_slope(slot, slope);
        build.selected = Some(slot);
        paint.selected = Some(slot);
    }
    (build, paint)
}

/// `measure` in a context whose fonts have settled: the weighted cuts bind on the third pass.
fn settled(measure: impl Fn(&egui::Ui) -> f32) -> f32 {
    let ctx = tree_checks::context(&|_| {});
    let mut value = 0.0;
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| value = measure(ui));
        output.textures_delta.clear();
    }
    value
}

/// Every card in every state that changes it passes the collection's tree checks, and each fits
/// the column.
#[test]
fn every_card_passes_the_tree_checks_in_every_state_and_fits_the_column() {
    let mut states: Vec<(&str, Option<(Shape, Slope)>)> = vec![("no band", None)];
    for (name, shape, slope) in [
        ("bell", Shape::Bell, Slope::Db12),
        ("low shelf", Shape::LowShelf, Slope::Db12),
        ("high shelf", Shape::HighShelf, Slope::Db12),
        ("low cut 12", Shape::LowCut, Slope::Db12),
        ("high cut 96", Shape::HighCut, Slope::Db96),
        ("low cut 6, no Q", Shape::LowCut, Slope::Db6),
        ("notch", Shape::Notch, Slope::Db12),
        ("band pass", Shape::BandPass, Slope::Db12),
        ("tilt, no Q, no solo", Shape::TiltShelf, Slope::Db12),
    ] {
        states.push((name, Some((shape, slope))));
    }
    // The card's room inside the column: its width less the scroll bar's.
    let room = COLUMN_WIDTH - 12.0;
    for (state, select) in states {
        let (build, mut paint) = pair(select);
        let title = column::band_title(&build);
        let floor = settled(|ui| mxm_ui::tree::card_floor(ui, &title, &column::band_tree(&build)));
        assert!(
            floor <= room,
            "Band ({state}) is {floor} wide, the column holds {room}"
        );
        tree_checks::card(
            &|_| {},
            state,
            &title,
            floor,
            &|_| column::band_tree(&build),
            &mut |ui, leaf, rect| {
                let tokens = *mxm_ui::theme::tokens(ui.ctx());
                paint.paint_leaf(ui, &tokens, *leaf, rect);
            },
        );
    }
    let (build, mut paint) = pair(None);
    type Tree = fn(&EqApp) -> mxm_ui::tree::Node<column::Leaf>;
    let cards: [(&str, Tree); 2] = [
        (column::OUTPUT_TITLE, column::output_tree),
        (column::ANALYSER_TITLE, |_| column::analyser_tree()),
    ];
    for (title, tree) in cards {
        let floor = settled(|ui| mxm_ui::tree::card_floor(ui, title, &tree(&build)));
        assert!(
            floor <= room,
            "{title} is {floor} wide, the column holds {room}"
        );
        tree_checks::card(
            &|_| {},
            "default",
            title,
            floor,
            &|_| tree(&build),
            &mut |ui, leaf, rect| {
                let tokens = *mxm_ui::theme::tokens(ui.ctx());
                paint.paint_leaf(ui, &tokens, *leaf, rect);
            },
        );
    }
}

/// Nothing below the Band card moves when a band is selected, changes shape, or is deselected.
#[test]
fn the_band_card_keeps_its_height_through_every_shape() {
    let mut tops = Vec::new();
    for select in [
        None,
        Some((Shape::Bell, Slope::Db12)),
        Some((Shape::LowCut, Slope::Db48)),
        Some((Shape::TiltShelf, Slope::Db12)),
    ] {
        let (_, app) = pair(select);
        let mut rig = Rig::new();
        rig.app = app;
        for _ in 0..3 {
            rig.press(Vec::new());
        }
        let output = mxm_ui::navigation::spots(&rig.ctx)
            .into_iter()
            .find(|spot| spot.key == "output")
            .map(|spot| spot.rect.top())
            .unwrap();
        tops.push(output);
    }
    assert!(
        tops.windows(2).all(|pair| (pair[0] - pair[1]).abs() < 0.5),
        "{tops:?}"
    );
}
