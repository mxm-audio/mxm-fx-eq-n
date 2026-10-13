//! Undo and redo, plain (plan §5.4; the owner, 2026-10-12: *"simplify undo"*).
//!
//! - **An action is the parameters one gesture changed, before and after.** A gesture runs from
//!   the first `begin` to the last `end` with nothing left open, so a drag that moves frequency and
//!   gain together, or a keyboard gesture's steps from the verb to OUT, is one action.
//! - **Every editor write is seen**, because the editor's host context is a [`Recorder`]: the
//!   column's controls, the graph's drags and the keyboard's commands all write through it. The
//!   shared binding needs no observer.
//! - **Undo writes the "before" values back**, as one gesture; redo writes the "after" values. If
//!   a host automates one of those parameters, its next point puts its own value back, and a host
//!   recording automation records the undo like any other edit. No provenance is tracked.
//! - **Epochs.** A preset load or Init (whatever the preset bar writes) and a host's state restore
//!   replace the whole patch, and clear the history, so an undo never reaches back across one.
//! - **It belongs to the plugin instance** ([`crate::MxmFxEqN`] holds it), so it survives the editor
//!   closing, as `mxm-fx-curve`'s does. Only the GUI thread touches it.

use std::sync::{Arc, Mutex, PoisonError};

use nice_plug::context::PluginApi;
use nice_plug::context::gui::{GuiContext, GuiContextInner};
use nice_plug::params::internals::ParamPtr;
use nice_plug::prelude::PluginState;

/// How many actions undo keeps.
pub const LENGTH: usize = 256;

/// One parameter one action changed, as normalised values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    /// The parameter's place in [`crate::params::EqParams::pointers`].
    pub index: usize,
    pub before: f32,
    pub after: f32,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    /// The gesture under way, and the parameters it has open.
    open: Vec<Change>,
    open_params: Vec<usize>,
    /// Values sent that the host may not have applied yet: the parameter, what it read when the
    /// value was sent, and the value. A gesture that starts before the host catches up starts from
    /// the value sent, not the stale readback.
    sent: Vec<(usize, f32, f32)>,
    /// Inside the preset bar's calls: what is written there replaces the patch.
    replacing: bool,
    replaced: bool,
    /// The host restores this history has seen.
    restores: Option<u64>,
    /// Parameters whose gesture a replacement or a removal ended at the host while a control
    /// still held it: what that control sends for one is dropped until it begins anew.
    abandoned: Vec<usize>,
    /// Of those, the ones the editor has still to end at the host ([`take_to_end`](Self::take_to_end)).
    to_end: Vec<usize>,
}

impl History {
    /// Where a parameter really is, given what it reads: the value this history last sent while
    /// the host has not applied it yet, otherwise the readback.
    fn current(&mut self, index: usize, readback: f32) -> f32 {
        if let Some(at) = self.sent.iter().position(|(sent, ..)| *sent == index) {
            let (_, then, value) = self.sent[at];
            if readback == then && readback != value {
                return value;
            }
            self.sent.remove(at);
        }
        readback
    }

    /// Where a parameter is as this history last sent it, without forgetting anything: the value
    /// sent while the host has not applied it yet, otherwise the readback. nice-plug changes a
    /// parameter only when its queued event is written, at the end of a process cycle or a flush,
    /// so what the editor just wrote reads back unchanged until then.
    pub fn pending(&self, index: usize, readback: f32) -> f32 {
        match self.sent.iter().find(|(sent, ..)| *sent == index) {
            Some(&(_, then, value)) if readback == then && readback != value => value,
            _ => readback,
        }
    }

    /// Once a frame, before anything reads: a value sent whose readback has moved — applied, or
    /// overridden by the host — is no longer pending. Without this, automation returning a
    /// parameter to where it was before the edit would read as the edit still on its way.
    pub fn settle(&mut self, readback: impl Fn(usize) -> f32) {
        self.sent
            .retain(|&(index, then, _)| readback(index) == then);
    }

    fn note_sent(&mut self, index: usize, readback: f32, value: f32) {
        self.sent.retain(|(sent, ..)| *sent != index);
        if readback != value {
            self.sent.push((index, readback, value));
        }
    }

    pub fn begin(&mut self, index: usize, readback: f32) {
        // A new gesture: whatever was abandoned of this parameter is over.
        self.abandoned.retain(|&abandoned| abandoned != index);
        if self.replacing {
            self.replaced = true;
            return;
        }
        if !self.open.iter().any(|change| change.index == index) {
            let before = self.current(index, readback);
            self.open.push(Change {
                index,
                before,
                after: before,
            });
        }
        self.open_params.push(index);
    }

    pub fn set(&mut self, index: usize, normalised: f32, readback: f32) {
        self.note_sent(index, readback, normalised);
        if self.replacing {
            self.replaced = true;
            return;
        }
        match self.open.iter_mut().find(|change| change.index == index) {
            Some(change) => change.after = normalised,
            // A write outside any begin is still an edit; it is its own action.
            None if self.open_params.is_empty() => {
                let before = self.current(index, readback);
                self.push(vec![Change {
                    index,
                    before,
                    after: normalised,
                }]);
            }
            None => {
                let before = self.current(index, readback);
                self.open.push(Change {
                    index,
                    before,
                    after: normalised,
                });
            }
        }
    }

    pub fn end(&mut self, index: usize) {
        if self.replacing {
            return;
        }
        if let Some(at) = self.open_params.iter().position(|open| *open == index) {
            self.open_params.remove(at);
        }
        if self.open_params.is_empty() && !self.open.is_empty() {
            let changes: Vec<Change> = std::mem::take(&mut self.open)
                .into_iter()
                .filter(|change| change.before != change.after)
                .collect();
            if !changes.is_empty() {
                self.push(changes);
            }
        }
    }

    fn push(&mut self, action: Vec<Change>) {
        if self.undo.len() == LENGTH {
            self.undo.remove(0);
        }
        self.undo.push(action);
        self.redo.clear();
    }

    /// Clears everything: the patch was replaced. A gesture still open is abandoned.
    pub fn new_epoch(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.abandon_open();
    }

    /// Every gesture still open ends, unrecorded, whichever control opened it — the graph, the
    /// keyboard or a shared control in the column: the patch it was editing is gone. Its
    /// parameters wait for the editor to end them at the host, and what a control still sends for
    /// one is dropped until it begins anew.
    pub fn abandon_open(&mut self) {
        let mut open = std::mem::take(&mut self.open_params);
        open.sort_unstable();
        open.dedup();
        for index in open {
            if !self.abandoned.contains(&index) {
                self.abandoned.push(index);
                self.to_end.push(index);
            }
        }
        self.open.clear();
    }

    /// The abandoned parameters still to end at the host, once each.
    pub fn take_to_end(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.to_end)
    }

    fn is_abandoned(&self, index: usize) -> bool {
        self.abandoned.contains(&index)
    }

    /// An abandoned parameter's own end arrived: the host had it already.
    fn take_abandoned_end(&mut self, index: usize) -> bool {
        let at = self
            .abandoned
            .iter()
            .position(|&abandoned| abandoned == index);
        if let Some(at) = at {
            self.abandoned.remove(at);
        }
        at.is_some()
    }

    /// The host replaced the patch itself, unseen by this history: nothing sent before is pending
    /// any more. (A preset load's writes pass through [`set`](Self::set), each replacing its
    /// parameter's entry, so its own values stay pending.)
    fn restored(&mut self) {
        self.new_epoch();
        self.sent.clear();
    }

    /// Around the preset bar's own calls, whose writes are whole-patch replacements.
    pub fn start_replacing(&mut self) {
        self.replacing = true;
        self.replaced = false;
    }

    /// Ends [`start_replacing`](Self::start_replacing); a new epoch began if anything was written.
    pub fn finish_replacing(&mut self) -> bool {
        self.replacing = false;
        let replaced = std::mem::take(&mut self.replaced);
        if replaced {
            self.new_epoch();
        }
        replaced
    }

    /// The host's restore count, read each frame: a change since the last is a new epoch.
    pub fn observe_restores(&mut self, restores: u64) -> bool {
        let seen = self.restores.replace(restores);
        let restored = seen.is_some_and(|seen| seen != restores);
        if restored {
            self.restored();
        }
        restored
    }

    pub fn gesture_open(&self) -> bool {
        !self.open_params.is_empty()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() && !self.gesture_open()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty() && !self.gesture_open()
    }

    /// The writes an undo makes, `(index, value)`, moving the action to redo.
    pub fn take_undo(&mut self) -> Option<Vec<(usize, f32)>> {
        if !self.can_undo() {
            return None;
        }
        let action = self.undo.pop()?;
        let writes = action
            .iter()
            .map(|change| (change.index, change.before))
            .collect();
        self.redo.push(action);
        Some(writes)
    }

    /// The writes a redo makes, moving the action back to undo.
    pub fn take_redo(&mut self) -> Option<Vec<(usize, f32)>> {
        if !self.can_redo() {
            return None;
        }
        let action = self.redo.pop()?;
        let writes = action
            .iter()
            .map(|change| (change.index, change.after))
            .collect();
        self.undo.push(action);
        Some(writes)
    }

    /// How many actions undo holds.
    pub fn depth(&self) -> usize {
        self.undo.len()
    }
}

/// The instance's history, shared by the plugin and whichever editor is open.
pub type Shared = Arc<Mutex<History>>;

pub fn lock(history: &Shared) -> std::sync::MutexGuard<'_, History> {
    history.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The editor's host context: every write passes to the host and is recorded on the way.
pub struct Recorder {
    host: GuiContext,
    history: Shared,
    pointers: Arc<dyn Fn() -> Vec<ParamPtr> + Send + Sync>,
}

impl Recorder {
    /// `pointers` lists every host parameter in the order the history's indices name them.
    pub fn context(
        host: GuiContext,
        history: Shared,
        pointers: Arc<dyn Fn() -> Vec<ParamPtr> + Send + Sync>,
    ) -> GuiContext {
        GuiContext::new(Arc::new(Recorder {
            host,
            history,
            pointers,
        }))
    }

    fn index(&self, param: ParamPtr) -> Option<usize> {
        (self.pointers)().iter().position(|other| *other == param)
    }
}

/// Writes `writes` as one gesture through `host`, bypassing the record: an undo or a redo. Each
/// value is noted as sent, so a gesture that starts before the host applies it starts from it.
pub fn write(host: &GuiContext, history: &Shared, pointers: &[ParamPtr], writes: &[(usize, f32)]) {
    let targets: Vec<(usize, ParamPtr, f32)> = writes
        .iter()
        .filter_map(|&(index, value)| pointers.get(index).map(|ptr| (index, *ptr, value)))
        .collect();
    // SAFETY: every pointer names a parameter of this plugin's `EqParams`, which the plugin keeps
    // alive for as long as any editor or context exists.
    unsafe {
        for (_, ptr, _) in &targets {
            host.raw_begin_set_parameter(*ptr);
        }
        for (index, ptr, value) in &targets {
            let readback = ptr.unmodulated_normalized_value();
            lock(history).note_sent(*index, readback, *value);
            host.raw_set_parameter_normalized(*ptr, *value);
        }
        for (_, ptr, _) in &targets {
            host.raw_end_set_parameter(*ptr);
        }
    }
}

impl GuiContextInner for Recorder {
    fn plugin_api(&self) -> PluginApi {
        self.host.plugin_api()
    }

    unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
        if let Some(index) = self.index(param) {
            // SAFETY: the pointer came from the caller's live parameter.
            let readback = unsafe { param.unmodulated_normalized_value() };
            lock(&self.history).begin(index, readback);
        }
        // SAFETY: forwarded unchanged.
        unsafe { self.host.raw_begin_set_parameter(param) };
    }

    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
        if let Some(index) = self.index(param) {
            let mut history = lock(&self.history);
            // An abandoned gesture's later writes would land on the patch that replaced it.
            if history.is_abandoned(index) {
                return;
            }
            // SAFETY: as above.
            let readback = unsafe { param.unmodulated_normalized_value() };
            history.set(index, normalized, readback);
        }
        // SAFETY: forwarded unchanged.
        unsafe { self.host.raw_set_parameter_normalized(param, normalized) };
    }

    unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
        let index = self.index(param);
        if index.is_some_and(|index| lock(&self.history).take_abandoned_end(index)) {
            return;
        }
        // SAFETY: forwarded unchanged.
        unsafe { self.host.raw_end_set_parameter(param) };
        if let Some(index) = index {
            lock(&self.history).end(index);
        }
    }

    fn get_state(&self) -> PluginState {
        self.host.get_state()
    }

    fn request_restart(&self) {
        self.host.request_restart();
    }

    /// A state carrying parameters replaces the patch; a parameter-free one only marks the project
    /// dirty.
    fn set_state(&self, state: PluginState) {
        if !state.params.is_empty() {
            let ended = {
                let mut history = lock(&self.history);
                history.restored();
                history.take_to_end()
            };
            let pointers = (self.pointers)();
            for index in ended {
                if let Some(ptr) = pointers.get(index) {
                    // SAFETY: as above.
                    unsafe { self.host.raw_end_set_parameter(*ptr) };
                }
            }
        }
        self.host.set_state(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(index: usize, before: f32, after: f32) -> Change {
        Change {
            index,
            before,
            after,
        }
    }

    #[test]
    fn a_gesture_over_two_parameters_is_one_action_and_undo_redo_reverse_it() {
        let mut history = History::default();
        history.begin(3, 0.5);
        history.begin(4, 0.2);
        history.set(3, 0.6, 0.5);
        history.set(4, 0.1, 0.2);
        history.set(3, 0.7, 0.6);
        history.end(3);
        assert!(!history.can_undo(), "still open: gain is held");
        history.end(4);
        assert_eq!(history.depth(), 1);
        assert_eq!(history.take_undo(), Some(vec![(3, 0.5), (4, 0.2)]));
        assert!(!history.can_undo());
        assert_eq!(history.take_redo(), Some(vec![(3, 0.7), (4, 0.1)]));
        assert_eq!(
            history.undo,
            vec![vec![change(3, 0.5, 0.7), change(4, 0.2, 0.1)]]
        );
    }

    #[test]
    fn a_gesture_that_returns_to_where_it_began_records_nothing() {
        let mut history = History::default();
        history.begin(0, 0.5);
        history.set(0, 0.6, 0.5);
        history.set(0, 0.5, 0.6);
        history.end(0);
        assert_eq!(history.depth(), 0);
    }

    #[test]
    fn a_new_action_clears_redo_and_the_length_is_bounded() {
        let mut history = History::default();
        for step in 0..LENGTH + 10 {
            history.begin(0, step as f32);
            history.set(0, step as f32 + 1.0, step as f32);
            history.end(0);
        }
        assert_eq!(history.depth(), LENGTH);
        history.take_undo();
        assert!(history.can_redo());
        history.begin(1, 0.0);
        history.set(1, 1.0, 0.0);
        history.end(1);
        assert!(!history.can_redo(), "a new edit forgets what was undone");
    }

    #[test]
    fn a_gesture_before_the_host_catches_up_starts_from_the_value_sent() {
        let mut history = History::default();
        // An instant edit sends 0.6; the host has not applied it when the next gesture begins.
        history.begin(2, 0.5);
        history.set(2, 0.6, 0.5);
        history.end(2);
        history.begin(2, 0.5);
        history.set(2, 0.7, 0.5);
        history.end(2);
        assert_eq!(
            history.take_undo(),
            Some(vec![(2, 0.6)]),
            "not back past the first edit"
        );
        // Once the host applied something else (its own automation), the readback is trusted.
        let mut history = History::default();
        history.begin(2, 0.5);
        history.set(2, 0.6, 0.5);
        history.end(2);
        history.begin(2, 0.9);
        history.set(2, 0.8, 0.9);
        history.end(2);
        assert_eq!(history.take_undo(), Some(vec![(2, 0.9)]));
    }

    #[test]
    fn a_preset_load_and_a_restore_each_start_an_epoch() {
        let mut history = History::default();
        history.begin(0, 0.0);
        history.set(0, 1.0, 0.0);
        history.end(0);
        history.start_replacing();
        history.begin(5, 0.0);
        history.set(5, 0.3, 0.0);
        history.end(5);
        assert!(history.finish_replacing());
        assert_eq!(
            history.depth(),
            0,
            "the load is not an action, and nothing before it remains"
        );

        history.start_replacing();
        assert!(
            !history.finish_replacing(),
            "the bar wrote nothing: no epoch"
        );

        assert!(
            !history.observe_restores(0),
            "the first reading is not a restore"
        );
        history.begin(0, 0.0);
        history.set(0, 1.0, 0.0);
        history.end(0);
        assert!(history.observe_restores(1));
        assert_eq!(history.depth(), 0);
    }

    #[test]
    fn a_gesture_open_across_an_epoch_is_abandoned_once() {
        let mut history = History::default();
        history.begin(3, 0.5);
        history.set(3, 0.6, 0.5);
        history.begin(3, 0.5);
        history.new_epoch();
        assert!(!history.gesture_open());
        assert_eq!(history.take_to_end(), vec![3], "once, though begun twice");
        assert!(history.take_to_end().is_empty());
        assert!(history.is_abandoned(3));
        assert!(
            history.take_abandoned_end(3),
            "its own end is the host's already"
        );
        assert!(!history.is_abandoned(3));
        assert_eq!(history.depth(), 0, "nothing recorded");
    }

    #[test]
    fn a_pending_value_reads_until_the_host_applies_it() {
        let mut history = History::default();
        history.set(1, 0.8, 0.2);
        assert_eq!(history.pending(1, 0.2), 0.8, "not applied yet");
        assert_eq!(history.pending(1, 0.8), 0.8, "applied");
        assert_eq!(history.pending(1, 0.4), 0.4, "something else moved it");
        assert_eq!(history.pending(2, 0.3), 0.3);
        history.settle(|_| 0.2);
        assert_eq!(history.pending(1, 0.2), 0.8, "still on its way");
        // The host applied it, then automation put the old value back: that is not the edit.
        history.settle(|_| 0.8);
        assert_eq!(history.pending(1, 0.2), 0.2);
    }

    #[test]
    fn a_restore_forgets_what_was_sent_before_it_and_a_load_keeps_its_own() {
        // An edit to 0.6 the host has not applied, then a restore that leaves 0.5.
        let mut history = History::default();
        history.observe_restores(0);
        history.begin(2, 0.5);
        history.set(2, 0.6, 0.5);
        history.end(2);
        assert!(history.observe_restores(1));
        history.begin(2, 0.5);
        history.set(2, 0.7, 0.5);
        history.end(2);
        assert_eq!(
            history.take_undo(),
            Some(vec![(2, 0.5)]),
            "back to what the restore left, not the edit before it"
        );

        // A load the host has not applied yet: the next gesture starts from the load's value.
        let mut history = History::default();
        history.start_replacing();
        history.begin(5, 0.0);
        history.set(5, 0.3, 0.0);
        history.end(5);
        assert!(history.finish_replacing());
        history.begin(5, 0.0);
        history.set(5, 0.4, 0.0);
        history.end(5);
        assert_eq!(history.take_undo(), Some(vec![(5, 0.3)]));
    }
}
