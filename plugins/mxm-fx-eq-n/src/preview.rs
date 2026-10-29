//! Match's Preview (plan §7.6): the proposed bands heard before they are applied. A transient
//! command, as band solo is (`audition.rs`): no id, state or preset value, so a project never
//! reopens previewing and automation never plays one.
//!
//! - **Into the slots Apply writes**: the editor puts each proposed band in the slot Apply would
//!   write, and the shell plays it there each block, only with an editor connected, whatever mode
//!   the host says it runs in (`MxmFxEqN::sync`). In an Off slot always; in a slot holding a band
//!   Match applied only when the editor marks it to **replace** that band, as Apply would, and a
//!   slot of Match's that Apply would turn Off is previewed as an Off band. A slot the player has
//!   since used is the player's. Each slot is set once a block, so the engine's transitions run as
//!   for any band.
//! - **A sequence lock of atomics**: the editor writes the bands between two counts of a sequence,
//!   odd while it writes; the audio thread reads the count, the bands and the count again, and keeps
//!   what it last read whole when they differ. One writer, the editor; nothing allocates, locks or
//!   waits.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering, fence};

use mxm_fx_eq_n_dsp::{Band, SLOTS, Shape};

/// One slot's previewed band: its shape (0 none, [`OFF`] an Off band, else its place in
/// `params::SHAPES` and one), whether it replaces the slot's own band, and its frequency, gain and
/// Q as bits.
#[derive(Default)]
struct Slot {
    shape: AtomicU8,
    replace: AtomicBool,
    freq: AtomicU32,
    gain_db: AtomicU32,
    q: AtomicU32,
}

/// A slot's shape byte for an Off band: a band Match applied that Apply would turn Off.
const OFF: u8 = u8::MAX;

/// What the editor previews in one slot: the slot, the band (`None` for Off), and whether it
/// replaces the slot's own band, one Match applied, rather than only an Off slot's.
pub type Entry = (usize, Option<(Shape, f32, f32, f32)>, bool);

/// What the shell plays in a slot while Preview is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Previewed {
    /// The band, static as Apply writes it, its slope the default; Off for one Apply turns Off.
    pub band: Band,
    /// Whether it replaces the slot's own band; otherwise it plays only while the slot is Off.
    pub replace: bool,
}

/// The bands the editor previews, by slot.
#[derive(Default)]
pub struct Preview {
    sequence: AtomicU32,
    slots: [Slot; SLOTS],
}

/// A previewed band in its slot: static, as Apply writes it, the slope the default.
fn band(shape: Option<Shape>, freq: f32, gain_db: f32, q: f32) -> Band {
    Band {
        shape,
        bypass: false,
        freq,
        gain_db,
        q,
        slope: crate::params::DEFAULT_SLOPE,
    }
}

impl Preview {
    /// The editor previews `entries`, each in its slot, and nothing elsewhere; none ends it.
    pub fn publish(&self, entries: &[Entry]) {
        let sequence = self.sequence.load(Ordering::Relaxed);
        self.sequence
            .store(sequence.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for slot in &self.slots {
            slot.shape.store(0, Ordering::Relaxed);
        }
        for &(index, entry, replace) in entries {
            let Some(slot) = self.slots.get(index) else {
                continue;
            };
            let shape = match entry {
                None => OFF,
                Some((shape, freq, gain_db, q)) => {
                    let Some(place) = crate::params::SHAPES.iter().position(|s| *s == shape) else {
                        continue;
                    };
                    slot.freq.store(freq.to_bits(), Ordering::Relaxed);
                    slot.gain_db.store(gain_db.to_bits(), Ordering::Relaxed);
                    slot.q.store(q.to_bits(), Ordering::Relaxed);
                    place as u8 + 1
                }
            };
            slot.replace.store(replace, Ordering::Relaxed);
            slot.shape.store(shape, Ordering::Relaxed);
        }
        self.sequence
            .store(sequence.wrapping_add(2), Ordering::Release);
    }

    /// Ends the preview.
    pub fn clear(&self) {
        self.publish(&[]);
    }

    /// The audio thread's read into `out`, by slot: left as it was while the editor writes.
    pub fn read(&self, out: &mut [Option<Previewed>; SLOTS]) {
        let before = self.sequence.load(Ordering::Acquire);
        if before % 2 == 1 {
            return;
        }
        let mut read = [None; SLOTS];
        for (slot, read) in self.slots.iter().zip(read.iter_mut()) {
            let shape = match slot.shape.load(Ordering::Relaxed) {
                0 => continue,
                OFF => None,
                place => match crate::params::SHAPES.get(usize::from(place - 1)) {
                    Some(&shape) => Some(shape),
                    None => continue,
                },
            };
            *read = Some(Previewed {
                band: band(
                    shape,
                    f32::from_bits(slot.freq.load(Ordering::Relaxed)),
                    f32::from_bits(slot.gain_db.load(Ordering::Relaxed)),
                    f32::from_bits(slot.q.load(Ordering::Relaxed)),
                ),
                replace: slot.replace.load(Ordering::Relaxed),
            });
        }
        fence(Ordering::Acquire);
        if self.sequence.load(Ordering::Relaxed) == before {
            *out = read;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **What is published is read whole, and cleared** (plan §7.6): bands in their slots, one to
    /// replace a band Match applied and one Off, nothing elsewhere, and none once cleared; a read
    /// during a write keeps what was read before.
    #[test]
    fn what_is_published_is_read_whole_and_cleared() {
        let preview = Preview::default();
        let mut out = [None; SLOTS];
        preview.publish(&[
            (2, Some((Shape::LowShelf, 120.0, 3.0, 0.71)), false),
            (5, Some((Shape::Bell, 2_000.0, -2.0, 1.5)), true),
            (7, None, true),
        ]);
        preview.read(&mut out);
        let at = |slot: usize| out[slot].expect("previewed");
        assert_eq!(
            at(2).band,
            band(Some(Shape::LowShelf), 120.0, 3.0, 0.71),
            "an Off slot's"
        );
        assert!(!at(2).replace);
        assert_eq!(at(5).band, band(Some(Shape::Bell), 2_000.0, -2.0, 1.5));
        assert!(at(5).replace, "a band Match applied, replaced");
        assert_eq!(
            (at(7).band.shape, at(7).replace),
            (None, true),
            "turned Off"
        );
        assert_eq!(out.iter().flatten().count(), 3);
        // A write in progress: the sequence odd, so what was read stays.
        preview.sequence.fetch_add(1, Ordering::Relaxed);
        let before = out;
        preview.read(&mut out);
        assert_eq!(out, before);
        preview.sequence.fetch_add(1, Ordering::Relaxed);
        preview.clear();
        preview.read(&mut out);
        assert!(out.iter().all(Option::is_none));
    }
}
