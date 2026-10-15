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
            rig.app.hold_post = Some(true);
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
        (column::ANALYSER_TITLE, |_| column::analyser_tree(true)),
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
/// what the cursor is on), the one a right-click opens; with no band selected it opens none.
#[test]
fn menu_on_the_graph_opens_the_selected_band_s_menu() {
    let menu_shown = |rig: &mut Rig| {
        let output = rig.frame(Rig::opening(), Vec::new());
        output
            .platform_output
            .accesskit_update
            .is_some_and(|update| {
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label().is_some_and(|label| label == "Delete"))
            })
    };
    let mut rig = Rig::new();
    rig.ctx.enable_accesskit();
    rig.press(Vec::new());
    rig.press(Vec::new());
    rig.type_keys(&[job_key(MENU)]);
    assert!(!menu_shown(&mut rig), "no band, no menu");
    rig.type_keys(&[job_key(ADD)]);
    rig.press(Vec::new());
    assert_eq!(rig.app.selected, Some(0));
    rig.type_keys(&[job_key(MENU)]);
    assert!(menu_shown(&mut rig), "the band's menu");
    assert_eq!(rig.app.context_slot, Some(0));
}

/// **The Side chain view is offered only on a layout with the port** (plan §7.4), and turning it
/// on tells the audio thread to copy the key.
#[test]
fn the_side_chain_view_is_offered_only_with_the_port() {
    for (port, offered) in [(false, false), (true, true)] {
        let found: Vec<column::Leaf> = column::analyser_tree(port)
            .keys()
            .into_iter()
            .copied()
            .collect();
        assert_eq!(found.contains(&column::Leaf::SideChainView), offered);
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
    let mut rig = Rig::new();
    rig.press(Vec::new());
    rig.press(Vec::new());
    let axes = graph::Axes::new(rig.app.graph_rect, rig.app.high());
    let freqs = graph::Curves::compute(&rig.app, &axes).freqs;
    let levels: Vec<f32> = freqs
        .iter()
        .map(|&f| {
            let detune = 8.0 * (f / centre - centre / f);
            10.0 * (1.0e-6 + 1.0e-3 / (1.0 + detune * detune)).log10()
        })
        .collect();
    let now = rig.ctx.input(|input| input.time);
    rig.app.hold.push(now, &levels);
    rig.app.hold_post = Some(true);
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
    rig.app.detach();
    assert!(rig.app.frozen.is_none(), "closing the editor ends it");

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
