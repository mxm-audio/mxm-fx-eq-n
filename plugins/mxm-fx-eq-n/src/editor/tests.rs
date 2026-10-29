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
            preview: Arc::new(crate::preview::Preview::default()),
            history: history::Shared::default(),
            analyser: None,
            instance: crate::instances::Instance::detached(),
        });
        let host = Arc::new(Host::default());
        app.attach(GuiContext::new(host.clone()));
        // Find's searches run in the frame that asks, so a test sees what they found.
        app.inline_search = true;
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

    /// Every parameter's gestures are balanced: as many ends as begins, each set inside one, and
    /// no begin while that parameter is already open (nice-plug's own checks reject one).
    fn gestures_balanced(&self) -> bool {
        let log = self.host.log.lock().unwrap();
        let mut open: Vec<ParamPtr> = Vec::new();
        for (what, param) in log.iter() {
            match what {
                'b' if open.contains(param) => return false,
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
const MENU: Job = Job::Action(mxm_keys::Action::Menu);
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
    for (sub, bands) in [("empty", 0usize), ("bands", 4), ("frozen", 1)] {
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
        if sub == "frozen" {
            // Two resonances held over a sloping floor, frozen: the markers and their labels.
            let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
            let freqs = graph::Curves::compute(&rig.app, &axes).freqs;
            let levels: Vec<f32> = freqs
                .iter()
                .map(|&f| {
                    let bell = |centre: f32, height: f32| {
                        let detune = 8.0 * (f / centre - centre / f);
                        height / (1.0 + detune * detune)
                    };
                    let floor = 1.0e-5 * (100.0 / f).min(10.0);
                    10.0 * (floor + bell(320.0, 1.0e-3) + bell(3_100.0, 2.0e-4)).log10()
                })
                .collect();
            rig.app.hold.push(0.0, &levels);
            rig.app.hold_basis = Some(grab::Basis {
                view: rig.app.params.view.get(),
                generation: rig.app.spectrum.generation(),
            });
            rig.app.freeze_asked = true;
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
        (
            Shape::Bell,
            &[
                "gain_1",
                "q_1",
                "range_1",
                "threshold_1",
                "attack_1",
                "release_1",
                "key_1",
                "place_1",
                "split",
                "freeze",
            ][..],
        ),
        (
            Shape::LowCut,
            &["q_1", "slope_1", "place_1", "split", "freeze"][..],
        ),
    ] {
        let params = Arc::new(EqParams::default());
        let recorder = Arc::new(mxm_plugin_test::keyboard_checks::Recorder::default());
        let mut app = EqApp::new(Handles {
            params,
            telemetry: Arc::new(Telemetry::default()),
            audition: Arc::new(Audition::default()),
            preview: Arc::new(crate::preview::Preview::default()),
            history: history::Shared::default(),
            analyser: None,
            instance: crate::instances::Instance::detached(),
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
            "autogain",
            "phase",
            "resolution",
            "analyser",
            "collisions",
            "external",
            "show",
            "resolution",
            "speed",
            "tilt",
            "find",
            "apply all",
            "amount",
            "learn",
            "match apply",
            "preview",
            "match to",
            "save reference",
            "open references",
            "match amount",
            "detail",
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
        preview: Arc::new(crate::preview::Preview::default()),
        history: rig.app.history.clone(),
        analyser: None,
        instance: crate::instances::Instance::detached(),
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

/// A preset load while a shared control holds a parameter it writes: the control's gesture ends at
/// the host before the load's own begin, the load lands, and the control's tail is dropped.
#[test]
fn a_preset_load_never_nests_a_begin_in_a_held_control_s_gesture() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let gui = rig.app.gui.clone().unwrap();
    let setter = gui.param_setter();
    let params = rig.app.params.clone();
    let output = params.param(OUTPUT_INDEX);
    output.begin(&setter);
    output.set(&setter, 0.3);

    rig.app.start_replacing();
    rig.app.instant(&[(OUTPUT_INDEX, 0.7)]);
    rig.app.finish_replacing(&rig.ctx.clone());
    assert!(
        rig.gestures_balanced(),
        "no nested begin, every gesture ended"
    );
    assert_eq!(output.normalised(), 0.7);

    output.set(&setter, 0.9);
    output.end(&setter);
    assert_eq!(
        output.normalised(),
        0.7,
        "the held control's tail is dropped"
    );
    assert!(rig.gestures_balanced());
}

/// A selected target stays in view when the window shrinks or the zoom grows under it.
#[test]
fn a_resize_or_a_zoom_keeps_the_cursor_s_target_in_view() {
    for zoom in [false, true] {
        let mut rig = Rig::new();
        rig.press(Vec::new());
        rig.press(Vec::new());
        rig.view(Key::ArrowRight);
        for _ in 0..3 {
            rig.press(Vec::new());
        }
        assert!(!rig.app.cursor_on_graph(), "on a column card");
        let size = if zoom {
            rig.ctx.set_zoom_factor(2.0);
            Rig::opening()
        } else {
            egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32)
        };
        for _ in 0..30 {
            rig.frame(size, Vec::new());
        }
        let window = rig.ctx.content_rect();
        let rect = mxm_ui::navigation::spots(&rig.ctx)
            .into_iter()
            .find(|spot| {
                Some(spot.card) == rig.app.nav.card()
                    && Some(spot.key.as_str()) == rig.app.nav.parameter()
            })
            .map(|spot| spot.rect)
            .expect("drawn");
        assert!(
            rect.min.x >= window.min.x - 0.5 && rect.max.x <= window.max.x + 0.5,
            "zoom {zoom}: {rect:?} inside {window:?}"
        );
    }
}

/// The editor's per-frame reads allocate nothing: a band as last set, for every slot, and a
/// recorded write inside an open gesture.
#[test]
fn the_editor_s_hot_reads_and_recorded_writes_allocate_nothing() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    let gui = rig.app.gui.clone().unwrap();
    let setter = gui.param_setter();
    let params = rig.app.params.clone();
    let freq = params.param(band_index(0, Field::Freq));
    freq.begin(&setter);
    freq.set(&setter, 0.6);
    rig.host.log.lock().unwrap().reserve(64);
    nice_assert_no_alloc::assert_no_alloc(|| {
        for slot in 0..SLOTS {
            let _ = rig.app.band_now(slot);
        }
        for step in 0..8 {
            freq.set(&setter, 0.6 + step as f32 * 0.01);
        }
    });
    freq.end(&setter);
    assert!(rig.gestures_balanced());
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
            key: [[0.0; BLOCK_FRAMES]; 2],
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

/// **Solo is a toggle** (the owner, 2026-10-14: held, the band could not be edited while it
/// played). It stays on with nothing held, and goes when it is pressed again, when the shell ends
/// it, when its band leaves the sound, when another band is selected and when the editor closes.
#[test]
fn solo_latches_on_the_selected_band_until_something_lets_it_go() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.app.toggle_solo(0);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), Some(0), "latched");
    // Edited while it plays, with nothing held.
    rig.app.instant(&[(band_index(0, Field::Gain), 0.7)]);
    rig.press(Vec::new());
    rig.press(Vec::new());
    assert_eq!(rig.audition.slot(), Some(0), "still on");
    rig.app.toggle_solo(0);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), None, "pressed again");

    // The shell ends it: the latch goes with it, and stays gone.
    rig.app.toggle_solo(0);
    rig.app.publish_solo();
    rig.audition.end();
    rig.app.publish_solo();
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), None);
    assert_eq!(rig.app.latched, None);

    // A bypassed band is not in the sound: the latch is let go, and stays let go.
    rig.app.toggle_solo(0);
    rig.app.publish_solo();
    rig.app.set_bypass(0, true);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), None);
    rig.app.set_bypass(0, false);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), None, "un-bypassed, not soloed again");

    // Another band selected lets it go.
    rig.app.toggle_solo(0);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), Some(0));
    rig.app.insert_at(4_000.0);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), None);

    // Closing the editor ends it.
    rig.app.select(0);
    rig.app.toggle_solo(0);
    rig.app.publish_solo();
    assert_eq!(rig.audition.slot(), Some(0));
    rig.app.detach();
    assert_eq!(rig.audition.slot(), None);
    assert!(!rig.telemetry.connected());
}

/// A preset load replaces the band the solo was on, so the latch goes with the patch.
#[test]
fn a_preset_load_lets_a_latched_solo_go() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.app.toggle_solo(0);
    rig.press(Vec::new());
    assert_eq!(rig.audition.slot(), Some(0));
    rig.app.start_replacing();
    rig.app.instant(&[(band_index(0, Field::Freq), 0.3)]);
    rig.app.finish_replacing(&rig.ctx.clone());
    rig.press(Vec::new());
    assert_eq!(rig.audition.slot(), None);
}

/// OPEN with the cursor on Solo presses it as a click does: once on, again off.
#[test]
fn open_on_solo_toggles_it() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.view(Key::ArrowRight);
    rig.up_to_the_band_card();
    for _ in 0..12 {
        match rig.app.nav.parameter() {
            Some(SOLO_KEY) => break,
            // Solo and Delete share the bottom row: ← along it.
            Some("delete") => rig.type_keys(&[Key::ArrowLeft]),
            _ => rig.type_keys(&[Key::ArrowDown]),
        }
    }
    assert_eq!(
        rig.app.nav.parameter(),
        Some(SOLO_KEY),
        "the arrows reach Solo"
    );
    rig.type_keys(&[Key::Enter]);
    for _ in 0..3 {
        rig.press(Vec::new());
    }
    assert_eq!(rig.audition.slot(), Some(0), "on, with nothing held");
    rig.type_keys(&[Key::Enter]);
    rig.press(Vec::new());
    assert_eq!(rig.audition.slot(), None, "off");
}

/// **The gain scale fades while it has nothing to scale**: no band in the sound with a gain. A
/// notch, a cut and a band pass have none, and a bypassed bell is not in the sound.
#[test]
fn the_gain_scale_is_heard_only_with_a_band_that_has_a_gain() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    assert!(!rig.app.gain_heard(), "Init: no band");
    rig.app.insert_at(1_000.0);
    assert!(rig.app.gain_heard(), "a bell");
    rig.app.set_bypass(0, true);
    assert!(!rig.app.gain_heard(), "the bell bypassed");
    rig.app.set_bypass(0, false);
    for shape in [Shape::Notch, Shape::LowCut, Shape::BandPass] {
        rig.app.set_shape(0, shape);
        assert!(!rig.app.gain_heard(), "{shape:?}");
    }
    for shape in [Shape::LowShelf, Shape::TiltShelf] {
        rig.app.set_shape(0, shape);
        assert!(rig.app.gain_heard(), "{shape:?}");
    }
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
    // With the side-chain port the Analyser card's row holds three toggles (plan §7.4).
    for port in [false, true] {
        let mut rig = Rig::new();
        rig.app.telemetry.publish_side_chain_port(port);
        rig.press(Vec::new());
        rig.app.insert_at(1_000.0);
        for _ in 0..3 {
            rig.press(Vec::new());
        }
        let (drawn, room) = rig.app.column_extent;
        assert!(
            drawn <= room,
            "the column needs {drawn} of {room} (port {port})"
        );
        assert!(
            room - drawn < 24.0,
            "and the opening is hugged to it: {room} for {drawn} (port {port})"
        );
    }
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
            preview: Arc::new(crate::preview::Preview::default()),
            history: history::Shared::default(),
            analyser: None,
            instance: crate::instances::Instance::detached(),
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
    let cards: [(&str, Tree); 4] = [
        (column::OUTPUT_TITLE, column::output_tree),
        (column::ANALYSER_TITLE, |_| {
            column::analyser_tree(vec!["Off".to_owned(), "Side chain".to_owned()])
        }),
        (column::RESONANCES_TITLE, |_| column::resonances_tree()),
        (column::MATCH_TITLE, |app| app.match_card_tree()),
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

/// Writes a parameter at once, as a host's automation would.
fn set_plain(param: &FloatParam, plain: f32) {
    // SAFETY: the test owns the parameters.
    unsafe {
        let _ = param
            .as_ptr()
            ._internal_set_normalized_value(param.preview_normalized(plain));
    }
}

/// **A new band is static, and a copy takes its original's dynamics** (plan §7.1): a Range left
/// in a slot by the band that was there is not inherited, and DUPLICATE carries all ten fields.
#[test]
fn a_new_band_is_static_and_a_copy_takes_the_original_s_dynamics() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    set_plain(&rig.app.params.dynamics[0].range, -12.0);
    set_plain_bool(&rig.app.params.keys[0].side_chain, true);
    set_plain_int(&rig.app.params.places[0].placement, 3);
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.press(Vec::new());
    assert_eq!(rig.band(0).shape, Some(Shape::Bell));
    assert_eq!(
        rig.app.params.dynamics[0].dynamics(),
        mxm_fx_eq_n_dsp::Dynamics::STATIC
    );
    assert!(
        !rig.app.params.keys[0].side_chain.value(),
        "a new band hears its own input"
    );
    assert_eq!(
        rig.app.params.places[0].placement.value(),
        0,
        "and is Stereo"
    );
    set_plain_bool(&rig.app.params.keys[0].side_chain, true);
    set_plain_int(&rig.app.params.places[0].placement, 2);

    set_plain(&rig.app.params.dynamics[0].range, -9.0);
    set_plain(&rig.app.params.dynamics[0].attack, 3.0);
    rig.press(Vec::new());
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
    assert_eq!(rig.band(1).shape, Some(Shape::Bell));
    let (original, copy) = (
        rig.app.params.dynamics[0].dynamics(),
        rig.app.params.dynamics[1].dynamics(),
    );
    assert!((copy.range_db + 9.0).abs() < 1e-3 && (copy.attack_ms - 3.0).abs() < 1e-3);
    // Through each parameter's plain value, as the band's fields are copied.
    for (a, b) in [
        (copy.range_db, original.range_db),
        (copy.threshold_db, original.threshold_db),
        (copy.attack_ms, original.attack_ms),
        (copy.release_ms, original.release_ms),
    ] {
        assert!((a - b).abs() < 1e-3, "{copy:?} {original:?}");
    }
    assert!(
        rig.app.params.keys[1].side_chain.value(),
        "a copy of a keyed band is keyed"
    );
    assert_eq!(
        rig.app.params.places[1].placement.value(),
        2,
        "and keeps its placement"
    );
    assert!(rig.gestures_balanced());
}

/// **The Dynamics group is a bell's and a shelf's** (plan §7.1): the Band card holds Range,
/// Threshold, Attack, Release and the moving meter for those, and none of them for the rest.
#[test]
fn the_dynamics_group_belongs_to_the_bells_and_shelves() {
    let params = EqParams::default();
    for shape in crate::params::SHAPES {
        let body = column::band_body(
            &params,
            0,
            shape,
            DEFAULT_SLOPE,
            column::PlacementRow::Offered,
        );
        let found: Vec<column::Leaf> = body.keys().into_iter().copied().collect();
        let group = [
            column::Leaf::Slider(column::Slider::Range),
            column::Leaf::Slider(column::Slider::Threshold),
            column::Leaf::Slider(column::Slider::Attack),
            column::Leaf::Slider(column::Slider::Release),
            column::Leaf::Moving,
            column::Leaf::SideChain,
        ];
        for leaf in group {
            assert_eq!(
                found.contains(&leaf),
                shape.is_dynamic(),
                "{shape:?} {leaf:?}"
            );
        }
    }
}

/// **A moving band is drawn as heard, over the curve as drawn** (plan §7.1): with no movement
/// published there is one curve; with a bell's heard gain 6 dB under its drawn one the curve as
/// heard is the composite at the moved gain, and a cut's movement is never drawn.
#[test]
fn a_moving_band_is_drawn_as_heard_over_the_drawn_curve() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    assert!(graph::Curves::compute(&rig.app, &axes).moving.is_none());

    rig.app.telemetry.publish_gains(0, 0.0, -6.0);
    let curves = graph::Curves::compute(&rig.app, &axes);
    let moving = curves.moving.as_ref().expect("the curve as heard");
    let centre = curves
        .freqs
        .iter()
        .position(|&freq| freq >= 1_000.0)
        .unwrap();
    assert!(curves.composite[centre].abs() < 0.1, "drawn at 0 dB");
    assert!(
        (moving[centre] + 6.0).abs() < 0.2,
        "heard 6 dB down: {}",
        moving[centre]
    );

    // A drawn-gain edit the DSP is still gliding to: the curve is the heard gain, not the target
    // plus the movement.
    set_plain(&rig.app.params.bands[0].gain, 24.0);
    rig.press(Vec::new());
    rig.app.telemetry.publish_gains(0, 0.0, -6.0);
    let curves = graph::Curves::compute(&rig.app, &axes);
    let moving = curves.moving.as_ref().expect("the curve as heard");
    assert!(
        (moving[centre] + 6.0).abs() < 0.2,
        "heard, not drawn: {}",
        moving[centre]
    );

    set_plain_int(&rig.app.params.bands[0].kind, type_of(Some(Shape::LowCut)));
    rig.press(Vec::new());
    assert!(graph::Curves::compute(&rig.app, &axes).moving.is_none());
}

fn set_plain_int(param: &IntParam, plain: i32) {
    // SAFETY: the test owns the parameters.
    unsafe {
        let _ = param
            .as_ptr()
            ._internal_set_normalized_value(param.preview_normalized(plain));
    }
}

/// **The meter and the range line are what is heard, under the gain scale** (plan §7.1): the
/// range line ends at the drawn gain plus Range, bounded, times the scale; the meter is the
/// movement times the scale; at 0 % both are nothing, as the audio is.
#[test]
fn the_meter_and_the_range_line_follow_the_gain_scale() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    set_plain(&rig.app.params.bands[0].gain, 6.0);
    set_plain(&rig.app.params.dynamics[0].range, -24.0);
    rig.press(Vec::new());
    for (scale, reach, moving) in [
        (1.0, -18.0, -8.0),
        (0.5, -9.0, -4.0),
        (0.0, 0.0, 0.0),
        (2.0, -36.0, -16.0),
    ] {
        // The audio thread's scale has glided there too.
        set_plain(&rig.app.params.gain_scale, scale);
        rig.press(Vec::new());
        rig.app.telemetry.publish_gains(0, 6.0, -2.0);
        rig.app.telemetry.publish_heard_scale(scale);
        let at = rig.app.range_reach_db(0).expect("a dynamic bell");
        assert!(
            (at - reach).abs() < 1e-3,
            "{scale}: the line ends at {at}, not {reach}"
        );
        let meter = rig.app.moving_db(0);
        assert!(
            (meter - moving).abs() < 1e-3,
            "{scale}: the meter reads {meter}"
        );
    }
    set_plain(&rig.app.params.dynamics[0].range, 0.0);
    rig.press(Vec::new());
    assert_eq!(rig.app.range_reach_db(0), None, "a static band has no line");
}

/// **The curve as heard is every band as heard** (plan §7.1): while one band moves, a second,
/// static band the DSP is still gliding is drawn where the DSP has it, not at its target.
#[test]
fn while_a_band_moves_every_band_is_drawn_as_heard() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    rig.app.insert_at(8_000.0);
    rig.press(Vec::new());
    set_plain(&rig.app.params.bands[1].gain, 24.0);
    rig.press(Vec::new());
    rig.app.telemetry.publish_gains(0, 0.0, -6.0);
    rig.app.telemetry.publish_gains(1, 0.0, 0.0);
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let curves = graph::Curves::compute(&rig.app, &axes);
    let moving = curves.moving.as_ref().expect("band 1 moves");
    let at = curves
        .freqs
        .iter()
        .position(|&freq| freq >= 8_000.0)
        .unwrap();
    assert!(curves.composite[at] > 20.0, "drawn at its target");
    assert!(
        moving[at].abs() < 1.5,
        "heard where the DSP has it: {}",
        moving[at]
    );
}

/// **The meter and the curve as heard follow the scale the audio thread has glided to**, not the
/// parameter's target: halfway down a glide to 0 %, half the movement is heard.
#[test]
fn the_heard_display_follows_the_glided_gain_scale() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    set_plain(&rig.app.params.gain_scale, 0.0);
    rig.press(Vec::new());
    rig.app.telemetry.publish_gains(0, 0.0, -6.0);
    rig.app.telemetry.publish_heard_scale(0.5);
    assert!(
        (rig.app.moving_db(0) + 3.0).abs() < 1e-4,
        "{}",
        rig.app.moving_db(0)
    );
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let curves = graph::Curves::compute(&rig.app, &axes);
    let centre = curves
        .freqs
        .iter()
        .position(|&freq| freq >= 1_000.0)
        .unwrap();
    let moving = curves.moving.as_ref().expect("the curve as heard");
    assert!((moving[centre] + 3.0).abs() < 0.2, "{}", moving[centre]);
}

/// **Undo and redo of a dynamics edit show its band** (`slot_of`): with another band selected,
/// the band whose Range changed back is the one in the column.
#[test]
fn undoing_a_dynamics_edit_shows_its_band() {
    assert_eq!(slot_of(band_index(3, Field::Gain)), Some(3));
    assert_eq!(slot_of(OUTPUT_INDEX), None);
    assert_eq!(slot_of(GAIN_SCALE_INDEX), None);
    assert_eq!(slot_of(dynamics_index(5, DynamicsField::Release)), Some(5));
    assert_eq!(slot_of(key_index(0)), Some(0));
    assert_eq!(slot_of(key_index(23)), Some(23));
    assert_eq!(slot_of(place_index(0)), Some(0));
    assert_eq!(slot_of(place_index(23)), Some(23));

    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    rig.app.insert_at(4_000.0);
    rig.press(Vec::new());
    // A Range edit as a slider makes it, through the editor's recording context.
    let range = &rig.app.params.dynamics[0].range;
    let gui = rig.app.gui.clone().expect("attached");
    let setter = gui.param_setter();
    setter.begin_set_parameter(range);
    setter.set_parameter(range, -12.0);
    setter.end_set_parameter(range);
    rig.press(Vec::new());
    assert!((rig.app.params.dynamics[0].range.value() + 12.0).abs() < 1e-3);

    rig.app.selected = Some(1);
    rig.app.undo();
    rig.press(Vec::new());
    assert_eq!(rig.app.selected, Some(0), "undo shows the band it changed");
    assert_eq!(rig.app.params.dynamics[0].range.value(), 0.0);
    rig.app.selected = Some(1);
    rig.app.redo();
    rig.press(Vec::new());
    assert_eq!(rig.app.selected, Some(0), "and so does redo");
    assert!((rig.app.params.dynamics[0].range.value() + 12.0).abs() < 1e-3);
}

fn set_plain_bool(param: &BoolParam, plain: bool) {
    // SAFETY: the test owns the parameters.
    unsafe {
        let _ = param
            .as_ptr()
            ._internal_set_normalized_value(param.preview_normalized(plain));
    }
}

/// **The editor counts the switch only on a bell or shelf**, as the DSP does: a band changed to a
/// cut with its switch still on does not listen to the side chain.
#[test]
fn a_switch_kept_from_a_dynamic_shape_means_nothing_to_another() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    set_plain_bool(&rig.app.params.keys[0].side_chain, true);
    rig.press(Vec::new());
    assert!(rig.app.listens_to_key(0));
    set_plain_int(&rig.app.params.bands[0].kind, type_of(Some(Shape::LowCut)));
    rig.press(Vec::new());
    assert!(rig.app.key_now(0), "the switch is kept");
    assert!(!rig.app.listens_to_key(0), "but a cut does not listen");
}

/// **The Band card's placement row** (plan §7.3): the selector and Split in a stereo layout; in
/// mono nothing for a Stereo band, and only the selector, the way back, for a placed one.
#[test]
fn the_placement_row_follows_the_layout() {
    let params = EqParams::default();
    for (row, placement, split) in [
        (column::PlacementRow::Offered, true, true),
        (column::PlacementRow::Restore, true, false),
        (column::PlacementRow::Absent, false, false),
    ] {
        let body = column::band_body(&params, 0, Shape::Bell, DEFAULT_SLOPE, row);
        let found: Vec<column::Leaf> = body.keys().into_iter().copied().collect();
        assert_eq!(
            found.contains(&column::Leaf::Placement),
            placement,
            "{row:?}"
        );
        assert_eq!(found.contains(&column::Leaf::Split), split, "{row:?}");
    }
}

/// **Split makes a pair of one band, as one step** (plan §7.3): Left and Right, or Mid and Side
/// with Alt, every other field copied, the original still selected; undo takes the pair back; a
/// placed band, a full set of slots and a mono layout refuse.
#[test]
fn split_makes_a_pair_as_one_step_and_refuses_what_it_cannot() {
    use mxm_fx_eq_n_dsp::Placement;
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    set_plain(&rig.app.params.bands[0].gain, 6.0);
    rig.press(Vec::new());
    let depth = rig.depth();
    rig.app.split(0, false);
    rig.press(Vec::new());
    assert_eq!(rig.app.placement_now(0), Placement::Left);
    assert_eq!(rig.app.placement_now(1), Placement::Right);
    // Every other field copied, through each parameter's plain value.
    let (original, copy) = (rig.band(0), rig.band(1));
    assert_eq!(
        (copy.shape, copy.bypass, copy.slope, copy.gain_db),
        (
            original.shape,
            original.bypass,
            original.slope,
            original.gain_db
        )
    );
    assert!((copy.freq - original.freq).abs() < 1e-2 && (copy.q - original.q).abs() < 1e-4);
    assert_eq!(rig.app.selected, Some(0));
    assert_eq!(rig.depth(), depth + 1, "one undo step");
    assert_eq!(
        rig.app.split_refused(0),
        Some(SplitRefusal::Placed),
        "a placed band does not split"
    );
    rig.app.undo();
    rig.press(Vec::new());
    assert_eq!(rig.band(1).shape, None);
    assert_eq!(rig.app.placement_now(0), Placement::Stereo);

    rig.app.split(0, true);
    rig.press(Vec::new());
    assert_eq!(
        (rig.app.placement_now(0), rig.app.placement_now(1)),
        (Placement::Mid, Placement::Side)
    );
    rig.app.undo();
    rig.press(Vec::new());
    for slot in 1..SLOTS {
        rig.app.insert_at(100.0 + 300.0 * slot as f32);
    }
    rig.press(Vec::new());
    assert_eq!(rig.app.split_refused(0), Some(SplitRefusal::Full));
    rig.app.telemetry.publish_channels(1);
    assert_eq!(rig.app.split_refused(0), Some(SplitRefusal::Mono));
}

/// **In mono a placed band is not in the sound** (plan §7.3): no solo, no gain to scale, not drawn
/// into the curve.
#[test]
fn in_mono_a_placed_band_is_not_heard() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    set_plain(&rig.app.params.bands[0].gain, 6.0);
    set_plain_int(&rig.app.params.places[0].placement, 1);
    rig.press(Vec::new());
    assert!(rig.app.offers_solo(0) && rig.app.gain_heard());
    rig.app.telemetry.publish_channels(1);
    assert!(!rig.app.placement_heard(0));
    assert!(!rig.app.offers_solo(0), "no solo");
    assert!(!rig.app.gain_heard(), "no gain to scale");
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let curves = graph::Curves::compute(&rig.app, &axes);
    assert!(curves.channels.is_empty());
    assert!(curves.composite.iter().all(|db| db.abs() < 1.0e-6), "flat");
}

/// The gain, in dB, a left-only (or mid-only, `mid` true) 1 kHz tone takes on its own channel
/// through an engine set as the editor's bands are.
fn rendered_self_gain(bands: &[(f32, f32, mxm_fx_eq_n_dsp::Placement)], mid: bool) -> f64 {
    let mut engine = mxm_fx_eq_n_dsp::Engine::new(48_000.0);
    for (slot, (freq, gain_db, placement)) in bands.iter().enumerate() {
        engine.set_band(
            slot,
            mxm_fx_eq_n_dsp::Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq: *freq,
                gain_db: *gain_db,
                q: 1.0,
                slope: DEFAULT_SLOPE,
            },
        );
        engine.set_placement(slot, *placement);
    }
    engine.reset();
    let n = 48_000;
    let tone: Vec<f32> = (0..n)
        .map(|i| (0.25 * (std::f64::consts::TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let mut left = tone.clone();
    let mut right = if mid { tone.clone() } else { vec![0.0; n] };
    engine.process_stereo(&mut left, &mut right);
    let (mut c, mut s) = (0.0f64, 0.0f64);
    for (i, (&l, &r)) in left.iter().zip(&right).enumerate().skip(n / 2) {
        let y = if mid { (l + r) * 0.5 } else { l };
        let phase = std::f64::consts::TAU * 1_000.0 * i as f64 / 48_000.0;
        c += f64::from(y) * phase.cos();
        s += f64::from(y) * phase.sin();
    }
    20.0 * (2.0 * (c * c + s * s).sqrt() / (n / 2) as f64 / 0.25).log10()
}

/// **Each line is its channel's response to itself, as the audio renders it** (plan §7.3), with
/// one family and with both.
#[test]
fn each_channel_line_is_what_its_channel_renders() {
    use mxm_fx_eq_n_dsp::Placement;
    for bands in [
        vec![(1_000.0, 6.0, Placement::Left)],
        vec![
            (1_000.0, 6.0, Placement::Left),
            (1_000.0, 6.0, Placement::Mid),
        ],
        vec![
            (800.0, -4.0, Placement::Stereo),
            (1_200.0, 5.0, Placement::Side),
            (1_000.0, 3.0, Placement::Mid),
        ],
    ] {
        let mut rig = Rig::new();
        rig.press(Vec::new());
        for (slot, (freq, gain, placement)) in bands.iter().enumerate() {
            rig.app.insert_at(*freq);
            rig.press(Vec::new());
            set_plain(&rig.app.params.bands[slot].gain, *gain);
            set_plain(&rig.app.params.bands[slot].freq, *freq);
            set_plain_int(
                &rig.app.params.places[slot].placement,
                crate::params::placement_value(*placement),
            );
        }
        rig.press(Vec::new());
        let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
        let curves = graph::Curves::compute(&rig.app, &axes);
        let at = curves
            .freqs
            .iter()
            .position(|&freq| freq >= 1_000.0)
            .unwrap();
        let line = |letter: &str| {
            curves
                .channels
                .iter()
                .find(|line| line.letter == letter)
                .map(|line| f64::from(line.db[at]))
        };
        let near = |drawn: f64, heard: f64| (drawn - heard).abs() < 0.25;
        if let Some(drawn) = line("L") {
            let heard = rendered_self_gain(&bands, false);
            assert!(
                near(drawn, heard),
                "{bands:?}: L drawn {drawn}, heard {heard}"
            );
        }
        if let Some(drawn) = line("M") {
            let heard = rendered_self_gain(&bands, true);
            assert!(
                near(drawn, heard),
                "{bands:?}: M drawn {drawn}, heard {heard}"
            );
        }
        assert!(!curves.channels.is_empty(), "{bands:?}");
    }
}

/// **A placed band's Solo is drawn disabled in mono**, as it offers none (plan §7.3), and enabled
/// again in stereo.
#[test]
fn a_placed_band_s_solo_is_disabled_in_mono() {
    let solo_disabled = |mono: bool| {
        let mut rig = Rig::new();
        rig.ctx.enable_accesskit();
        rig.press(Vec::new());
        rig.app.insert_at(1_000.0);
        rig.press(Vec::new());
        set_plain_int(&rig.app.params.places[0].placement, 1);
        rig.app.telemetry.publish_channels(if mono { 1 } else { 2 });
        rig.press(Vec::new());
        let output = rig.frame(Rig::opening(), Vec::new());
        let update = output
            .platform_output
            .accesskit_update
            .expect("an accessibility tree");
        let (_, node) = update
            .nodes
            .iter()
            .find(|(_, node)| node.label().is_some_and(|label| label.starts_with("Solo")))
            .expect("the Solo toggle");
        node.is_disabled()
    };
    assert!(solo_disabled(true), "disabled in mono");
    assert!(!solo_disabled(false), "enabled in stereo");
}

/// **The point's reading names its placement**: always in a stereo layout, Stereo included; in
/// mono only a placed band's, with why it is not heard.
#[test]
fn the_point_reading_names_the_placement() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    assert!(
        rig.app.point_text(0).ends_with("Stereo"),
        "{}",
        rig.app.point_text(0)
    );
    set_plain_int(&rig.app.params.places[0].placement, 1);
    rig.press(Vec::new());
    assert!(rig.app.point_text(0).ends_with("Left"));
    rig.app.telemetry.publish_channels(1);
    assert!(rig.app.point_text(0).ends_with("Left, not heard in mono"));
    set_plain_int(&rig.app.params.places[0].placement, 0);
    rig.press(Vec::new());
    assert!(
        !rig.app.point_text(0).contains("Stereo"),
        "{}",
        rig.app.point_text(0)
    );
}

/// **Split copies every field** (plan §7.3): the dynamics and the side chain too, the copy Right.
#[test]
fn split_copies_every_field() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    let dynamics = &rig.app.params.dynamics[0];
    set_plain(&dynamics.range, -9.0);
    set_plain(&dynamics.threshold, -33.0);
    set_plain(&dynamics.attack, 4.0);
    set_plain(&dynamics.release, 300.0);
    set_plain_bool(&rig.app.params.keys[0].side_chain, true);
    rig.press(Vec::new());
    rig.app.split(0, false);
    rig.press(Vec::new());
    let (original, copy) = (rig.app.dynamics_now(0), rig.app.dynamics_now(1));
    for (a, b) in [
        (copy.range_db, original.range_db),
        (copy.threshold_db, original.threshold_db),
        (copy.attack_ms, original.attack_ms),
        (copy.release_ms, original.release_ms),
    ] {
        assert!((a - b).abs() < 1e-2, "{copy:?} {original:?}");
    }
    assert!(rig.app.key_now(1), "the side chain");
    assert_eq!(rig.app.placement_now(1), mxm_fx_eq_n_dsp::Placement::Right);
}

/// **An unheard band shows no range** (plan §7.3): a dynamic Left band with Range has its reach in
/// stereo and none in mono, where it moves nothing.
#[test]
fn an_unheard_band_has_no_range_line() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    rig.press(Vec::new());
    set_plain(&rig.app.params.dynamics[0].range, -9.0);
    set_plain_int(&rig.app.params.places[0].placement, 1);
    rig.press(Vec::new());
    assert!(rig.app.range_reach_db(0).is_some());
    rig.app.telemetry.publish_channels(1);
    assert_eq!(rig.app.range_reach_db(0), None);
}

/// **MENU on the graph opens the selected band's menu** (mxm-keys' MENU: the right-click menu of
/// what the cursor is on), the one a right-click opens; with no band selected, the background's,
/// Paste and Select all (plan §7.9).
#[test]
fn menu_on_the_graph_opens_the_selected_band_s_menu() {
    let menu_shows = |rig: &mut Rig, item: &str| {
        let output = rig.frame(Rig::opening(), Vec::new());
        output
            .platform_output
            .accesskit_update
            .is_some_and(|update| {
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label().is_some_and(|label| label == item))
            })
    };
    let mut rig = Rig::new();
    rig.ctx.enable_accesskit();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(MENU)]);
    assert!(
        menu_shows(&mut rig, "Select all") && !menu_shows(&mut rig, "Delete"),
        "no band: the background's menu"
    );
    rig.type_keys(&[job_key(BACK)]);
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.press(Vec::new());
    assert_eq!(rig.app.selected, Some(0));
    rig.type_keys(&[job_key(MENU)]);
    assert!(menu_shows(&mut rig, "Delete"), "the band's menu");
    assert_eq!(rig.app.context_slot, Some(0));
}

/// **The side chain is offered only on a layout with the port** (plan §7.4, §7.8: one of the
/// External selector's choices), and choosing it tells the audio thread to copy the key.
#[test]
fn the_side_chain_is_offered_only_with_the_port() {
    for (port, offered) in [(false, false), (true, true)] {
        let rig = Rig::new();
        rig.app.telemetry.publish_side_chain_port(port);
        let names = rig.app.external_names();
        assert_eq!(names.contains(&"Side chain".to_owned()), offered);
        assert_eq!(names[0], "Off");
    }
    let mut rig = Rig::new();
    rig.app.telemetry.publish_side_chain_port(true);
    rig.press(Vec::new());
    assert!(!rig.app.telemetry.side_chain_wanted());
    rig.app.set_view(crate::view::ViewSettings {
        side_chain: true,
        ..rig.app.params.view.get()
    });
    rig.press(Vec::new());
    assert!(
        rig.app.telemetry.side_chain_wanted(),
        "the view reaches the audio thread"
    );
}

/// A rig whose analyser holds a resonance at `centre` over a floor, frozen: the hold is fed that
/// spectrum, then Freeze is asked for and a frame makes it.
fn frozen_rig(centre: f32) -> Rig {
    frozen_rig_at(centre, 1.0e-3)
}

/// [`frozen_rig`] with the resonance's top at `top` in power (1e-3 is −30 dB) over a −60 dB floor.
fn frozen_rig_at(centre: f32, top: f32) -> Rig {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let freqs = graph::Curves::compute(&rig.app, &axes).freqs;
    let levels: Vec<f32> = freqs
        .iter()
        .map(|&f| {
            let detune = 8.0 * (f / centre - centre / f);
            10.0 * (1.0e-6 + top / (1.0 + detune * detune)).log10()
        })
        .collect();
    let now = rig.ctx.input(|input| input.time);
    rig.app.hold.push(now, &levels);
    rig.app.hold_basis = Some(grab::Basis {
        view: rig.app.params.view.get(),
        generation: rig.app.spectrum.generation(),
    });
    rig.app.freeze_asked = true;
    rig.press(Vec::new());
    rig
}

/// **Freeze holds the last two seconds and marks its peaks** (plan §7.4); a change of an analyser
/// setting, the analyser off, the editor closed and a restore each end it.
#[test]
fn freeze_holds_marks_its_peaks_and_ends_on_any_change_under_it() {
    let rig = frozen_rig(1_000.0);
    let frozen = rig.app.frozen.as_ref().expect("frozen");
    assert!(frozen.source_post, "post, as it is shown");
    assert_eq!(frozen.peaks.len(), 1);
    assert!((frozen.peaks[0].freq / 1_000.0).log2().abs() < 0.05);
    let held = frozen.post.clone();
    let mut rig = rig;
    for _ in 0..30 {
        rig.press(Vec::new());
    }
    assert_eq!(
        rig.app.frozen.as_ref().unwrap().post,
        held,
        "nothing moves while frozen"
    );

    let mut settings = rig.app.params.view.get();
    settings.tilt = crate::view::Tilt::Three;
    rig.app.set_view(settings);
    rig.press(Vec::new());
    assert!(rig.app.frozen.is_none(), "a setting change ends it");

    let mut rig = frozen_rig(1_000.0);
    let mut off = rig.app.params.view.get();
    off.analyser = false;
    rig.app.set_view(off);
    rig.press(Vec::new());
    assert!(!rig.app.freezing(), "the analyser off ends it");

    let mut rig = frozen_rig(1_000.0);
    let saved = rig.app.params.view.get();
    rig.app.detach();
    assert!(rig.app.frozen.is_none(), "closing the editor ends it");
    assert_eq!(rig.app.params.view.get(), saved, "nothing of it saved");

    let mut rig = frozen_rig(1_000.0);
    nice_plug::params::persist::PersistentField::set(
        &rig.app.params.view,
        rig.app.params.view.get(),
    );
    rig.press(Vec::new());
    assert!(rig.app.frozen.is_none(), "a restore ends it");
}

/// **A grab makes a bell at the peak, its Q from the width, its gain from the drag** (plan §7.4),
/// as one undo step; the frequency stays at the peak.
#[test]
fn a_drag_from_a_peak_makes_its_bell_as_one_step() {
    let mut rig = frozen_rig(1_000.0);
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let from = egui::pos2(axes.x(peak.freq), axes.spectrum_y(peak.db));
    let depth = rig.depth();
    let to = from + egui::vec2(40.0, 60.0);
    rig.press(pointer(from, Some(true)));
    for step in 1..=8 {
        rig.press(pointer(from + (to - from) * (step as f32 / 8.0), None));
    }
    rig.press(pointer(to, Some(false)));
    let band = rig.band(0);
    assert_eq!(band.shape, Some(Shape::Bell));
    assert!(
        (band.freq / peak.freq - 1.0).abs() < 1.0e-3,
        "at the peak: {}",
        band.freq
    );
    assert!((band.q - peak.q).abs() < 1.0e-3, "its width's Q");
    assert!(band.gain_db < -1.0, "down cuts: {}", band.gain_db);
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **The keyboard grabs too**: frozen with no band selected, → walks the peaks and ADD makes the
/// focused one's bell.
#[test]
fn the_keyboard_walks_the_peaks_and_add_grabs_one() {
    let mut rig = frozen_rig(2_000.0);
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    rig.app.focus_freq = 100.0;
    rig.app.on_surface(mxm_keys::Output::Navigate {
        direction: mxm_keys::Direction::Right,
        coarse: false,
    });
    assert_eq!(rig.app.focus_peak, Some(0));
    assert_eq!(rig.app.focus_freq, peak.freq);
    rig.app
        .on_surface(mxm_keys::Output::Action(mxm_keys::Action::Add));
    rig.press(Vec::new());
    let band = rig.band(0);
    assert!((band.freq / peak.freq - 1.0).abs() < 1.0e-3);
    assert!((band.q - peak.q).abs() < 1.0e-3);
}

/// **With every slot in use a peak makes nothing.**
#[test]
fn a_full_set_grabs_nothing() {
    let mut rig = frozen_rig(1_000.0);
    for slot in 0..SLOTS {
        rig.app.insert_at(30.0 * 1.3f32.powi(slot as i32));
    }
    rig.press(Vec::new());
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    assert!(!rig.app.free_slot_exists());
    assert_eq!(rig.app.insert_peak(peak, false), None);
}

/// **A grab takes the lowest Off slot, BACK during its drag leaves nothing, and a click on a peak
/// makes nothing** (plan §7.4).
#[test]
fn a_grab_takes_the_lowest_off_slot_and_back_or_a_click_leaves_nothing() {
    let mut rig = frozen_rig(1_000.0);
    for freq in [60.0, 120.0, 240.0] {
        rig.app.insert_at(freq);
    }
    rig.app.set_shape(1, Shape::Bell);
    let kind = band_index(1, Field::Type);
    rig.app.instant(&[(kind, 0.0)]);
    rig.press(Vec::new());
    assert_eq!(rig.band(1).shape, None, "slot 1 is Off");
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let from = egui::pos2(axes.x(peak.freq), axes.spectrum_y(peak.db));
    let to = from + egui::vec2(0.0, 50.0);

    // A click: no drag, no band.
    rig.press(pointer(from, Some(true)));
    rig.press(pointer(from, Some(false)));
    rig.press(Vec::new());
    assert_eq!(rig.band(1).shape, None, "a click makes nothing");
    rig.app.selected = None;

    let depth = rig.depth();
    let drag = |rig: &mut Rig, back: bool| {
        rig.press(pointer(from, Some(true)));
        for step in 1..=8 {
            rig.press(pointer(from + (to - from) * (step as f32 / 8.0), None));
        }
        if back {
            rig.press(vec![
                key(job_key(BACK), true, NONE),
                key(job_key(BACK), false, NONE),
            ]);
        }
        rig.press(pointer(to, Some(false)));
        rig.press(Vec::new());
    };
    drag(&mut rig, true);
    assert_eq!(rig.band(1).shape, None, "BACK took it away");
    assert_eq!(rig.depth(), depth, "and left nothing to undo");
    assert!(rig.gestures_balanced());

    drag(&mut rig, false);
    let band = rig.band(1);
    assert_eq!(band.shape, Some(Shape::Bell), "the lowest Off slot");
    assert!((band.freq / peak.freq - 1.0).abs() < 1.0e-3);
    assert_eq!(rig.band(3).shape, None);
}

/// **The hold never mixes representations** (plan §7.4): levels held under one Tilt are gone once
/// it changes, so a freeze right after holds only what the new Tilt shows.
#[test]
fn a_setting_change_restarts_the_hold() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let freqs = graph::Curves::compute(&rig.app, &axes).freqs;
    let loud: Vec<f32> = freqs
        .iter()
        .map(|&f| {
            if (f / 1_000.0).log2().abs() < 0.05 {
                -10.0
            } else {
                -60.0
            }
        })
        .collect();
    let now = rig.ctx.input(|input| input.time);
    rig.app.hold.push(now, &loud);
    rig.app.hold_basis = Some(grab::Basis {
        view: rig.app.params.view.get(),
        generation: rig.app.spectrum.generation(),
    });
    let mut settings = rig.app.params.view.get();
    settings.tilt = crate::view::Tilt::Three;
    rig.app.set_view(settings);
    rig.app.freeze_asked = true;
    rig.press(Vec::new());
    let frozen = rig.app.frozen.as_ref().expect("frozen");
    assert!(
        frozen.peaks.is_empty(),
        "the old Tilt's peak is gone: {:?}",
        frozen.peaks
    );
    assert!(frozen.post.iter().all(|&db| db < -50.0));
}

/// **The side chain's line holds with the freeze** (plan §7.4): nothing moves while frozen, and
/// turning the Side chain view on or off ends it.
#[test]
fn the_side_chain_line_holds_while_frozen() {
    let mut rig = Rig::new();
    rig.app.telemetry.publish_side_chain_port(true);
    rig.app.set_view(crate::view::ViewSettings {
        side_chain: true,
        ..rig.app.params.view.get()
    });
    rig.press(Vec::new());
    rig.press(Vec::new());
    let len = rig.app.spectrum.key_db.len();
    rig.app.spectrum.key_db = vec![-20.0; len];
    rig.app.hold_basis = Some(grab::Basis {
        view: rig.app.params.view.get(),
        generation: rig.app.spectrum.generation(),
    });
    rig.app.freeze_asked = true;
    rig.press(Vec::new());
    let held = rig.app.frozen.as_ref().expect("frozen").key.clone();
    assert_eq!(held.len(), len);
    rig.app.spectrum.key_db = vec![-5.0; len];
    assert_eq!(
        rig.app.shown_spectra()[2],
        held.as_slice(),
        "the drawn key is the held one"
    );
    rig.press(Vec::new());
    assert!(rig.app.frozen.is_some());
    rig.app.set_view(crate::view::ViewSettings {
        side_chain: false,
        ..rig.app.params.view.get()
    });
    rig.press(Vec::new());
    assert!(rig.app.frozen.is_none(), "the view toggled ends it");
}

/// **A click on a peak makes nothing, even on the curve** (plan §7.4): a peak at −48 dB puts its
/// marker on the flat 0 dB curve, and a click there makes no band.
#[test]
fn a_click_on_a_peak_lying_on_the_curve_makes_nothing() {
    let mut rig = frozen_rig_at(1_000.0, 10f32.powf(-4.8));
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let at = egui::pos2(axes.x(peak.freq), axes.spectrum_y(peak.db));
    let curves = graph::Curves::compute(&rig.app, &axes);
    assert!(
        curves.distance(&axes, at) <= 8.0,
        "the marker lies on the curve"
    );
    rig.press(pointer(at, Some(true)));
    rig.press(pointer(at, Some(false)));
    rig.press(Vec::new());
    assert!(
        (0..SLOTS).all(|slot| rig.band(slot).shape.is_none()),
        "no band"
    );
}

/// **A freeze made stale by the frame's audio grabs nothing** (plan §7.4): the first block after a
/// reset arrives in the frame ADD is pressed, and ADD makes a plain bell, not the old peak's.
#[test]
fn add_in_the_frame_a_reset_s_audio_arrives_grabs_nothing() {
    let mut rig = frozen_rig(2_000.0);
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    rig.app.focus_freq = 100.0;
    rig.app.on_surface(mxm_keys::Output::Navigate {
        direction: mxm_keys::Direction::Right,
        coarse: false,
    });
    assert_eq!(rig.app.focus_peak, Some(0));
    let block = crate::analyser::Block {
        seq: 0,
        generation: 7,
        sample_rate: rig.app.spectrum.sample_rate(),
        channels: 2,
        pre: [[0.0; crate::analyser::BLOCK_FRAMES]; 2],
        post: [[0.0; crate::analyser::BLOCK_FRAMES]; 2],
        key: [[0.0; crate::analyser::BLOCK_FRAMES]; 2],
    };
    rig.app.spectrum.ingest(&block);
    rig.type_keys(&[job_key(ADD)]);
    assert!(rig.app.frozen.is_none(), "the reset's audio ended it");
    let band = rig.band(0);
    assert_eq!(band.shape, Some(Shape::Bell));
    assert!(
        (band.q - 1.0).abs() < 1.0e-3,
        "a plain bell, not the peak's Q {}",
        peak.q
    );
}

/// **A peak's label is part of the peak** (plan §7.4): the pointer finds the peak on its drawn
/// label, away from the marker, so a click there makes nothing and a drag grabs it.
#[test]
fn a_peak_s_label_is_part_of_the_peak() {
    let mut rig = frozen_rig(1_000.0);
    rig.press(Vec::new());
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    let &(_, label) = rig.app.peak_labels.first().expect("its label is drawn");
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let marker = egui::pos2(axes.x(peak.freq), axes.spectrum_y(peak.db));
    let corner = label.right_top() + egui::vec2(-1.0, 1.0);
    assert!(corner.distance(marker) > 12.0, "away from the marker");
    let found = rig
        .app
        .peak_at(&axes, corner)
        .expect("the label finds the peak");
    assert_eq!(found.freq, peak.freq);
}

/// **The band's menu answers the arrows** (the owner, 2026-10-15: "I cannot select any of these
/// with the arrows"): MENU opens it with the current shape's row holding the keyboard, ↓ walks
/// to the next shape and Enter picks it; ↓ to Delete and Enter deletes.
#[test]
fn the_band_menu_walks_with_the_arrows_and_enter_picks() {
    // Nothing copied, so Paste is disabled and the arrows pass it (plan §7.9).
    let _turn = clipboard_turn();
    let tap = |key_: Key| vec![key(key_, true, NONE), key(key_, false, NONE)];
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(ADD)]);
    rig.press(Vec::new());
    assert_eq!(rig.band(0).shape, Some(Shape::Bell));
    rig.type_keys(&[job_key(MENU)]);
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.press(tap(Key::ArrowDown));
    rig.press(tap(Key::Enter));
    rig.press(Vec::new());
    assert_eq!(rig.band(0).shape, Some(Shape::LowShelf), "↓ then Enter");
    assert!(!egui::Popup::is_any_open(&rig.ctx), "and the menu closed");

    // From the low shelf past the shapes, then Copy, Cut and Invert gain, to Delete, the last.
    rig.type_keys(&[job_key(MENU)]);
    rig.press(Vec::new());
    rig.press(Vec::new());
    for _ in 0..Shape::ALL.len() + 2 {
        rig.press(tap(Key::ArrowDown));
    }
    rig.press(tap(Key::Enter));
    rig.press(Vec::new());
    assert_eq!(rig.band(0).shape, None, "Delete, reached by the arrows");
    assert_eq!(
        rig.app.paste_refusal(),
        Some(super::selection::PasteRefusal::Empty),
        "Delete, not Cut: nothing was copied"
    );
}

/// **The display keeps nothing of before a change** (plan §7.4): a loud line drawn under one Tilt,
/// or before a reset, is gone from a freeze made the frame after the change, not falling out of it.
#[test]
fn a_freeze_after_a_change_holds_no_old_display_level() {
    for change in ["tilt", "reset"] {
        let mut rig = Rig::new();
        rig.press(Vec::new());
        rig.press(Vec::new());
        let len = rig.app.spectrum.post_db.len();
        assert!(len > 0);
        rig.app.spectrum.post_db = vec![-10.0; len];
        rig.app.hold_basis = Some(grab::Basis {
            view: rig.app.params.view.get(),
            generation: rig.app.spectrum.generation(),
        });
        if change == "tilt" {
            let mut settings = rig.app.params.view.get();
            settings.tilt = crate::view::Tilt::Three;
            rig.app.set_view(settings);
        } else {
            rig.app.spectrum.ingest(&crate::analyser::Block {
                seq: 0,
                generation: 9,
                sample_rate: rig.app.spectrum.sample_rate(),
                channels: 2,
                pre: [[0.0; crate::analyser::BLOCK_FRAMES]; 2],
                post: [[0.0; crate::analyser::BLOCK_FRAMES]; 2],
                key: [[0.0; crate::analyser::BLOCK_FRAMES]; 2],
            });
        }
        rig.app.freeze_asked = true;
        rig.press(Vec::new());
        let frozen = rig.app.frozen.as_ref().expect("frozen");
        assert!(
            frozen.post.iter().all(|&db| db < -50.0),
            "{change}: no old level held"
        );
    }
}

/// **A resize ends a freeze before the pointer** (plan §7.4): a press on the old peak that moves
/// past the drag threshold in the first frame at a new width makes no band.
#[test]
fn a_resize_ends_the_freeze_before_the_pointer() {
    let wider = Rig::opening() + egui::vec2(240.0, 0.0);
    let mut sizing = Rig::new();
    for _ in 0..2 {
        sizing.frame(wider, Vec::new());
    }
    let axes = graph::Axes::new(sizing.app.graph_rect, sizing.app.high());
    let mut rig = frozen_rig(1_000.0);
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    let at = graph::peak_marker(&axes, &peak);
    let mut events = pointer(at, Some(true));
    events.push(egui::Event::PointerMoved(at + egui::vec2(0.0, 30.0)));
    rig.frame(wider, events);
    assert!(rig.app.frozen.is_none(), "the new width ended it");
    rig.frame(wider, pointer(at + egui::vec2(0.0, 30.0), Some(false)));
    assert!(
        (0..SLOTS).all(|slot| rig.band(slot).shape.is_none()),
        "no band"
    );
}

/// **A peak louder than the drawn range stays on the graph** (plan §7.4): a peak reading +5 dB
/// keeps its marker and label inside the graph, and the pointer finds and grabs it there.
#[test]
fn a_loud_peak_is_drawn_and_grabbed_inside_the_graph() {
    let mut rig = frozen_rig_at(10_000.0, 10f32.powf(0.5));
    rig.press(Vec::new());
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    assert!(peak.db > 0.0, "{}", peak.db);
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let at = graph::peak_marker(&axes, &peak);
    assert!(axes.rect.contains(at), "the marker on the graph");
    let &(_, label) = rig.app.peak_labels.first().expect("its label is drawn");
    assert!(
        axes.rect.contains_rect(label),
        "the label on the graph: {label:?}"
    );
    assert_eq!(
        rig.app.peak_at(&axes, at).map(|found| found.freq),
        Some(peak.freq)
    );
    let to = at + egui::vec2(0.0, 60.0);
    rig.press(pointer(at, Some(true)));
    for step in 1..=8 {
        rig.press(pointer(at + (to - at) * (step as f32 / 8.0), None));
    }
    rig.press(pointer(to, Some(false)));
    assert_eq!(rig.band(0).shape, Some(Shape::Bell), "grabbed");
}

/// **Any resize ends a freeze, however small** (plan §7.4): half a point wider keeps the number of
/// points but moves the grid, and the freeze ends.
#[test]
fn a_resize_too_small_to_change_the_count_ends_the_freeze() {
    let mut rig = frozen_rig(1_000.0);
    let width = rig.app.graph_rect.width();
    let delta = if (width / 2.0).fract() < 0.75 {
        0.5
    } else {
        -0.5
    };
    let points = rig.app.frozen.as_ref().unwrap().freqs.len();
    let size = Rig::opening() + egui::vec2(delta, 0.0);
    rig.frame(size, Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    assert_eq!(
        graph::Curves::compute(&rig.app, &axes).freqs.len(),
        points,
        "the count holds"
    );
    assert!(rig.app.frozen.is_none(), "the moved grid ended it");
}

/// **A label near either side stays inside the graph** (plan §7.4).
#[test]
fn a_label_near_a_side_is_shifted_into_the_graph() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let size = egui::vec2(48.0, 12.0);
    let y = axes.rect.center().y;
    for at in [
        egui::pos2(axes.rect.left() + 3.0, y),
        egui::pos2(axes.rect.right() - 3.0, y),
    ] {
        let label = graph::peak_label(&axes, at, size);
        assert!(
            axes.rect.contains_rect(label),
            "{label:?} in {:?}",
            axes.rect
        );
    }
}

/// **A click is judged where it was pressed** (plan §7.4): pressed on a label, let go a little
/// lower on the curve, within a click's reach, it makes nothing.
#[test]
fn a_click_pressed_on_a_label_and_let_go_on_the_curve_makes_nothing() {
    // A peak whose marker sits 6 points under the flat curve: its label ends just above it.
    let mut sizing = Rig::new();
    sizing.press(Vec::new());
    sizing.press(Vec::new());
    let axes = graph::Axes::new(sizing.app.graph_rect, sizing.app.high());
    let per_db = axes.spectrum_y(-47.0) - axes.spectrum_y(-48.0);
    let db = -48.0 + (axes.y(0.0) + 6.0 - axes.spectrum_y(-48.0)) / per_db;
    let mut rig = frozen_rig_at(1_000.0, 10f32.powf(db / 10.0) - 1.0e-6);
    rig.press(Vec::new());
    let peak = rig.app.frozen.as_ref().unwrap().peaks[0];
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let marker = graph::peak_marker(&axes, &peak);
    let &(_, label) = rig.app.peak_labels.first().expect("its label is drawn");
    let press = egui::pos2(label.left() + 2.0, label.bottom() - 1.0);
    let release = egui::pos2(press.x, axes.y(0.0) + 1.0);
    assert!(press.distance(release) < 6.0, "within a click's reach");
    assert!(!label.contains(release) && release.distance(marker) > 12.0);
    let curves = graph::Curves::compute(&rig.app, &axes);
    assert!(
        curves.distance(&axes, release) <= 8.0,
        "let go on the curve"
    );
    rig.press(pointer(press, Some(true)));
    rig.press(pointer(release, Some(false)));
    rig.press(Vec::new());
    assert!(
        (0..SLOTS).all(|slot| rig.band(slot).shape.is_none()),
        "no band"
    );
}

/// **The side chain's "SC" stays on the graph**: a loud key's line ending above the top keeps its
/// label a line of text inside.
#[test]
fn a_loud_key_s_label_stays_on_the_graph() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let end = egui::pos2(axes.rect.right(), axes.rect.top() - 30.0);
    let at = graph::side_chain_label_at(&axes, end);
    assert!(
        at.y - 13.0 >= axes.rect.top(),
        "{at:?} under {:?}",
        axes.rect.top()
    );
}

// ---- Find (plan §7.5) ----------------------------------------------------------------------

/// A candidate as the finder would have measured it.
fn candidate(freq: f32, bands: &[(finder::Domain, f32, f32)]) -> finder::Candidate {
    let bands: Vec<finder::Measured> = bands
        .iter()
        .map(|&(domain, prominence, q)| finder::Measured {
            domain,
            rung: 0,
            freq,
            db: -30.0,
            prominence,
            q,
        })
        .collect();
    finder::Candidate {
        freq,
        audibility: bands
            .iter()
            .map(finder::Measured::audibility)
            .reduce(f32::max)
            .unwrap(),
        bands,
        seen: 0.0,
    }
}

/// A rig with Find on and `candidates` as if the finder had found them, its search run.
fn finding_rig(candidates: Vec<finder::Candidate>) -> Rig {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    finding(&mut rig, candidates);
    rig
}

/// Find on in `rig`, with `candidates` found and proposed on a frame.
fn finding(rig: &mut Rig, candidates: Vec<finder::Candidate>) {
    rig.app.set_find(true);
    rig.app.candidates = candidates;
    rig.app.found_revision = rig.app.finder.revision();
    rig.app.found_at = Some(rig.time);
    rig.app.proposals_stale = true;
    rig.press(Vec::new());
}

/// Where a proposal's triangle is, for the pointer.
fn triangle(rig: &Rig, index: usize) -> egui::Pos2 {
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let curves = graph::Curves::compute(&rig.app, &axes);
    let freq = rig.app.proposals[index].freq;
    graph::proposal_marker(&axes, &curves, rig.app.proposal_spectrum(), freq) - egui::vec2(0.0, 5.0)
}

/// **Every apply writes a band's every field** (plan §7.5): a static, unkeyed bell at its
/// placement, whatever the Off slot held, as one undo step; what was applied leaves the proposals.
#[test]
fn applying_a_proposal_writes_every_field_over_a_stale_slot_as_one_step() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    // Slot 1 left stale: a bypassed, keyed, dynamic Left low cut, then turned Off.
    let stale = rig.app.slot_writes(
        0,
        mxm_fx_eq_n_dsp::Band {
            shape: Some(Shape::LowCut),
            bypass: true,
            freq: 300.0,
            gain_db: 5.0,
            q: 3.0,
            slope: Slope::Db48,
        },
        mxm_fx_eq_n_dsp::Dynamics {
            range_db: -9.0,
            ..mxm_fx_eq_n_dsp::Dynamics::STATIC
        },
        true,
        mxm_fx_eq_n_dsp::Placement::Left,
    );
    rig.app.instant(&stale);
    rig.app.delete(0);
    rig.press(Vec::new());
    finding(
        &mut rig,
        vec![candidate(2_000.0, &[(finder::Domain::Mid, 10.0, 6.0)])],
    );
    assert_eq!(rig.app.proposals.len(), 1);
    let proposed = rig.app.proposals[0].bands[0];
    let depth = rig.depth();
    assert_eq!(rig.app.apply_proposals(&[0], false), [0]);
    assert!(
        rig.app.proposals.is_empty(),
        "applied, it leaves the proposals at once"
    );
    rig.press(Vec::new());
    let band = rig.band(0);
    assert_eq!(band.shape, Some(Shape::Bell));
    assert!(!band.bypass);
    assert!((band.freq - 2_000.0).abs() < 1.0, "{}", band.freq);
    assert!(
        (band.gain_db - proposed.gain_db).abs() < 0.05,
        "{}",
        band.gain_db
    );
    assert!((band.q - 6.0).abs() < 1.0e-3);
    assert_eq!(band.slope, crate::params::DEFAULT_SLOPE);
    assert_eq!(rig.app.dynamics_now(0).range_db, 0.0, "static");
    assert!(!rig.app.key_now(0));
    assert_eq!(rig.app.placement_now(0), mxm_fx_eq_n_dsp::Placement::Mid);
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **Apply all, a click and a drag are each one undo step, and BACK during the drag leaves
/// nothing** (plan §7.5); a pair is applied as its two bands and dragged together.
#[test]
fn apply_all_a_click_and_a_drag_are_each_one_step_and_back_leaves_nothing() {
    let found = || {
        vec![
            candidate(
                500.0,
                &[
                    (finder::Domain::Left, 14.0, 6.0),
                    (finder::Domain::Right, 8.0, 6.0),
                ],
            ),
            candidate(3_000.0, &[(finder::Domain::Average, 10.0, 8.0)]),
        ]
    };
    let mut rig = finding_rig(found());
    assert_eq!(rig.app.proposals.len(), 2);
    let depth = rig.depth();
    let all: Vec<usize> = (0..rig.app.proposals.len()).collect();
    assert_eq!(rig.app.apply_proposals(&all, false), [0, 1, 2]);
    rig.press(Vec::new());
    assert_eq!(rig.depth(), depth + 1, "Apply all is one step");
    rig.app.undo();
    rig.press(Vec::new());
    assert!(
        (0..3).all(|slot| !rig.app.band_in_use(slot)),
        "and one undo takes all back"
    );

    // A click on a triangle.
    let mut rig = finding_rig(found());
    let depth = rig.depth();
    let at = triangle(&rig, 1);
    rig.press(pointer(at, Some(true)));
    rig.press(pointer(at, Some(false)));
    rig.press(Vec::new());
    assert_eq!(rig.band(0).shape, Some(Shape::Bell), "the click applied it");
    assert!((rig.band(0).freq - 3_000.0).abs() < 1.0);
    assert_eq!(rig.depth(), depth + 1);

    // A drag from the pair's triangle: both bands, their gains moving together.
    let mut rig = finding_rig(found());
    let proposal = rig.app.proposals[0].clone();
    let depth = rig.depth();
    let from = triangle(&rig, 0);
    let to = from + egui::vec2(0.0, 60.0);
    rig.press(pointer(from, Some(true)));
    for step in 1..=6 {
        rig.press(pointer(from + (to - from) * (step as f32 / 6.0), None));
    }
    rig.press(pointer(to, Some(false)));
    rig.press(Vec::new());
    let (left, right) = (rig.band(0), rig.band(1));
    assert!(
        left.gain_db < proposal.bands[0].gain_db - 1.0,
        "dragged deeper"
    );
    let apart = proposal.bands[1].gain_db - proposal.bands[0].gain_db;
    assert!(
        ((right.gain_db - left.gain_db) - apart).abs() < 0.05,
        "the pair moves together"
    );
    assert_eq!(rig.depth(), depth + 1, "the drag is one step");
    assert_eq!(
        rig.app.selection(),
        vec![0, 1],
        "a pair is a group (plan §7.9)"
    );

    // BACK during the drag: nothing is left.
    let mut rig = finding_rig(found());
    let depth = rig.depth();
    let from = triangle(&rig, 1);
    let to = from + egui::vec2(0.0, 60.0);
    rig.press(pointer(from, Some(true)));
    for step in 1..=6 {
        rig.press(pointer(from + (to - from) * (step as f32 / 6.0), None));
    }
    rig.press(vec![
        key(Key::Escape, true, NONE),
        key(Key::Escape, false, NONE),
    ]);
    rig.press(pointer(to, Some(false)));
    rig.press(Vec::new());
    assert!(!rig.app.band_in_use(0), "BACK took the band away");
    assert_eq!(rig.depth(), depth, "and left no step");
    assert!(rig.gestures_balanced());
}

/// **The keyboard walks the proposals and ADD applies one** (plan §7.5).
#[test]
fn the_keyboard_walks_the_proposals_and_add_applies_one() {
    let mut rig = finding_rig(vec![
        candidate(400.0, &[(finder::Domain::Average, 10.0, 8.0)]),
        candidate(2_500.0, &[(finder::Domain::Average, 12.0, 8.0)]),
    ]);
    rig.app.selected = None;
    rig.app.focus_freq = 1_000.0;
    rig.app.on_surface(mxm_keys::Output::Navigate {
        direction: mxm_keys::Direction::Right,
        coarse: false,
    });
    let index = rig.app.focus_proposal.expect("a proposal focused");
    assert_eq!(rig.app.proposals[index].freq, 2_500.0);
    rig.app
        .on_surface(mxm_keys::Output::Action(mxm_keys::Action::Add));
    rig.press(Vec::new());
    let band = rig.band(0);
    assert_eq!(band.shape, Some(Shape::Bell));
    assert!((band.freq - 2_500.0).abs() < 1.0);
}

/// **Find and Freeze exclude each other; Find needs the analyser; closing the editor ends it; it
/// is never saved** (plan §7.5). Amount is saved, an edit marks the project dirty, and it ends no
/// freeze.
#[test]
fn find_s_toggles_follow_their_rules_and_amount_is_view_state() {
    let mut rig = frozen_rig(1_000.0);
    assert!(rig.app.frozen.is_some());
    let saved = rig.app.params.view.get();
    rig.app.set_find(true);
    rig.press(Vec::new());
    assert!(
        rig.app.find && rig.app.frozen.is_none(),
        "Find ends a freeze"
    );
    rig.app.ask_freeze();
    rig.press(Vec::new());
    assert!(!rig.app.find, "Freeze ends Find");
    assert_eq!(rig.app.params.view.get(), saved, "neither is saved");

    let mut rig = finding_rig(Vec::new());
    let mut off = rig.app.params.view.get();
    off.analyser = false;
    rig.app.set_view(off);
    rig.press(Vec::new());
    assert!(!rig.app.find, "the analyser off ends Find");
    off.analyser = true;
    rig.app.set_view(off);
    rig.press(Vec::new());
    assert!(!rig.app.find, "and back on leaves it off");

    let mut rig = finding_rig(Vec::new());
    rig.app.detach();
    assert!(!rig.app.find, "closing the editor ends it");

    let mut rig = frozen_rig(1_000.0);
    let states = *rig.host.states.lock().unwrap();
    let mut settings = rig.app.params.view.get();
    settings.amount = 50;
    rig.app.set_view(settings);
    rig.press(Vec::new());
    assert_eq!(rig.app.params.view.get().amount, 50);
    assert_eq!(*rig.host.states.lock().unwrap(), states + 1, "marked dirty");
    assert!(rig.app.frozen.is_some(), "an Amount edit ends no freeze");
}

/// **A larger Amount proposes deeper, and with every slot in use nothing is proposed** (plan
/// §7.5).
#[test]
fn amount_sets_the_depth_and_a_full_set_proposes_nothing() {
    let found = || vec![candidate(1_000.0, &[(finder::Domain::Average, 10.0, 8.0)])];
    let mut rig = finding_rig(found());
    let shallow = rig.app.proposals[0].bands[0].gain_db;
    assert!((shallow + 2.0).abs() < 0.05, "20 % of 10 dB: {shallow}");
    let mut settings = rig.app.params.view.get();
    settings.amount = 60;
    rig.app.set_view(settings);
    for _ in 0..8 {
        rig.press(Vec::new());
    }
    let deep = rig.app.proposals[0].bands[0].gain_db;
    assert!((deep + 6.0).abs() < 0.05, "60 %: {deep}");

    let mut rig = Rig::new();
    rig.press(Vec::new());
    for slot in 0..SLOTS {
        rig.app.insert_at(30.0 * 1.3f32.powi(slot as i32));
    }
    rig.press(Vec::new());
    finding(&mut rig, found());
    assert!(rig.app.proposals.is_empty());
    assert_eq!(rig.app.find_refusal(), Some(FindRefusal::Full));
}

/// **At a low gain scale a proposal is the depth Apply gives it** (plan §7.5): the band written
/// is heard at the depth shown.
#[test]
fn at_a_low_gain_scale_apply_gives_the_depth_shown() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    set_plain(&rig.app.params.gain_scale, 0.1);
    rig.press(Vec::new());
    finding(
        &mut rig,
        vec![candidate(1_000.0, &[(finder::Domain::Average, 30.0, 8.0)])],
    );
    let shown = rig.app.proposals[0].bands[0].gain_db;
    assert!(shown >= -3.0 - 1.0e-4, "a band's 30 dB at 10 %: {shown}");
    rig.app.apply_proposals(&[0], false);
    rig.press(Vec::new());
    let heard = rig.band(0).gain_db * 0.1;
    assert!((heard - shown).abs() < 0.05, "heard {heard}, shown {shown}");
}

/// **A new rate starts the listen again** (plan §7.5): what was found and proposed goes.
#[test]
fn a_new_rate_forgets_what_was_found() {
    let mut rig = finding_rig(vec![candidate(
        1_000.0,
        &[(finder::Domain::Average, 10.0, 8.0)],
    )]);
    assert!(!rig.app.proposals.is_empty());
    rig.telemetry.publish_sample_rate(96_000.0);
    rig.press(Vec::new());
    assert!(rig.app.proposals.is_empty() && rig.app.candidates.is_empty());
    assert!(rig.app.find, "Find stays on, listening afresh");
}

/// **The keyboard's proposal follows its frequency** (plan §7.5): when a search reorders the
/// proposals, the focus stays on the one it was on.
#[test]
fn the_keyboard_s_proposal_follows_its_frequency_through_a_refresh() {
    let mut rig = finding_rig(vec![
        candidate(400.0, &[(finder::Domain::Average, 10.0, 8.0)]),
        candidate(2_500.0, &[(finder::Domain::Average, 12.0, 8.0)]),
    ]);
    rig.app.selected = None;
    rig.app.focus_freq = 1_000.0;
    rig.app.on_surface(mxm_keys::Output::Navigate {
        direction: mxm_keys::Direction::Right,
        coarse: false,
    });
    assert_eq!(rig.app.focus_freq, 2_500.0);
    // A new search finds a stronger one first.
    let found = vec![
        candidate(5_000.0, &[(finder::Domain::Average, 20.0, 8.0)]),
        candidate(400.0, &[(finder::Domain::Average, 10.0, 8.0)]),
        candidate(2_500.0, &[(finder::Domain::Average, 12.0, 8.0)]),
    ];
    rig.app.candidates = found;
    rig.app.proposals_stale = true;
    rig.press(Vec::new());
    let index = rig.app.focus_proposal.expect("still focused");
    assert_eq!(rig.app.proposals[index].freq, 2_500.0);
    rig.app
        .on_surface(mxm_keys::Output::Action(mxm_keys::Action::Add));
    rig.press(Vec::new());
    assert!((rig.band(0).freq - 2_500.0).abs() < 1.0, "ADD applies it");
}

/// **A rate the display cannot read is built for once** (plan §7.5): below 8 kHz the display takes
/// another rate, and the editor still compares with the rate it built for, so neither the spectrum
/// nor the finder is built again every frame.
#[test]
fn a_low_rate_builds_the_analysers_once() {
    let mut rig = finding_rig(Vec::new());
    rig.telemetry.publish_sample_rate(4_000.0);
    rig.press(Vec::new());
    assert_eq!(rig.app.finder.sample_rate(), 4_000.0);
    let version = rig.app.proposals_version;
    rig.press(Vec::new());
    rig.press(Vec::new());
    assert_eq!(
        rig.app.proposals_version, version,
        "not forgotten again each frame"
    );
}

// ---- Match (plan §7.6) -----------------------------------------------------------------------

use super::SaveRefusal;
use super::matcher::{Fitted, MatchBand};
use crate::analyser::{BLOCK_FRAMES, Block};
use crate::instances::Instance;
use crate::learned::{Learned, MatchTo};

/// White noise uniform in ±0.3 (xorshift), the same run each time.
fn noise() -> impl FnMut() -> f32 {
    let mut state = 0x2545_f491u32;
    move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        0.3 * ((state as f32 / u32::MAX as f32) * 2.0 - 1.0)
    }
}

/// `seconds` of blocks at 48 kHz, each filled sample by sample with what `next` gives: the input
/// before the EQ, then the output after it.
fn blocks(seconds: f32, mut next: impl FnMut() -> (f32, f32)) -> Vec<Block> {
    let rate = 48_000.0f32;
    (0..(rate * seconds) as u64 / BLOCK_FRAMES as u64)
        .map(|seq| {
            let mut block = Block {
                seq,
                generation: 0,
                sample_rate: rate,
                channels: 2,
                pre: [[0.0; BLOCK_FRAMES]; 2],
                post: [[0.0; BLOCK_FRAMES]; 2],
                key: [[0.0; BLOCK_FRAMES]; 2],
            };
            for index in 0..BLOCK_FRAMES {
                let (pre, post) = next();
                block.pre[0][index] = pre;
                block.pre[1][index] = pre;
                block.post[0][index] = post;
                block.post[1][index] = post;
            }
            block
        })
        .collect()
}

/// What another EQ learns of 11 s of noise whose output is tilted, `y[n] = x[n] + 0.9·x[n−1]`:
/// louder in the lows, so a track of the plain noise matched to it is proposed bands.
fn tilted_learned() -> Learned {
    let mut matcher = matcher::Matcher::new(48_000.0);
    let (mut next, mut last) = (noise(), 0.0f32);
    for block in blocks(11.0, move || {
        let x = next();
        let y = x + 0.9 * last;
        last = x;
        (x, y)
    }) {
        matcher.ingest(&block);
    }
    matcher.learned()
}

/// Another EQ in this process, on a track named `name`, that has learned `learned`.
fn other_eq(name: &str, learned: Option<Learned>) -> Arc<Instance> {
    let identity = crate::instances::IdentityField::default();
    let (_tap, consumer) = crate::analyser::ring_with(crate::instances::SHARE_BLOCKS);
    let other = Instance::join(identity.shared(), consumer);
    other.set_track(name);
    other.store_learned(learned.map(Arc::new));
    other
}

/// A rig whose EQ is matched to another's, which learned a tilted noise, and learns its own track;
/// the other EQ, kept for the test's length.
fn match_rig() -> (Rig, Arc<Instance>) {
    let other = other_eq("zz-match Outro", Some(tilted_learned()));
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.set_match_to(MatchTo::Track(other.id()));
    rig.app.set_learn(true);
    rig.press(Vec::new());
    (rig, other)
}

/// `seconds` of plain noise, input and output alike, straight into the matcher while Learn is on.
fn match_heard(rig: &mut Rig, seconds: f32) {
    let mut next = noise();
    for block in blocks(seconds, move || {
        let x = next();
        (x, x)
    }) {
        if rig.app.learning {
            rig.app.matcher.ingest(&block);
        }
    }
}

/// **Match needs only the analyser, and is one proposal set of three** (plan §7.6): offered on
/// every layout; Learn ends Find and a freeze, and each of them ends Learn, keeping what it
/// learned; the analyser off ends it; Match is never saved in the view.
#[test]
fn learn_needs_the_analyser_and_excludes_find_and_freeze() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    assert!(rig.app.match_offered(&rig.app.params.view.get()));
    rig.app.set_find(true);
    rig.app.set_learn(true);
    assert!(rig.app.learning && !rig.app.find, "Learn ends Find");
    match_heard(&mut rig, 1.0);
    rig.app.set_find(true);
    assert!(rig.app.find && !rig.app.learning, "Find ends Learn");
    assert!(
        rig.app.params.learned.get().is_some(),
        "and what it learned is kept"
    );
    rig.app.set_learn(true);
    rig.app.ask_freeze();
    rig.press(Vec::new());
    assert!(
        !rig.app.learning && rig.app.freezing(),
        "a freeze ends Learn"
    );
    rig.app.set_learn(true);
    assert!(!rig.app.freezing(), "Learn ends a freeze");

    let mut quiet = rig.app.params.view.get();
    quiet.analyser = false;
    rig.app.set_view(quiet);
    rig.press(Vec::new());
    assert!(!rig.app.learning, "the analyser off ends it");
    let mut on = quiet;
    on.analyser = true;
    rig.app.set_view(on);
    rig.press(Vec::new());
    assert_eq!(
        rig.app.params.view.get(),
        on,
        "Match is never saved in the view"
    );
}

/// **Learn keeps what it learned with the project** (plan §7.6): the lamp lit while it hears and
/// dark once nothing comes; turned off, what it learned is the project's, read back whole by
/// another instance, and nothing more is learned; on again, it learns anew.
#[test]
fn learn_keeps_what_it_learned_with_the_project() {
    let (mut rig, _other) = match_rig();
    assert!(!rig.app.hearing, "dark: nothing heard yet");
    match_heard(&mut rig, 11.0);
    rig.press(Vec::new());
    assert!(rig.app.hearing, "lit while it hears");
    for _ in 0..(super::LAMP_SECONDS * 60.0).ceil() as usize + 1 {
        rig.press(Vec::new());
    }
    assert!(!rig.app.hearing, "dark once nothing comes");
    rig.app.set_learn(false);
    let kept = rig.app.params.learned.get().expect("kept");
    assert!(kept.usable() && kept.seconds > 10.0);
    let fields = rig.app.params.serialize_fields();
    let restored = EqParams::default();
    restored.deserialize_fields(&fields);
    assert_eq!(
        restored.learned.get(),
        Some(kept.clone()),
        "read back whole"
    );
    assert_eq!(
        *restored.match_to.read().unwrap(),
        rig.app.match_to(),
        "and what it is matched to"
    );
    match_heard(&mut rig, 2.0);
    assert_eq!(
        rig.app.params.learned.get(),
        Some(kept),
        "nothing learned while off"
    );
    rig.app.set_learn(true);
    assert_eq!(rig.app.learned_seconds(), 0.0, "anew");
}

/// **One EQ is matched to another** (plan §7.6): Match to lists every other EQ that has learned,
/// by its track's name, never one that has not; chosen, the bands are proposed once this track is
/// learned too; Find hides them, and choosing what to match to again ends Find and brings them
/// back.
#[test]
fn one_eq_is_matched_to_another() {
    let (mut rig, other) = match_rig();
    let quiet = other_eq("zz-match Quiet", None);
    let choices = rig.app.match_choices();
    assert_eq!(choices[0], (MatchTo::Nothing, "Nothing".to_owned()));
    // Other tests' EQs share the process, so this one's name may carry a number.
    assert!(
        choices
            .iter()
            .any(|(to, name)| *to == MatchTo::Track(other.id())
                && name.starts_with("zz-match Outro")),
        "{choices:?}"
    );
    assert!(
        !choices
            .iter()
            .any(|(to, _)| *to == MatchTo::Track(quiet.id())),
        "one that has learned nothing is not offered"
    );
    match_heard(&mut rig, 11.0);
    rig.app.set_learn(false);
    rig.press(Vec::new());
    assert!(!rig.app.match_bands.is_empty(), "proposed");
    rig.app.set_find(true);
    rig.press(Vec::new());
    assert!(rig.app.match_bands.is_empty(), "Find hides them");
    rig.app.set_match_to(MatchTo::Nothing);
    rig.app.set_match_to(MatchTo::Track(other.id()));
    rig.press(Vec::new());
    assert!(!rig.app.find, "choosing ends Find");
    assert!(!rig.app.match_bands.is_empty(), "and brings them back");
}

/// **Preview hands the proposals over in the slots Apply writes** (plan §7.6): nothing while it is
/// off; on, every proposal in its free slot as Apply would write it, and nothing else; nothing once
/// it is off again or the window closes.
#[test]
fn preview_hands_the_proposals_to_the_free_slots() {
    let (mut rig, _other) = match_rig();
    match_heard(&mut rig, 11.0);
    rig.app.set_learn(false);
    rig.press(Vec::new());
    assert!(!rig.app.match_bands.is_empty(), "proposed");
    let read = |rig: &Rig| {
        let mut out = [None; mxm_fx_eq_n_dsp::SLOTS];
        rig.app.preview.read(&mut out);
        out
    };
    assert!(read(&rig).iter().all(Option::is_none), "off: nothing");
    rig.app.set_preview(true);
    rig.press(Vec::new());
    let bands = rig.app.match_bands.clone();
    let slots: Vec<usize> = rig.app.free_slots().take(bands.len()).collect();
    let out = read(&rig);
    for (slot, band) in slots.iter().zip(&bands) {
        let previewed = out[*slot].expect("in the slot Apply writes");
        assert_eq!(previewed.shape, Some(band.shape));
        assert!((previewed.gain_db - band.gain_db).abs() < 1.0e-4);
        assert!((previewed.q - band.q).abs() < 1.0e-6);
    }
    assert_eq!(
        out.iter().flatten().count(),
        slots.len(),
        "and nothing else"
    );
    rig.app.set_preview(false);
    rig.press(Vec::new());
    assert!(read(&rig).iter().all(Option::is_none), "off again");
    rig.app.set_preview(true);
    rig.press(Vec::new());
    mxm_editor::Panel::closed(&mut rig.app, &GuiContext::new(rig.host.clone()));
    assert!(read(&rig).iter().all(Option::is_none), "the window closed");
    assert!(!rig.app.previewing);
}

/// **What this EQ learned is saved as a reference by name, and matched to** (plan §7.6, decision
/// 48): Save asks for a name, this track's offered; saved, what it is matched to is unchanged; the
/// saved one chosen is what this track sounded like; a name taken is replaced only on the second
/// press; a file gone reads Missing and matches nothing.
#[test]
fn a_saved_reference_is_chosen_remembered_and_replaced_only_when_asked() {
    let dir = std::env::temp_dir()
        .join("mxm-fx-eq-n-references")
        .join(format!("editor-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut rig, other) = match_rig();
    rig.app.references_dir = Some(dir.clone());
    assert!(!rig.app.can_save_reference(), "not while it learns");
    match_heard(&mut rig, 11.0);
    rig.app.set_learn(false);
    assert!(rig.app.can_save_reference());

    rig.app.press_save_reference();
    let offered = rig.app.naming.clone().expect("a name asked for");
    assert!(!offered.name.is_empty());
    rig.app.naming.as_mut().unwrap().name = "Studio VO".to_owned();
    rig.app.press_save_reference();
    assert!(rig.app.naming.is_none(), "saved");
    assert_eq!(rig.app.match_to(), MatchTo::Track(other.id()), "unchanged");
    rig.app.set_match_to(MatchTo::Saved("Studio VO".to_owned()));
    let sound = rig.app.params.learned.get().unwrap().sound();
    assert_eq!(
        rig.app.matched_reference(),
        Some(sound),
        "the file holds what this track sounded like"
    );
    let fields = rig.app.params.serialize_fields();
    assert!(
        fields
            .get("match-to")
            .is_some_and(|field| field.contains("Studio VO")),
        "remembered with the project"
    );

    // Learned again and saved under the same name: replaced only on the second press.
    rig.app.set_learn(true);
    match_heard(&mut rig, 11.0);
    rig.app.set_learn(false);
    rig.app.press_save_reference();
    rig.app.naming.as_mut().unwrap().name = "Studio VO".to_owned();
    rig.app.press_save_reference();
    assert!(
        rig.app.naming.as_ref().is_some_and(|naming| naming.taken),
        "taken: asks again"
    );
    assert_eq!(
        rig.app.save_reference("Studio VO", false),
        Err(SaveRefusal::Taken)
    );
    rig.app.press_save_reference();
    assert!(rig.app.naming.is_none(), "replaced");
    assert_eq!(rig.app.saved_references.len(), 1);

    std::fs::remove_dir_all(&dir).unwrap();
    rig.app.refresh_references(1.0e6);
    let names: Vec<String> = rig
        .app
        .match_choices()
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    assert!(
        names.contains(&"Missing: Studio VO".to_owned()),
        "{names:?}"
    );
    assert_eq!(rig.app.matched_reference(), None, "nothing to match");
}

/// **What was learned is drawn at the analyser's level** (plan §7.6): nothing before Match has
/// anything; then this track's line and the reference's, each reading at 1 kHz the twelfth-octave
/// band power the analyser draws for it, within 1 dB: `4σ²·B/fs` for the white noise, and that
/// raised by `|1 + 0.9·e^(−jω)|²` for the tilted one.
#[test]
fn what_was_learned_is_drawn_at_the_analysers_level() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    assert!(rig.app.learned_lines(0.0).is_none(), "nothing learned");
    let (mut rig, _other) = match_rig();
    match_heard(&mut rig, 11.0);
    rig.app.set_learn(false);
    let lines = rig.app.learned_lines(1.0).expect("drawn while matched");
    // The noise is uniform in ±0.3: its variance is 0.3² / 3.
    let half = 2f32.powf(super::spectrum::BAND_OCTAVES / 2.0);
    let band = 1_000.0 * (half - half.recip());
    let white = 10.0 * (4.0 * 0.03 * band / 48_000.0).log10();
    let w = std::f32::consts::TAU * 1_000.0 / 48_000.0;
    let tilted = white + 10.0 * (1.81 + 1.8 * w.cos()).log10();
    for (cells, expected, which) in [(&lines[0], tilted, "Ref"), (&lines[1], white, "Track")] {
        let cell = super::matcher::cell_at(cells, 1_000.0).expect("read at 1 kHz");
        let drawn = graph::learned_display_db(cell, 1_000.0, 0.0);
        assert!(
            (drawn - expected).abs() <= 1.0,
            "{which}: {drawn} dB against {expected} dB"
        );
    }
}

/// **Apply writes the match on its current basis only, every field, as one undo step** (plan
/// §7.6): a fit from an older listen is dropped; a moved Amount refuses it; applied, a static,
/// unkeyed Stereo shelf and bell over a stale Off slot, and the match empties until the next fit.
#[test]
fn match_applies_on_its_current_basis_every_field_in_one_step() {
    let (mut rig, _other) = match_rig();
    let stale = rig.app.slot_writes(
        0,
        mxm_fx_eq_n_dsp::Band {
            shape: Some(Shape::LowCut),
            bypass: true,
            freq: 300.0,
            gain_db: 5.0,
            q: 3.0,
            slope: Slope::Db48,
        },
        mxm_fx_eq_n_dsp::Dynamics {
            range_db: -9.0,
            ..mxm_fx_eq_n_dsp::Dynamics::STATIC
        },
        true,
        mxm_fx_eq_n_dsp::Placement::Left,
    );
    rig.app.instant(&stale);
    rig.app.delete(0);
    rig.press(Vec::new());
    let settings = rig.app.params.view.get();
    let bands = vec![
        MatchBand {
            shape: Shape::LowShelf,
            freq: 120.0,
            gain_db: 3.0,
            q: matcher::SHELF_Q,
        },
        MatchBand {
            shape: Shape::Bell,
            freq: 2_000.0,
            gain_db: -2.0,
            q: 1.5,
        },
    ];
    rig.app.accept_fit(Fitted {
        bands: bands.clone(),
        listen: rig.app.matcher.listen(),
        basis: rig.app.match_basis(&settings),
    });
    rig.app.accept_fit(Fitted {
        bands: Vec::new(),
        listen: rig.app.matcher.listen().wrapping_sub(1),
        basis: rig.app.match_basis(&settings),
    });
    assert_eq!(
        rig.app.match_bands, bands,
        "an older listen's fit is dropped"
    );
    assert!(rig.app.match_current(&settings));
    let mut moved = settings;
    moved.match_amount = 70;
    assert!(
        rig.app.apply_match(&moved).is_empty(),
        "not on a basis that has moved"
    );
    let depth = rig.depth();
    assert_eq!(rig.app.apply_match(&settings), [0, 1]);
    assert!(rig.app.match_bands.is_empty());
    rig.press(Vec::new());
    let shelf = rig.band(0);
    assert_eq!(shelf.shape, Some(Shape::LowShelf));
    assert!(!shelf.bypass);
    assert!((shelf.freq - 120.0).abs() < 0.5 && (shelf.gain_db - 3.0).abs() < 0.05);
    assert_eq!(shelf.slope, crate::params::DEFAULT_SLOPE);
    assert_eq!(rig.app.dynamics_now(0).range_db, 0.0, "static");
    assert!(!rig.app.key_now(0));
    assert_eq!(rig.app.placement_now(0), mxm_fx_eq_n_dsp::Placement::Stereo);
    let bell = rig.band(1);
    assert_eq!(bell.shape, Some(Shape::Bell));
    assert!((bell.q - 1.5).abs() < 1.0e-3 && (bell.gain_db + 2.0).abs() < 0.05);
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **A fit runs on the worker, asked on what is current when it is free** (plan §7.6): while one
/// runs nothing more is asked; once it is back, the next is asked on the Amount moved meanwhile,
/// and what it proposes is current.
#[test]
fn a_fit_runs_on_the_worker_and_the_latest_basis_wins() {
    let (mut rig, _other) = match_rig();
    rig.app.inline_search = false;
    match_heard(&mut rig, 11.0);
    assert!(
        rig.app
            .learned()
            .is_some_and(|learned| learned.seconds > 10.0)
    );
    rig.press(Vec::new());
    let mut moved = rig.app.params.view.get();
    moved.match_amount = 60;
    rig.app.set_view(moved);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        rig.press(Vec::new());
        if rig.app.match_current(&moved) || std::time::Instant::now() > deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        rig.app.match_fitted_on,
        rig.app.match_basis(&moved),
        "the fit shown was asked on the Amount moved while one ran"
    );
}

/// **The Match card sits under the Band card** (plan §7.6, decision 34): in the first column, left
/// of Output.
#[test]
fn the_match_card_sits_under_the_band_card() {
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(1_000.0);
    // The cursor's registry is the frame before's.
    rig.press(Vec::new());
    rig.press(Vec::new());
    let spots = mxm_ui::navigation::spots(&rig.ctx);
    let at = |key: &str| {
        spots
            .iter()
            .find(|spot| spot.key == key)
            .map(|spot| spot.rect)
            .unwrap_or_else(|| panic!("{key} is drawn"))
    };
    let (shape, delete) = (at("type_1"), at("delete"));
    let (matching, output) = (at("match to"), at("output"));
    assert!(matching.top() > delete.bottom(), "under the Band card");
    assert!(
        matching.left() < shape.right() && matching.right() > shape.left(),
        "in the same column"
    );
    assert!(matching.right() < output.left(), "left of Output");
}

// ---- Phase modes (plan §7.7) -------------------------------------------------------------------

/// **A phase is asked for in one step, and the curve follows the phase in force** (plan §7.7):
/// while a change waits for the host's restart the drawn curve is the one heard; in Linear phase
/// it is the kernel's own, a narrow low bell smoothed by the resolution; a dynamic band shows no
/// reach there.
#[test]
fn the_curve_follows_the_phase_in_force() {
    use mxm_fx_eq_n_dsp::linear::{Heard, Sizes, heard_lines};
    use mxm_fx_eq_n_dsp::{Phase, Resolution};
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.insert_at(60.0);
    rig.press(Vec::new());
    let slot = rig.app.selected.expect("a band");
    let index = |field| band_index(slot, field);
    let writes = [
        (
            index(Field::Gain),
            rig.app.normalised(index(Field::Gain), 9.0),
        ),
        (index(Field::Q), rig.app.normalised(index(Field::Q), 8.0)),
    ];
    rig.app.instant(&writes);
    rig.press(Vec::new());
    let curve_at = |rig: &Rig| {
        let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
        graph::Curves::compute(&rig.app, &axes).composite_at(60.0)
    };
    let zero = curve_at(&rig);
    assert!((zero - 9.0).abs() < 0.1, "{zero}");

    let depth = rig.depth();
    rig.app.set_phase_value(crate::params::PHASE_INDEX, 1);
    rig.app.set_phase_value(crate::params::RESOLUTION_INDEX, 0);
    rig.press(Vec::new());
    assert_eq!(rig.depth(), depth + 2, "each one undo step");
    assert_eq!(rig.app.phase_asked(), Phase::Linear(Resolution::Low));
    assert_eq!(
        curve_at(&rig),
        zero,
        "the phase in force is drawn while the change waits"
    );

    rig.telemetry.publish_phase(Phase::Linear(Resolution::Low));
    rig.press(Vec::new());
    let linear = curve_at(&rig);
    assert!(linear < zero - 1.0, "smoothed by Low: {linear}");
    let band = rig.band(slot);
    let mut heard = Heard::EMPTY;
    heard.bands[slot] = Some((
        mxm_fx_eq_n_dsp::design(
            mxm_fx_eq_n_dsp::Design {
                shape: Shape::Bell,
                slope: band.slope,
                freq: f64::from(band.freq),
                gain_db: f64::from(band.gain_db),
                q: f64::from(band.q),
            },
            48_000.0,
        ),
        mxm_fx_eq_n_dsp::Placement::Stereo,
    ));
    let [kernel, ..] = heard_lines(
        &heard,
        60.0,
        48_000.0,
        Sizes::new(48_000.0, Resolution::Low),
    );
    assert!((linear - (20.0 * kernel.log10()) as f32).abs() < 1.0e-3);

    let writes = [(
        dynamics_index(slot, DynamicsField::Range),
        rig.app
            .normalised(dynamics_index(slot, DynamicsField::Range), -12.0),
    )];
    rig.app.instant(&writes);
    rig.press(Vec::new());
    assert_eq!(rig.app.range_reach_db(slot), None, "held in Linear phase");
}

/// **The phase hovers tell the delay in force** (plan §7.7): while a change waits for the host's
/// restart, the sound is as late as the phase in force makes it, and the hover says what it then
/// becomes. At 48 kHz Low is 2,304 frames, 48 ms, and High 8,448, 176 ms.
#[test]
fn the_phase_hover_tells_the_delay_in_force() {
    use mxm_fx_eq_n_dsp::{Phase, Resolution};
    let low = Phase::Linear(Resolution::Low);
    let high = Phase::Linear(Resolution::High);
    let hover = |asked, in_force| column::phase_hover(true, asked, in_force, 48_000.0);
    let settled = hover(low, low);
    assert!(
        settled.contains("Now the sound is 48 ms late") && !settled.contains("restarts"),
        "{settled}"
    );
    let finer = hover(high, low);
    assert!(
        finer.contains("Now the sound is 48 ms late") && finer.contains("Then it is 176 ms late"),
        "{finer}"
    );
    let back = hover(Phase::ZeroLatency, low);
    assert!(
        back.contains("Now the sound is 48 ms late") && back.contains("Then the sound is on time"),
        "{back}"
    );
    let on = hover(low, Phase::ZeroLatency);
    assert!(
        !on.contains("Now the sound") && on.contains("Then it is 48 ms late"),
        "{on}"
    );
    let resolution = column::phase_hover(false, high, low, 48_000.0);
    assert!(
        resolution.contains("48 ms late") && resolution.contains("176 ms late"),
        "{resolution}"
    );
}

/// One frame of shared noise: into this editor's analyser, and into the other track's share tap
/// when `other` is given.
fn collide_frame(
    rig: &mut Rig,
    own: &mut (crate::analyser::Tap, rtrb::Consumer<crate::analyser::Block>),
    other: Option<&mut crate::analyser::Tap>,
    seed: &mut u32,
) {
    let frames: Vec<f32> = (0..800)
        .map(|_| {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 17;
            *seed ^= *seed << 5;
            (*seed as f32 / u32::MAX as f32 - 0.5) * 0.5
        })
        .collect();
    own.0
        .capture(2, [&frames, &frames], [&frames, &frames], None);
    if let Some(tap) = other {
        tap.capture_post(2, [&frames, &frames]);
    }
    while let Ok(block) = own.1.pop() {
        rig.app.spectrum.ingest(&block);
    }
    rig.press(Vec::new());
}

/// **Another track, watched and colliding** (plan §7.8): the External selector lists it by its
/// track's name; chosen, it is watched while drawn; Collisions marks nothing on silence, marks
/// where both carry sound at one level, and nothing once the other goes quiet past the analyser's
/// staleness; a closed window drops the watch; a track gone is shown as not found.
#[test]
fn another_track_is_listed_watched_and_collides() {
    use crate::instances::{IdentityField, Instance, SHARE_BLOCKS};
    let identity = IdentityField::default();
    let (mut tap, consumer) = crate::analyser::ring_with(SHARE_BLOCKS);
    let other = Instance::join(identity.shared(), consumer);
    other.set_track("zz-collide Bass");
    let mut rig = Rig::new();
    let names = rig.app.external_names();
    assert_eq!(names[0], "Off");
    assert!(names.contains(&"zz-collide Bass".to_owned()), "{names:?}");
    rig.app.set_view(crate::view::ViewSettings {
        external: other.id(),
        collisions: true,
        ..rig.app.params.view.get()
    });
    rig.press(Vec::new());
    assert!(other.watched(), "chosen and drawn, it is watched");
    assert!(rig.app.collision_marks.iter().all(|s| *s == 0.0), "silence");

    let mut own = crate::analyser::ring();
    own.0.set_wanted(true);
    tap.set_wanted(true);
    let mut seed = 7;
    for _ in 0..40 {
        collide_frame(&mut rig, &mut own, Some(&mut tap), &mut seed);
    }
    assert!(
        rig.app.collision_marks.iter().any(|s| *s > 0.5),
        "both carry sound at one level"
    );
    let on = rig.app.params.view.get();
    rig.app.set_view(crate::view::ViewSettings {
        analyser: false,
        ..on
    });
    rig.press(Vec::new());
    assert!(
        rig.app.collision_marks.is_empty() && rig.app.external_name.is_empty(),
        "the analyser off leaves nothing under the hover"
    );
    rig.app.set_view(on);
    for _ in 0..40 {
        collide_frame(&mut rig, &mut own, None, &mut seed);
    }
    assert!(
        rig.app.collision_marks.iter().all(|s| *s == 0.0),
        "the other went quiet"
    );

    mxm_editor::Panel::closed(&mut rig.app, &GuiContext::new(rig.host.clone()));
    assert!(!other.watched(), "a closed window watches nothing");
    drop(other);
    rig.press(Vec::new());
    assert!(rig.app.other.is_none());
    assert!(
        rig.app
            .external_names()
            .contains(&"Track not found".to_owned())
    );
}

/// **The host's track name** reaches the registry through the editor, whether or not a window is
/// open, and the editor never lists its own instance (plan §7.8).
#[test]
fn the_hosts_track_name_names_this_instance() {
    use crate::instances::{IdentityField, Instance, names};
    let identity = IdentityField::default();
    let (_tap, consumer) = crate::analyser::ring_with(1);
    let mut rig = Rig::new();
    rig.app.instance = Instance::join(identity.shared(), consumer);
    mxm_editor::Panel::track_info_changed(
        &mut rig.app,
        &nice_plug::prelude::TrackInfo::new("zz-info Keys", None),
    );
    assert!(
        names()
            .iter()
            .any(|listed| listed.id == identity.get() && listed.name == "zz-info Keys")
    );
    assert!(
        !rig.app
            .external_names()
            .contains(&"zz-info Keys".to_owned())
    );
}

/// **Auto gain's hover** gives the make-up in force while it is on (plan §7.8).
#[test]
fn the_auto_gain_hover_gives_the_make_up() {
    assert!(column::auto_gain_hover(true, -3.04).ends_with("Now -3.0 dB."));
    assert!(!column::auto_gain_hover(false, 0.0).contains("Now"));
}

// ---- several bands (plan §7.9) ------------------------------------------------------------

const SELECT: Job = Job::Verb(mxm_keys::Verb::Select);

/// Bells at `bands` (frequency, gain), each in the lowest free slot, the selection then none.
fn with_bells(rig: &mut Rig, bands: &[(f32, f32)]) -> Vec<usize> {
    rig.press(Vec::new());
    let slots: Vec<usize> = bands
        .iter()
        .map(|&(freq, gain)| {
            let slot = rig.app.insert_at(freq).expect("a free slot");
            let index = band_index(slot, Field::Gain);
            let value = rig.app.normalised(index, gain);
            rig.app.instant(&[(index, value)]);
            slot
        })
        .collect();
    rig.app.select_none();
    rig.press(Vec::new());
    slots
}

/// A pointer event with `modifiers` held, as a host's window sends one.
fn pointer_with(
    at: egui::Pos2,
    pressed: Option<bool>,
    modifiers: egui::Modifiers,
) -> Vec<egui::Event> {
    let mut events = vec![
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::PointerMoved(at),
    ];
    if let Some(pressed) = pressed {
        events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    events
}

/// The clipboard is the process's (decision 46), so the tests that use it take turns, each from
/// an empty one.
fn clipboard_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let turn = TURN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    super::selection::clear_clipboard();
    turn
}

fn point_of(rig: &Rig, slot: usize) -> egui::Pos2 {
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    rig.app.point_position(&axes, &rig.band(slot))
}

/// A drag from `from` to `to` in eight steps, then let go.
fn drag_between(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    rig.press(pointer(from, Some(true)));
    for step in 1..=8 {
        rig.press(pointer(from + (to - from) * (step as f32 / 8.0), None));
    }
    rig.press(pointer(to, Some(false)));
}

/// **The selection by the mouse** (plan §7.9, decision 44): a click selects one point alone;
/// Shift-click adds one, the primary moving to it, and takes one out, the primary the nearest
/// left, writing nothing; a rectangle dragged on the background selects what it takes in; a click
/// on the background selects none.
#[test]
fn the_mouse_selects_several_bands() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(100.0, 3.0), (400.0, -3.0), (1_600.0, 6.0)]);
    let depth = rig.depth();
    let shift = egui::Modifiers::SHIFT;
    rig.press(pointer(point_of(&rig, slots[0]), Some(true)));
    rig.press(pointer(point_of(&rig, slots[0]), Some(false)));
    assert_eq!(rig.app.selection(), vec![slots[0]]);
    rig.press(pointer_with(point_of(&rig, slots[2]), Some(true), shift));
    rig.press(pointer_with(point_of(&rig, slots[2]), Some(false), shift));
    assert_eq!(rig.app.selection(), vec![slots[0], slots[2]]);
    assert_eq!(
        rig.app.selected,
        Some(slots[2]),
        "the primary moves to the one added"
    );
    rig.press(pointer_with(point_of(&rig, slots[2]), Some(true), shift));
    rig.press(pointer_with(point_of(&rig, slots[2]), Some(false), shift));
    assert_eq!(rig.app.selection(), vec![slots[0]]);
    assert_eq!(rig.app.selected, Some(slots[0]), "the nearest left");
    assert_eq!(rig.depth(), depth, "selecting writes nothing");

    // A rectangle from the background's top left past the two lower points.
    rig.press(pointer_with(rig.app.graph_rect.center(), None, NONE));
    let corner = rig.app.graph_rect.left_top() + egui::vec2(4.0, 4.0);
    let past = egui::pos2(
        point_of(&rig, slots[1]).x + 6.0,
        rig.app.graph_rect.bottom() - 4.0,
    );
    drag_between(&mut rig, corner, past);
    assert_eq!(rig.app.selection(), vec![slots[0], slots[1]]);
    assert_eq!(
        rig.app.selected,
        Some(slots[0]),
        "the primary kept, still selected"
    );
    assert_eq!(rig.depth(), depth);

    // A click on the background: none.
    let empty = egui::pos2(
        rig.app.graph_rect.right() - 10.0,
        rig.app.graph_rect.top() + 10.0,
    );
    rig.press(pointer(empty, Some(true)));
    rig.press(pointer(empty, Some(false)));
    assert!(rig.app.selection().is_empty());
}

/// **The selection by the keyboard** (plan §7.9): SELECT with → and ← adds the next band along
/// the axis not selected yet, the primary moving to it, passing over the selected; from the
/// keyboard's place with none selected; nothing past the end. Ctrl/Cmd+A selects every band.
#[test]
fn the_keyboard_selects_several_bands() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(100.0, 3.0), (400.0, -3.0), (1_600.0, 6.0)]);
    rig.app.focus_freq = 50.0;
    rig.hold(&[SELECT], &[Key::ArrowRight]);
    assert_eq!(
        rig.app.selection(),
        vec![slots[0]],
        "from the keyboard's place"
    );
    rig.hold(&[SELECT], &[Key::ArrowRight, Key::ArrowRight]);
    assert_eq!(rig.app.selection(), slots);
    assert_eq!(rig.app.selected, Some(slots[2]));
    rig.hold(&[SELECT], &[Key::ArrowRight]);
    assert_eq!(rig.app.selection(), slots, "nothing past the end");
    rig.app.select(slots[1]);
    rig.hold(&[SELECT], &[Key::ArrowLeft]);
    rig.hold(&[SELECT], &[Key::ArrowRight]);
    assert_eq!(
        rig.app.selection(),
        slots,
        "back the other way past the selected"
    );
    rig.app.select_none();
    rig.press(vec![key(Key::A, true, egui::Modifiers::COMMAND)]);
    rig.press(Vec::new());
    assert_eq!(rig.app.selection(), slots, "Ctrl/Cmd+A");
}

/// **A group moves with its primary** (plan §7.9, decision 45): a drag moves every frequency by
/// one ratio and every gain by one offset, one undo step; at a member's limit the whole group
/// stops; a cut in the group keeps its gain.
#[test]
fn a_group_moves_by_one_ratio_and_one_offset() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    let cut = rig.app.insert_at(3_000.0).unwrap();
    rig.app.set_shape(cut, Shape::HighCut);
    rig.app
        .select_set(&[slots[0], slots[1], cut], Some(slots[0]));
    rig.press(Vec::new());
    let depth = rig.depth();
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let from = point_of(&rig, slots[0]);
    drag_between(&mut rig, from, egui::pos2(axes.x(400.0), axes.y(6.0)));
    let (a, b, c) = (rig.band(slots[0]), rig.band(slots[1]), rig.band(cut));
    assert!((a.freq / 400.0 - 1.0).abs() < 0.03, "{}", a.freq);
    assert!(
        (b.freq / a.freq / 4.0 - 1.0).abs() < 0.01,
        "one ratio: {}",
        b.freq
    );
    assert!(
        (c.freq / a.freq / 15.0 - 1.0).abs() < 0.01,
        "one ratio: {}",
        c.freq
    );
    assert!(((b.gain_db - a.gain_db) + 9.0).abs() < 0.05, "one offset");
    assert_eq!(c.gain_db, 0.0, "a cut has no gain to move");
    assert_eq!(rig.depth(), depth + 1, "one undo step");
    assert!(rig.gestures_balanced());

    // Up past +30 dB: the upper band reaches it first, and the group stops there together.
    let from = point_of(&rig, slots[1]);
    drag_between(&mut rig, from, egui::pos2(from.x, axes.y(30.0)));
    let (a, b) = (rig.band(slots[0]), rig.band(slots[1]));
    assert!((a.gain_db - 30.0).abs() < 0.05, "{}", a.gain_db);
    assert!(
        ((b.gain_db - a.gain_db) + 9.0).abs() < 0.05,
        "kept apart: {}",
        b.gain_db
    );
}

/// **The keys and the card move the group too** (plan §7.9): EXTENT's Q by one factor on the
/// bands with a Q, a 6 dB/oct cut keeping its own; a shape from the card set on every band, one
/// undo step.
#[test]
fn the_keys_and_the_card_move_the_group() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    let cut = rig.app.insert_at(3_000.0).unwrap();
    rig.app.set_shape(cut, Shape::HighCut);
    rig.app.set_slope(cut, Slope::Db6);
    rig.app
        .select_set(&[slots[0], slots[1], cut], Some(slots[0]));
    rig.press(Vec::new());
    let (q0, q1, qc) = (rig.band(slots[0]).q, rig.band(slots[1]).q, rig.band(cut).q);
    let extent = Job::Verb(mxm_keys::Verb::Extent);
    rig.hold(&[extent], &[Key::ArrowLeft]);
    let ratio = rig.band(slots[0]).q / q0;
    assert!(ratio > 1.01, "narrower: {ratio}");
    assert!((rig.band(slots[1]).q / q1 / ratio - 1.0).abs() < 1e-3);
    assert_eq!(rig.band(cut).q, qc, "no Q on a 6 dB/oct cut");

    let depth = rig.depth();
    rig.app
        .group_edit(super::selection::Writer::Instant, |app| {
            app.set_shape(slots[0], Shape::LowShelf);
        });
    rig.press(Vec::new());
    for slot in [slots[0], slots[1], cut] {
        assert_eq!(rig.band(slot).shape, Some(Shape::LowShelf), "{slot}");
    }
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **DELETE and DUPLICATE take the whole selection** (plan §7.9): DUPLICATE copies the bands in
/// slot order onto free slots in theirs, the copy of the primary primary; DELETE turns every
/// selected band Off in one undo step; with too few free slots DUPLICATE copies nothing.
#[test]
fn delete_and_duplicate_take_the_whole_selection() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0), (3_000.0, 2.0)]);
    rig.app.select_set(&[slots[0], slots[2]], Some(slots[2]));
    let duplicate = Job::Verb(mxm_keys::Verb::Duplicate);
    rig.hold(&[duplicate], &[Key::ArrowUp]);
    assert_eq!(rig.app.selection().len(), 2);
    assert_eq!(
        rig.app.selected,
        Some(4),
        "the copy of the primary, in the second free slot"
    );
    assert_eq!(rig.band(3).freq, rig.band(slots[0]).freq);
    assert!((rig.band(4).gain_db - rig.band(slots[2]).gain_db - 0.5).abs() < 0.05);

    let depth = rig.depth();
    rig.type_keys(&[job_key(DELETE)]);
    assert!(rig.band(3).shape.is_none() && rig.band(4).shape.is_none());
    assert!(rig.band(slots[0]).shape.is_some(), "the originals stay");
    assert_eq!(rig.depth(), depth + 1);

    // One free slot for two bands: refused whole.
    while rig.app.free_slot_exists() {
        rig.app.insert_at(5_000.0);
    }
    rig.app.delete(23);
    rig.press(Vec::new());
    rig.app.select_set(&[slots[0], slots[1]], Some(slots[0]));
    let depth = rig.depth();
    rig.hold(&[duplicate], &[Key::ArrowUp]);
    assert_eq!(rig.app.selection(), vec![slots[0], slots[1]]);
    assert_eq!(rig.depth(), depth, "nothing copied, no undo step");
}

/// **Reset empties the EQ in one undo step** (the owner, 2026-10-29): every band Off and the
/// output back to Init's, which Undo takes back whole; at Init it has nothing to write.
#[test]
fn reset_empties_the_eq_in_one_undo_step() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0), (3_000.0, 2.0)]);
    rig.app.select_set(&[slots[0], slots[2]], Some(slots[2]));
    let output = rig.app.params.param(OUTPUT_INDEX).default_normalised();
    rig.app.instant(&[(OUTPUT_INDEX, 0.7)]);
    rig.press(Vec::new());
    let depth = rig.depth();
    rig.app.reset();
    rig.press(Vec::new());
    assert!((0..SLOTS).all(|slot| rig.band(slot).shape.is_none()));
    assert_eq!(rig.app.normalised_now(OUTPUT_INDEX), output);
    assert_eq!(rig.depth(), depth + 1, "one undo step");
    assert!(
        rig.app.reset_writes().is_empty(),
        "at Init, nothing to write"
    );

    rig.app.undo();
    rig.press(Vec::new());
    assert!(slots.iter().all(|&slot| rig.band(slot).shape.is_some()));
    assert_eq!(rig.app.normalised_now(OUTPUT_INDEX), 0.7);
}

/// **Copy, cut and paste** (plan §7.9, decision 46): every field copied, pasted in slot order onto
/// the free slots in theirs, the copy of the primary primary, one undo step, from one editor into
/// another in the process; refused whole when they do not fit, the selection and the history as
/// they were; nothing while a value is being typed; Cut copies and deletes.
#[test]
fn copy_cut_and_paste_carry_every_field() {
    let _turn = clipboard_turn();
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    rig.app
        .set_placement(slots[1], mxm_fx_eq_n_dsp::Placement::Mid);
    rig.app.select_set(&slots, Some(slots[1]));
    rig.press(vec![key(Key::C, true, egui::Modifiers::COMMAND)]);
    rig.press(Vec::new());

    let mut other = Rig::new();
    with_bells(&mut other, &[(5_000.0, 1.0)]);
    let depth = other.depth();
    other.press(vec![key(Key::V, true, egui::Modifiers::COMMAND)]);
    other.press(Vec::new());
    assert_eq!(other.depth(), depth + 1, "one undo step");
    assert_eq!(other.app.selection(), vec![1, 2]);
    assert_eq!(other.app.selected, Some(2), "the copy of the primary");
    for (from, to) in [(slots[0], 1), (slots[1], 2)] {
        for field in 0..BAND_FIELDS.len() {
            let (a, b) = (
                band_index(from, Field::Type) + field,
                band_index(to, Field::Type) + field,
            );
            assert_eq!(
                rig.app.normalised_now(a),
                other.app.normalised_now(b),
                "field {field} of band {from}"
            );
        }
        assert_eq!(rig.app.placement_now(from), other.app.placement_now(to));
    }

    // Refused whole when they do not fit.
    while other.app.free_slot_exists() {
        other.app.insert_at(9_000.0);
    }
    other.app.delete(23);
    other.app.select(0);
    other.press(Vec::new());
    let depth = other.depth();
    assert_eq!(other.app.paste(), Err(super::selection::PasteRefusal::Full));
    assert_eq!((other.app.selection(), other.depth()), (vec![0], depth));

    // While a value is typed the shortcuts are the text field's.
    rig.app.text.insert("freq", Some("12".to_owned()));
    let selection = rig.app.selection();
    rig.press(vec![key(Key::X, true, egui::Modifiers::COMMAND)]);
    rig.app.text.insert("freq", None);
    rig.press(Vec::new());
    assert_eq!(rig.app.selection(), selection, "typing: nothing cut");
    rig.press(vec![key(Key::X, true, egui::Modifiers::COMMAND)]);
    rig.press(Vec::new());
    assert!(rig.band(slots[0]).shape.is_none() && rig.band(slots[1]).shape.is_none());
}

/// **Copies from two threads at once** never mix: the clipboard holds one of them whole.
#[test]
fn the_clipboard_takes_one_copy_whole_from_two_threads() {
    let _turn = clipboard_turn();
    let copies: Vec<Vec<f32>> = std::thread::scope(|scope| {
        let handles: Vec<_> = [(200.0f32, 2usize), (900.0, 3)]
            .into_iter()
            .map(|(freq, count)| {
                scope.spawn(move || {
                    let mut rig = Rig::new();
                    let bands: Vec<(f32, f32)> =
                        (0..count).map(|n| (freq * (n + 1) as f32, 1.0)).collect();
                    let slots = with_bells(&mut rig, &bands);
                    rig.app.select_set(&slots, None);
                    rig.app.copy_selection();
                    slots
                        .iter()
                        .map(|&slot| rig.band(slot).freq)
                        .collect::<Vec<f32>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.app.paste().unwrap();
    rig.press(Vec::new());
    let pasted: Vec<f32> = rig
        .app
        .selection()
        .iter()
        .map(|&slot| rig.band(slot).freq)
        .collect();
    // Whole: one copy's bands, all of them, never a mix of the two (plain values recomputed from
    // the normalised ones, so within rounding).
    let same = |copy: &Vec<f32>| {
        copy.len() == pasted.len()
            && copy
                .iter()
                .zip(&pasted)
                .all(|(a, b)| (a / b - 1.0).abs() < 1e-4)
    };
    assert!(copies.iter().any(same), "{pasted:?} against {copies:?}");
}

/// **Invert gain** turns every selected gain over in one undo step; a right-click on a selected
/// point makes it the primary of the selection; on one not selected, it selects that one alone,
/// so its Delete touches nothing else.
#[test]
fn invert_gain_and_the_point_s_menu_act_on_the_selection() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0), (3_000.0, 2.0)]);
    rig.app.select_set(&[slots[0], slots[1]], Some(slots[0]));
    let depth = rig.depth();
    rig.app.invert_selection();
    rig.press(Vec::new());
    assert_eq!(
        (rig.band(slots[0]).gain_db, rig.band(slots[1]).gain_db),
        (-3.0, 6.0)
    );
    assert_eq!(rig.band(slots[2]).gain_db, 2.0);
    assert_eq!(rig.depth(), depth + 1);

    let right_click = |rig: &mut Rig, at: egui::Pos2| {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: NONE,
        };
        rig.press(vec![egui::Event::PointerMoved(at), button(true)]);
        rig.press(vec![button(false)]);
    };
    let at = point_of(&rig, slots[1]);
    right_click(&mut rig, at);
    assert_eq!(rig.app.selection(), vec![slots[0], slots[1]]);
    assert_eq!(
        rig.app.selected,
        Some(slots[1]),
        "the point right-clicked is the primary"
    );
    let at = point_of(&rig, slots[2]);
    right_click(&mut rig, at);
    assert_eq!(rig.app.selection(), vec![slots[2]], "selected alone first");
    rig.app.delete_selection();
    rig.press(Vec::new());
    assert!(rig.band(slots[2]).shape.is_none());
    assert!(rig.band(slots[0]).shape.is_some() && rig.band(slots[1]).shape.is_some());
}

/// **The title counts the selection** (plan §7.9).
#[test]
fn the_title_counts_the_selection() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0), (3_000.0, 2.0)]);
    rig.app.select_set(&slots, Some(slots[1]));
    assert_eq!(
        column::band_title(&rig.app),
        format!("Band {} + 2", slots[1] + 1)
    );
    rig.app.select(slots[0]);
    assert_eq!(
        column::band_title(&rig.app),
        format!("Band {}", slots[0] + 1)
    );
}

/// **A rectangle with Shift adds once** (plan §7.9): overlapping a band already selected, it is
/// not counted twice.
#[test]
fn an_additive_rectangle_counts_each_band_once() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(100.0, 3.0), (400.0, -3.0), (1_600.0, 6.0)]);
    rig.app.select_set(&[slots[0], slots[2]], Some(slots[0]));
    let corner = rig.app.graph_rect.left_top() + egui::vec2(4.0, 4.0);
    let past = egui::pos2(
        point_of(&rig, slots[2]).x + 6.0,
        rig.app.graph_rect.bottom() - 4.0,
    );
    let shift = egui::Modifiers::SHIFT;
    rig.press(pointer_with(corner, Some(true), shift));
    for step in 1..=6 {
        rig.press(pointer_with(
            corner + (past - corner) * (step as f32 / 6.0),
            None,
            shift,
        ));
    }
    rig.press(pointer_with(past, Some(false), shift));
    assert_eq!(rig.app.selection(), slots);
    assert_eq!(rig.app.others_selected().len(), 2, "no band twice");
    assert_eq!(
        column::band_title(&rig.app),
        format!("Band {} + 2", slots[0] + 1)
    );
}

/// **A group reads frequencies as they run** (plan §7.9): below a low rate's top a band stored
/// above it is at the top, so the group cannot go up, nothing moves the other way, and the
/// stored band is not rewritten; down, both move by one ratio from where they run.
#[test]
fn a_group_reads_frequencies_as_they_run() {
    let mut rig = Rig::new();
    rig.app.telemetry.publish_sample_rate(22_050.0);
    let slots = with_bells(&mut rig, &[(1_000.0, 3.0)]);
    let high = rig.app.insert_at(2_000.0).unwrap();
    let index = band_index(high, Field::Freq);
    let above = rig.app.normalised(index, 30_000.0);
    rig.app.instant(&[(index, above)]);
    rig.app.select_set(&[slots[0], high], Some(slots[0]));
    rig.press(Vec::new());
    let top = rig.app.high();
    assert!(top < 30_000.0);
    rig.hold(&[MOVE], &[Key::ArrowRight]);
    assert!(
        (rig.band(slots[0]).freq / 1_000.0 - 1.0).abs() < 1e-3,
        "{}",
        rig.band(slots[0]).freq
    );
    assert_eq!(rig.app.normalised_now(index), above, "not rewritten");
    rig.hold(&[MOVE], &[Key::ArrowLeft]);
    let ratio = rig.band(slots[0]).freq / 1_000.0;
    assert!(ratio < 0.99, "{ratio}");
    assert!(
        (rig.band(high).freq / top / ratio - 1.0).abs() < 0.01,
        "{}",
        rig.band(high).freq
    );
}

/// **The wheel, the dynamics and a typed value move the group** (plan §7.9): the wheel's Q by one
/// factor; Range by one offset and Attack by one factor within their ranges; a value typed into
/// the card, entered, moves the others in the same undo step.
#[test]
fn the_wheel_the_dynamics_and_a_typed_value_move_the_group() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    rig.app.select_set(&slots, Some(slots[0]));
    rig.press(Vec::new());
    let (q0, q1) = (rig.band(slots[0]).q, rig.band(slots[1]).q);
    let at = point_of(&rig, slots[0]);
    let alt = egui::Modifiers::ALT;
    rig.press(vec![
        egui::Event::ModifiersChanged(alt),
        egui::Event::PointerMoved(at),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, 1.0),
            phase: egui::TouchPhase::Move,
            modifiers: alt,
        },
    ]);
    rig.press(vec![egui::Event::ModifiersChanged(NONE)]);
    let ratio = rig.band(slots[0]).q / q0;
    assert!((ratio - 1.0).abs() > 0.01, "{ratio}");
    assert!((rig.band(slots[1]).q / q1 / ratio - 1.0).abs() < 1e-3);
    assert_eq!(rig.app.selection(), slots, "the wheel keeps the group");
    // The wheel's gesture ends after a pause; here at once.
    rig.app.close_open();
    rig.press(Vec::new());

    // The card's dynamics, as their sliders write: Range by an offset, Attack by a factor.
    let range = |slot| dynamics_index(slot, DynamicsField::Range);
    let attack = |slot| dynamics_index(slot, DynamicsField::Attack);
    let depth = rig.depth();
    let (r1, a1) = (
        rig.app.current_plain(range(slots[1])),
        rig.app.current_plain(attack(slots[1])),
    );
    rig.app
        .group_edit(super::selection::Writer::Instant, |app| {
            let writes = [
                (range(slots[0]), app.normalised(range(slots[0]), -6.0)),
                (attack(slots[0]), app.normalised(attack(slots[0]), 20.0)),
            ];
            app.instant(&writes);
        });
    rig.press(Vec::new());
    let (r0, a0) = (-6.0, 20.0);
    let range_before = 0.0;
    assert!((rig.app.current_plain(range(slots[1])) - (r1 + (r0 - range_before))).abs() < 0.01);
    let attack_default = rig.app.params.dynamics[slots[0]]
        .attack
        .default_plain_value();
    let factor = a0 / attack_default;
    assert!((rig.app.current_plain(attack(slots[1])) / a1 / factor - 1.0).abs() < 0.01);
    assert_eq!(rig.depth(), depth + 1, "one undo step");

    // A typed frequency, entered, as the card's typed entry writes it.
    let depth = rig.depth();
    let freq = |slot| band_index(slot, Field::Freq);
    let (f0, f1) = (rig.band(slots[0]).freq, rig.band(slots[1]).freq);
    rig.app
        .group_edit(super::selection::Writer::Instant, |app| {
            let value = app.normalised(freq(slots[0]), f0 * 2.0);
            app.instant(&[(freq(slots[0]), value)]);
        });
    rig.press(Vec::new());
    assert!((rig.band(slots[1]).freq / f1 / 2.0 - 1.0).abs() < 0.01);
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **A point's menu offers the selection's items** (plan §7.9): Copy, Cut, Paste, Invert gain and
/// Delete, Paste disabled with nothing copied.
#[test]
fn a_point_s_menu_offers_the_selection_s_items() {
    let _turn = clipboard_turn();
    let mut rig = Rig::new();
    rig.ctx.enable_accesskit();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    rig.app.select_set(&slots, Some(slots[0]));
    rig.type_keys(&[job_key(MENU)]);
    let output = rig.frame(Rig::opening(), Vec::new());
    let update = output.platform_output.accesskit_update.expect("an update");
    let labelled = |item: &str| {
        update
            .nodes
            .iter()
            .find(|(_, node)| node.label().is_some_and(|label| label == item))
            .map(|(_, node)| node.is_disabled())
    };
    for item in ["Copy", "Cut", "Invert gain", "Delete"] {
        assert_eq!(labelled(item), Some(false), "{item}");
    }
    assert_eq!(labelled("Paste"), Some(true), "nothing copied");
}

/// The texts and the outlined circles a frame painted.
fn painted(output: &egui::FullOutput) -> (Vec<String>, Vec<(f32, f32)>) {
    let mut texts = Vec::new();
    let mut rings = Vec::new();
    for clipped in &output.shapes {
        match &clipped.shape {
            egui::epaint::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
            egui::epaint::Shape::Circle(circle)
                if circle.fill == egui::Color32::TRANSPARENT && circle.stroke.width > 0.0 =>
            {
                rings.push((circle.radius, circle.stroke.width));
            }
            _ => {}
        }
    }
    (texts, rings)
}

/// **The card's own controls move the group** (plan §7.9): its Threshold slider, operated by the
/// cursor as any control is (VALUE, ↑, OUT), moves the other band's Threshold by the same offset;
/// its Bypass toggle, an instant write, bypasses both; each one undo step.
#[test]
fn the_card_s_own_controls_move_the_group() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    rig.app.select_set(&slots, Some(slots[0]));
    rig.press(Vec::new());
    rig.view(Key::ArrowRight);
    if rig.app.nav.card() != Some(BAND_CARD) {
        rig.view(Key::ArrowLeft);
    }
    rig.up_to_the_band_card();
    let reach = |rig: &mut Rig, target: &str, arrow: Key| {
        let mut seen = Vec::new();
        for _ in 0..16 {
            if rig.app.nav.parameter() == Some(target) {
                return;
            }
            seen.push(rig.app.nav.parameter().map(str::to_owned));
            rig.type_keys(&[arrow]);
        }
        panic!("the arrows never reach {target}: {seen:?}");
    };
    let range = |slot| dynamics_index(slot, DynamicsField::Threshold);
    reach(&mut rig, "threshold_1", Key::ArrowDown);
    let (before0, before1) = (
        rig.app.current_plain(range(slots[0])),
        rig.app.current_plain(range(slots[1])),
    );
    let depth = rig.depth();
    let value = Job::Verb(mxm_keys::Verb::Value);
    rig.type_keys(&[job_key(value), Key::ArrowUp, job_key(Job::Out)]);
    let moved = rig.app.current_plain(range(slots[0])) - before0;
    assert!(moved.abs() > 0.01, "the slider moved: {moved}");
    let followed = rig.app.current_plain(range(slots[1])) - before1;
    assert!(
        (followed - moved).abs() < 0.01,
        "{followed} against {moved}"
    );
    assert_eq!(rig.depth(), depth + 1, "one undo step");

    // Bypass shares its row with Placement: up to the row, then ← along it.
    reach(&mut rig, "place_1", Key::ArrowUp);
    reach(&mut rig, "bypass_1", Key::ArrowLeft);
    let depth = rig.depth();
    rig.type_keys(&[Key::Enter]);
    rig.press(Vec::new());
    assert!(rig.band(slots[0]).bypass && rig.band(slots[1]).bypass);
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **What a selection paints** (plan §7.9): a ring round every selected point, the primary's
/// twice as thick; the rectangle while it is dragged; the title's hover naming the bands.
#[test]
fn a_selection_paints_its_rings_its_rectangle_and_its_names() {
    let ring = graph::POINT_RADIUS + 4.0;
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0), (3_000.0, 2.0)]);
    rig.app.select_set(&[slots[0], slots[2]], Some(slots[2]));
    let output = rig.frame(Rig::opening(), Vec::new());
    let (_, rings) = painted(&output);
    let mut widths: Vec<f32> = rings
        .iter()
        .filter(|(radius, _)| (radius - ring).abs() < 0.01)
        .map(|&(_, width)| width)
        .collect();
    widths.sort_by(f32::total_cmp);
    assert_eq!(widths.len(), 2, "a ring round each selected point");
    assert!(
        (widths[1] / widths[0] - 2.0).abs() < 1e-3,
        "the primary's twice as thick"
    );

    // The rectangle, dashed, while it is dragged on the background.
    let corner = rig.app.graph_rect.left_top() + egui::vec2(4.0, 4.0);
    let to = corner + egui::vec2(120.0, 90.0);
    rig.press(pointer(corner, Some(true)));
    rig.press(pointer(corner + egui::vec2(60.0, 45.0), None));
    let output = rig.frame(Rig::opening(), pointer(to, None));
    let dashes = output
        .shapes
        .iter()
        .filter(|clipped| match &clipped.shape {
            egui::epaint::Shape::LineSegment { points, .. } => {
                (points[0].y - corner.y).abs() < 0.5 && (points[1].y - corner.y).abs() < 0.5
            }
            _ => false,
        })
        .count();
    assert!(dashes >= 3, "the rectangle's top edge, dashed: {dashes}");
    rig.press(pointer(to, Some(false)));

    // The title's hover names the bands.
    rig.app.select_set(&slots, Some(slots[1]));
    rig.press(Vec::new());
    let names = column::group_names(&rig.app).expect("a group");
    assert!(names.contains(&format!("band {}", slots[1] + 1)), "{names}");
    let title = rig.app.band_title_rect.center();
    rig.press(vec![egui::Event::PointerMoved(title)]);
    let mut shown = false;
    for _ in 0..90 {
        let output = rig.frame(Rig::opening(), Vec::new());
        let (texts, _) = painted(&output);
        if texts.contains(&names) {
            shown = true;
            break;
        }
    }
    assert!(shown, "the title's hover: {names}");
}

/// **A card slider's group stops at a follower's limit without a nested gesture** (plan §7.9):
/// the Threshold slider held by the cursor (VALUE, ↑ five times, OUT) with the other band 1 dB
/// under the top; the group stops there, kept 29 dB apart, the slider's own parameter set inside
/// its gesture and never begun again, one undo step.
#[test]
fn a_card_slider_s_group_stops_at_a_limit_without_nesting() {
    let mut rig = Rig::new();
    let slots = with_bells(&mut rig, &[(200.0, 3.0), (800.0, -6.0)]);
    let threshold = |slot| dynamics_index(slot, DynamicsField::Threshold);
    let writes = [
        (
            threshold(slots[0]),
            rig.app.normalised(threshold(slots[0]), -30.0),
        ),
        (
            threshold(slots[1]),
            rig.app.normalised(threshold(slots[1]), -1.0),
        ),
    ];
    rig.app.instant(&writes);
    rig.app.select_set(&slots, Some(slots[0]));
    rig.press(Vec::new());
    rig.view(Key::ArrowRight);
    if rig.app.nav.card() != Some(BAND_CARD) {
        rig.view(Key::ArrowLeft);
    }
    rig.up_to_the_band_card();
    for _ in 0..16 {
        if rig.app.nav.parameter() == Some("threshold_1") {
            break;
        }
        rig.type_keys(&[Key::ArrowDown]);
    }
    assert_eq!(rig.app.nav.parameter(), Some("threshold_1"));
    let depth = rig.depth();
    let value = Job::Verb(mxm_keys::Verb::Value);
    rig.type_keys(&[
        job_key(value),
        Key::ArrowUp,
        Key::ArrowUp,
        Key::ArrowUp,
        Key::ArrowUp,
        Key::ArrowUp,
        job_key(Job::Out),
    ]);
    let (primary, follower) = (
        rig.app.current_plain(threshold(slots[0])),
        rig.app.current_plain(threshold(slots[1])),
    );
    assert!(
        (follower - 0.0).abs() < 0.01,
        "the follower at the top: {follower}"
    );
    assert!(
        ((follower - primary) - 29.0).abs() < 0.01,
        "kept apart: {primary}"
    );
    assert!(rig.gestures_balanced(), "no gesture begun twice");
    assert_eq!(rig.depth(), depth + 1, "one undo step");
}

/// **Find off holds what it learned; on again learns anew** (plan §7.5): off ends the learn and
/// keeps its proposals, as a noise reduction keeps its profile; on starts a learn from nothing,
/// replacing them.
#[test]
fn find_off_holds_what_it_learned_and_on_again_learns_anew() {
    let mut rig = finding_rig(vec![candidate(
        2_000.0,
        &[(finder::Domain::Average, 10.0, 6.0)],
    )]);
    assert!(rig.app.listening);
    assert_eq!(rig.app.proposals.len(), 1);
    rig.app.set_find(false);
    rig.press(Vec::new());
    assert!(
        rig.app.find && !rig.app.listening,
        "off ends the learn, not Find"
    );
    assert_eq!(rig.app.proposals.len(), 1, "what it learned is held");
    rig.app.set_find(true);
    assert!(rig.app.listening);
    assert!(
        rig.app.proposals.is_empty() && rig.app.candidates.is_empty(),
        "a new learn replaces what was held"
    );
}
