//! Several bands (plan §7.9, decisions 44–47): the selection and its primary, a group's edits,
//! copy, cut and paste through the process's clipboard, Select all and Invert gain.
//!
//! - **The selection** is the primary (`EqApp::selected`, the band the Band card shows, solo plays
//!   and MOVE's edge takes) and the others (`EqApp::others`). Editor state, never saved. Bands are
//!   ordered along the axis by frequency, equal frequencies by slot.
//! - **A group moves with its primary** ([`EqApp::group_edit`]): around any edit of the primary,
//!   its fields before and after are compared and the others follow, frequency, Q, Attack and
//!   Release by one ratio, gain, Range and Threshold by one offset, a discrete field set; a band
//!   moves in a field only if it has it, and the group stops at its first member's limit, the
//!   primary pulled back with it. The others are written as the edit was (`Writer`), so the whole
//!   is one undo step.
//! - **The clipboard is the process's** (decision 46): a mutex-guarded static, editor threads only,
//!   shared by every instance; a copy holds no slot numbers, so a paste never overwrites.

use std::sync::{Mutex, PoisonError};

use mxm_fx_eq_n_dsp::{FREQ_MIN, GAIN_RANGE, Q_MAX, Q_MIN, SLOTS};
use nice_plug::prelude::Param;

use super::{DynamicsField, EqApp, Field, OUTPUT_INDEX, band_index, dynamics_index, history};
use super::{key_index, place_index};

/// How the others follow a field of the primary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Follow {
    Ratio,
    Offset,
    Set,
}

/// One of a slot's twelve fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    Band(Field),
    Dynamics(DynamicsField),
    Key,
    Place,
}

impl Which {
    /// Every field of a slot, in [`EqApp::slot_writes`]'s order.
    const ALL: [Which; 12] = [
        Which::Band(Field::Type),
        Which::Band(Field::Bypass),
        Which::Band(Field::Freq),
        Which::Band(Field::Gain),
        Which::Band(Field::Q),
        Which::Band(Field::Slope),
        Which::Dynamics(DynamicsField::Range),
        Which::Dynamics(DynamicsField::Threshold),
        Which::Dynamics(DynamicsField::Attack),
        Which::Dynamics(DynamicsField::Release),
        Which::Key,
        Which::Place,
    ];

    fn index(self, slot: usize) -> usize {
        match self {
            Which::Band(field) => band_index(slot, field),
            Which::Dynamics(field) => dynamics_index(slot, field),
            Which::Key => key_index(slot),
            Which::Place => place_index(slot),
        }
    }

    fn follow(self) -> Follow {
        match self {
            Which::Band(Field::Freq | Field::Q)
            | Which::Dynamics(DynamicsField::Attack | DynamicsField::Release) => Follow::Ratio,
            Which::Band(Field::Gain)
            | Which::Dynamics(DynamicsField::Range | DynamicsField::Threshold) => Follow::Offset,
            _ => Follow::Set,
        }
    }
}

/// How a group's other bands are written: as the edit was, so the whole is one undo step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writer {
    /// Inside a gesture the editor holds open (`EqApp::send`): a drag, the wheel, the keys.
    Send,
    /// As one complete write, the action held open around it: the card's controls, a menu.
    Instant,
}

/// Why a paste or a copy of the selection cannot be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteRefusal {
    /// Nothing has been copied.
    Empty,
    /// Not every band fits in the Off slots.
    Full,
}

/// What Copy put on the process's clipboard: each band's twelve fields as normalised values, in
/// slot order, and which of them was the primary.
#[derive(Clone, Debug, PartialEq)]
struct Copied {
    bands: Vec<[f32; 12]>,
    primary: usize,
}

static CLIPBOARD: Mutex<Option<Copied>> = Mutex::new(None);

fn clipboard() -> std::sync::MutexGuard<'static, Option<Copied>> {
    CLIPBOARD.lock().unwrap_or_else(PoisonError::into_inner)
}

impl EqApp {
    // ---- the selection --------------------------------------------------------------------

    /// Every selected band in use, the primary among them, along the axis.
    pub(crate) fn selection(&self) -> Vec<usize> {
        let mut all: Vec<usize> = self
            .selected
            .into_iter()
            .chain(self.others.iter().copied())
            .filter(|&slot| self.band_in_use(slot))
            .collect();
        all.dedup();
        self.along_axis(&mut all);
        all
    }

    /// The selected bands besides the primary, in use.
    pub(crate) fn others_selected(&self) -> Vec<usize> {
        self.others
            .iter()
            .copied()
            .filter(|&slot| Some(slot) != self.selected && self.band_in_use(slot))
            .collect()
    }

    pub(crate) fn is_selected(&self, slot: usize) -> bool {
        self.selected == Some(slot) || self.others.contains(&slot)
    }

    /// `slots` ordered along the axis: by frequency, equal frequencies by slot.
    fn along_axis(&self, slots: &mut [usize]) {
        slots.sort_by(|&a, &b| {
            let freq = |slot| self.clamp_freq(self.band_now(slot).freq);
            freq(a).total_cmp(&freq(b)).then(a.cmp(&b))
        });
    }

    /// Nothing selected.
    pub(crate) fn select_none(&mut self) {
        self.selected = None;
        self.others.clear();
        self.edge = None;
    }

    /// `slots` selected, `primary` the primary if it is among them, else the first along the axis.
    pub(crate) fn select_set(&mut self, slots: &[usize], primary: Option<usize>) {
        let mut slots: Vec<usize> = slots
            .iter()
            .copied()
            .filter(|&slot| self.band_in_use(slot))
            .collect();
        slots.dedup();
        self.along_axis(&mut slots);
        let Some(&first) = slots.first() else {
            self.select_none();
            return;
        };
        let primary = primary.filter(|slot| slots.contains(slot)).unwrap_or(first);
        self.set_primary(primary);
        self.others = slots.into_iter().filter(|&slot| slot != primary).collect();
    }

    /// A press on a point that is to be edited: the primary if the point is selected, keeping the
    /// group; else the point alone.
    pub(crate) fn grab_point(&mut self, slot: usize) {
        if self.is_selected(slot) {
            let previous = self.selected;
            self.set_primary(slot);
            self.others.retain(|&other| other != slot);
            if let Some(previous) = previous.filter(|&previous| previous != slot) {
                self.others.push(previous);
            }
        } else {
            self.select(slot);
        }
    }

    /// Shift-click: `slot` added, the primary moving to it, or taken out, the primary then the
    /// selected band nearest it along the axis (of two as near, the lower).
    pub(crate) fn toggle_selected(&mut self, slot: usize) {
        if !self.is_selected(slot) {
            let previous = self.selected;
            self.set_primary(slot);
            if let Some(previous) = previous {
                self.others.push(previous);
            }
            return;
        }
        let remaining: Vec<usize> = self
            .selection()
            .into_iter()
            .filter(|&other| other != slot)
            .collect();
        if self.selected != Some(slot) {
            self.others.retain(|&other| other != slot);
            return;
        }
        let freq = |app: &Self, slot: usize| app.clamp_freq(app.band_now(slot).freq).ln();
        let at = freq(self, slot);
        let nearest = remaining.iter().copied().min_by(|&a, &b| {
            (freq(self, a) - at)
                .abs()
                .total_cmp(&(freq(self, b) - at).abs())
        });
        match nearest {
            Some(primary) => self.select_set(&remaining, Some(primary)),
            None => self.select_none(),
        }
    }

    /// SELECT with ← → (the language's "the selection, by extending it"): the next band along the
    /// axis not selected yet, from the primary (or the keyboard's place on the line) in that
    /// direction, added and made the primary; none left that way, nothing.
    pub(crate) fn extend_selection(&mut self, right: bool) {
        let bands = self.by_frequency();
        let from = self
            .selected
            .and_then(|primary| bands.iter().position(|&(_, slot)| slot == primary));
        let candidates: Vec<usize> = match (from, right) {
            (Some(at), true) => bands[at + 1..].iter().map(|&(_, slot)| slot).collect(),
            (Some(at), false) => bands[..at].iter().rev().map(|&(_, slot)| slot).collect(),
            (None, true) => bands
                .iter()
                .filter(|&&(freq, _)| freq > self.focus_freq)
                .map(|&(_, slot)| slot)
                .collect(),
            (None, false) => bands
                .iter()
                .rev()
                .filter(|&&(freq, _)| freq < self.focus_freq)
                .map(|&(_, slot)| slot)
                .collect(),
        };
        if let Some(next) = candidates.into_iter().find(|&slot| !self.is_selected(slot)) {
            let previous = self.selected;
            self.set_primary(next);
            if let Some(previous) = previous {
                self.others.push(previous);
            }
        }
    }

    /// Ctrl/Cmd+A: every band in use.
    pub(crate) fn select_all(&mut self) {
        let all: Vec<usize> = (0..SLOTS).filter(|&slot| self.band_in_use(slot)).collect();
        self.select_set(&all, self.selected);
    }

    /// Each frame: the others in use and never the primary; a primary gone (turned Off outside the
    /// editor) handing over to the first left along the axis.
    pub(crate) fn tidy_selection(&mut self) {
        let primary = self.selected.filter(|&slot| self.band_in_use(slot));
        let others: Vec<usize> = self.others_selected();
        match primary {
            Some(_) => self.others = others,
            None if others.is_empty() => {
                self.others.clear();
            }
            None => self.select_set(&others, None),
        }
    }

    // ---- a group's edits ----------------------------------------------------------------

    /// The primary's twelve fields, plain.
    fn plains(&self, slot: usize) -> [f32; 12] {
        Which::ALL.map(|which| self.current_plain(which.index(slot)))
    }

    /// Whether `slot` has the field: a field it lacks never moves with the group.
    fn has_field(&self, which: Which, slot: usize) -> bool {
        let band = self.band_now(slot);
        let Some(shape) = band.shape else {
            return false;
        };
        match which {
            Which::Band(Field::Gain) => shape.has_gain(),
            Which::Band(Field::Q) => shape.has_q(band.slope),
            Which::Band(Field::Slope) => shape.has_slope(),
            Which::Dynamics(_) | Which::Key => shape.is_dynamic(),
            Which::Place => !self.telemetry.mono(),
            Which::Band(_) => true,
        }
    }

    /// A continuous field's plain limits for `slot`.
    fn limits(&self, which: Which, slot: usize) -> (f32, f32) {
        let range = GAIN_RANGE as f32;
        match which {
            Which::Band(Field::Freq) => (FREQ_MIN as f32, self.high()),
            Which::Band(Field::Gain) => (-range, range),
            Which::Band(Field::Q) => (Q_MIN as f32, Q_MAX as f32),
            Which::Dynamics(field) => {
                let dynamics = &self.params.dynamics[slot];
                let param = match field {
                    DynamicsField::Range => &dynamics.range,
                    DynamicsField::Threshold => &dynamics.threshold,
                    DynamicsField::Attack => &dynamics.attack,
                    DynamicsField::Release => &dynamics.release,
                };
                (param.preview_plain(0.0), param.preview_plain(1.0))
            }
            _ => (f32::NEG_INFINITY, f32::INFINITY),
        }
    }

    /// Runs the edit `edit` of the primary, then moves the rest of the selection with it (plan
    /// §7.9), written by `writer`; with no group, the edit alone.
    pub(crate) fn group_edit<R>(&mut self, writer: Writer, edit: impl FnOnce(&mut Self) -> R) -> R {
        let primary = self
            .selected
            .filter(|&slot| self.band_in_use(slot) && !self.others_selected().is_empty());
        let Some(primary) = primary else {
            return edit(self);
        };
        // An instant edit closes its own action; held open, the group's writes join it.
        let hold = writer == Writer::Instant;
        if hold {
            let readback = self.erased(OUTPUT_INDEX).normalised();
            history::lock(&self.history).begin(OUTPUT_INDEX, readback);
        }
        let before = self.plains(primary);
        let result = edit(self);
        if self.selected == Some(primary) && self.band_in_use(primary) {
            self.follow(primary, &before, writer);
        }
        if hold {
            history::lock(&self.history).end(OUTPUT_INDEX);
        }
        result
    }

    /// The others follow the primary's change from `before` (plan §7.9, decision 45).
    fn follow(&mut self, primary: usize, before: &[f32; 12], writer: Writer) {
        let after = self.plains(primary);
        let others = self.others_selected();
        let mut writes: Vec<(usize, f32)> = Vec::new();
        for (at, which) in Which::ALL.into_iter().enumerate() {
            let (was, now) = (before[at], after[at]);
            if was == now {
                continue;
            }
            let members: Vec<usize> = others
                .iter()
                .copied()
                .filter(|&slot| self.has_field(which, slot))
                .collect();
            match which.follow() {
                Follow::Set => {
                    for slot in members {
                        let index = which.index(slot);
                        writes.push((index, self.normalised(index, now)));
                    }
                }
                Follow::Ratio if was > 0.0 && now > 0.0 => {
                    let wanted = now / was;
                    let mut ratio = wanted;
                    for &slot in &members {
                        let value = self.current_plain(which.index(slot));
                        let (low, high) = self.limits(which, slot);
                        if value > 0.0 {
                            ratio = if ratio > 1.0 {
                                ratio.min(high / value)
                            } else {
                                ratio.max(low / value)
                            };
                        }
                    }
                    if ratio != wanted {
                        let index = which.index(primary);
                        writes.push((index, self.normalised(index, was * ratio)));
                    }
                    for slot in members {
                        let index = which.index(slot);
                        let value = self.current_plain(index);
                        writes.push((index, self.normalised(index, value * ratio)));
                    }
                }
                Follow::Offset => {
                    let wanted = now - was;
                    let mut offset = wanted;
                    for &slot in &members {
                        let value = self.current_plain(which.index(slot));
                        let (low, high) = self.limits(which, slot);
                        offset = if offset > 0.0 {
                            offset.min(high - value)
                        } else {
                            offset.max(low - value)
                        };
                    }
                    if offset != wanted {
                        let index = which.index(primary);
                        writes.push((index, self.normalised(index, was + offset)));
                    }
                    for slot in members {
                        let index = which.index(slot);
                        let value = self.current_plain(index);
                        writes.push((index, self.normalised(index, value + offset)));
                    }
                }
                Follow::Ratio => {}
            }
        }
        if writes.is_empty() {
            return;
        }
        match writer {
            Writer::Send => {
                for (index, value) in writes {
                    self.send(index, value);
                }
            }
            Writer::Instant => self.instant(&writes),
        }
    }

    // ---- commands on the selection ---------------------------------------------------------

    /// DELETE, the card's or the menu's Delete: every selected band Off, one undo step.
    pub(crate) fn delete_selection(&mut self) {
        let selection = self.selection();
        let Some(&first) = selection.first() else {
            return;
        };
        self.close_open();
        let primary = self.selected.unwrap_or(first);
        let writes: Vec<(usize, f32)> = selection
            .iter()
            .map(|&slot| (band_index(slot, Field::Type), 0.0))
            .collect();
        let freq = self.clamp_freq(self.band_now(primary).freq);
        self.instant(&writes);
        self.select_none();
        self.focus_freq = freq;
    }

    /// The selected bands, each as its twelve fields' normalised values, in slot order, with the
    /// primary's place among them.
    fn copied(&self) -> Option<Copied> {
        let mut slots = self.selection();
        slots.sort_unstable();
        let primary = self
            .selected
            .and_then(|primary| slots.iter().position(|&slot| slot == primary))
            .unwrap_or(0);
        (!slots.is_empty()).then(|| Copied {
            bands: slots
                .iter()
                .map(|&slot| Which::ALL.map(|which| self.normalised_now(which.index(slot))))
                .collect(),
            primary,
        })
    }

    /// Ctrl/Cmd+C or the menu's Copy: the selection onto the process's clipboard. Whether there
    /// was one.
    pub(crate) fn copy_selection(&mut self) -> bool {
        let Some(copied) = self.copied() else {
            return false;
        };
        *clipboard() = Some(copied);
        true
    }

    /// Ctrl/Cmd+X or the menu's Cut: copied, then deleted.
    pub(crate) fn cut_selection(&mut self) {
        if self.copy_selection() {
            self.delete_selection();
        }
    }

    /// Why a paste would be refused now, or `None` when it would go through.
    pub(crate) fn paste_refusal(&self) -> Option<PasteRefusal> {
        let held = clipboard().as_ref().map(|copied| copied.bands.len());
        match held {
            None => Some(PasteRefusal::Empty),
            Some(count) if self.free_slots().count() < count => Some(PasteRefusal::Full),
            Some(_) => None,
        }
    }

    /// Ctrl/Cmd+V or the menu's Paste: the clipboard's bands into the lowest Off slots, in their
    /// slot order onto the free slots in theirs, one undo step, then the selection, the copy of the
    /// primary its primary. All or nothing.
    pub(crate) fn paste(&mut self) -> Result<(), PasteRefusal> {
        let Some(copied) = clipboard().clone() else {
            return Err(PasteRefusal::Empty);
        };
        let free: Vec<usize> = self.free_slots().take(copied.bands.len()).collect();
        if free.len() < copied.bands.len() {
            return Err(PasteRefusal::Full);
        }
        self.close_open();
        let writes: Vec<(usize, f32)> = free
            .iter()
            .zip(&copied.bands)
            .flat_map(|(&slot, fields)| {
                Which::ALL
                    .into_iter()
                    .zip(fields.iter().copied())
                    .map(move |(which, value)| (which.index(slot), value))
            })
            .collect();
        self.instant(&writes);
        self.select_set(&free, free.get(copied.primary).copied());
        Ok(())
    }

    /// The menu's Invert gain: every selected band with a gain turned over, one undo step.
    pub(crate) fn invert_selection(&mut self) {
        let writes: Vec<(usize, f32)> = self
            .selection()
            .into_iter()
            .filter(|&slot| self.has_field(Which::Band(Field::Gain), slot))
            .map(|slot| {
                let index = band_index(slot, Field::Gain);
                (index, self.normalised(index, -self.current_plain(index)))
            })
            .collect();
        if writes.is_empty() {
            return;
        }
        self.close_open();
        self.instant(&writes);
    }

    /// The menu's shape for the selection: set on every selected band, one undo step.
    pub(crate) fn set_shape_selection(&mut self, shape: mxm_fx_eq_n_dsp::Shape) {
        let writes: Vec<(usize, f32)> = self
            .selection()
            .into_iter()
            .map(|slot| {
                let index = band_index(slot, Field::Type);
                (
                    index,
                    self.normalised(index, crate::params::type_of(Some(shape)) as f32),
                )
            })
            .collect();
        self.close_open();
        self.instant(&writes);
    }

    /// DUPLICATE's first arrow on a selection: every selected band copied into the lowest Off
    /// slots inside the gesture, in slot order onto the free slots in theirs, refused whole when
    /// not all fit; the copies the selection, the copy of the primary its primary. Whether the
    /// copies were made.
    pub(crate) fn start_copy_selection(&mut self) -> bool {
        let mut originals = self.selection();
        originals.sort_unstable();
        let Some(primary) = self.selected.filter(|slot| originals.contains(slot)) else {
            return false;
        };
        let free: Vec<usize> = self.free_slots().take(originals.len()).collect();
        if free.len() < originals.len() {
            return false;
        }
        self.close_open();
        let mut primary_copy = free[0];
        for (&original, &slot) in originals.iter().zip(&free) {
            let writes = self.slot_writes(
                slot,
                self.band_now(original),
                self.dynamics_now(original),
                self.key_now(original),
                self.placement_now(original),
            );
            for (index, value) in writes {
                self.send(index, value);
            }
            if original == primary {
                primary_copy = slot;
            }
        }
        self.copy = Some((primary_copy, primary));
        self.copied_from = originals;
        self.select_set(&free, Some(primary_copy));
        true
    }
}

/// The process's clipboard emptied, for tests that need to start from nothing.
#[cfg(test)]
pub(crate) fn clear_clipboard() {
    *clipboard() = None;
}
