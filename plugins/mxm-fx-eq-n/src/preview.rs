//! Match's Preview (plan §7.6): the proposed bands heard before they are applied. A transient
//! command, as band solo is (`audition.rs`): no id, state or preset value, so a project never
//! reopens previewing and a render never plays one.
//!
//! - **Into the free slots**: the editor puts each proposed band in the slot Apply would write, and
//!   the shell plays it there in place of the slot's Off, each block, only with an editor connected
//!   and never offline (`MxmFxEqN::sync`); a slot the player has since used is the player's. Each
//!   slot is set once a block, so the engine's transitions run as for any band.
//! - **A sequence lock of atomics**: the editor writes the bands between two counts of a sequence,
//!   odd while it writes; the audio thread reads the count, the bands and the count again, and keeps
//!   what it last read whole when they differ. One writer, the editor; nothing allocates, locks or
//!   waits.

use std::sync::atomic::{AtomicU8, AtomicU32, Ordering, fence};

use mxm_fx_eq_n_dsp::{Band, SLOTS, Shape};

/// One slot's proposed band: its shape (0 none, else its place in `params::SHAPES` and one), and
/// its frequency, gain and Q as bits.
#[derive(Default)]
struct Slot {
    shape: AtomicU8,
    freq: AtomicU32,
    gain_db: AtomicU32,
    q: AtomicU32,
}

/// The bands the editor previews, by slot.
#[derive(Default)]
pub struct Preview {
    sequence: AtomicU32,
    slots: [Slot; SLOTS],
}

/// A previewed band in its slot: static, as Apply writes it, the slope the default.
fn band(shape: Shape, freq: f32, gain_db: f32, q: f32) -> Band {
    Band {
        shape: Some(shape),
        bypass: false,
        freq,
        gain_db,
        q,
        slope: crate::params::DEFAULT_SLOPE,
    }
}

impl Preview {
    /// The editor previews `bands`, each in its slot, and nothing elsewhere; none ends it.
    pub fn publish(&self, bands: &[(usize, Shape, f32, f32, f32)]) {
        let sequence = self.sequence.load(Ordering::Relaxed);
        self.sequence
            .store(sequence.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        for slot in &self.slots {
            slot.shape.store(0, Ordering::Relaxed);
        }
        for &(index, shape, freq, gain_db, q) in bands {
            let Some(slot) = self.slots.get(index) else {
                continue;
            };
            let Some(place) = crate::params::SHAPES.iter().position(|s| *s == shape) else {
                continue;
            };
            slot.freq.store(freq.to_bits(), Ordering::Relaxed);
            slot.gain_db.store(gain_db.to_bits(), Ordering::Relaxed);
            slot.q.store(q.to_bits(), Ordering::Relaxed);
            slot.shape.store(place as u8 + 1, Ordering::Relaxed);
        }
        self.sequence
            .store(sequence.wrapping_add(2), Ordering::Release);
    }

    /// Ends the preview.
    pub fn clear(&self) {
        self.publish(&[]);
    }

    /// The audio thread's read into `out`, by slot: left as it was while the editor writes.
    pub fn read(&self, out: &mut [Option<Band>; SLOTS]) {
        let before = self.sequence.load(Ordering::Acquire);
        if before % 2 == 1 {
            return;
        }
        let mut read = [None; SLOTS];
        for (slot, read) in self.slots.iter().zip(read.iter_mut()) {
            let shape = slot.shape.load(Ordering::Relaxed);
            *read = shape
                .checked_sub(1)
                .and_then(|place| crate::params::SHAPES.get(usize::from(place)))
                .map(|&shape| {
                    band(
                        shape,
                        f32::from_bits(slot.freq.load(Ordering::Relaxed)),
                        f32::from_bits(slot.gain_db.load(Ordering::Relaxed)),
                        f32::from_bits(slot.q.load(Ordering::Relaxed)),
                    )
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

    /// **What is published is read whole, and cleared** (plan §7.6): bands in their slots, nothing
    /// elsewhere, and none once cleared; a read during a write keeps what was read before.
    #[test]
    fn what_is_published_is_read_whole_and_cleared() {
        let preview = Preview::default();
        let mut out = [None; SLOTS];
        preview.publish(&[
            (2, Shape::LowShelf, 120.0, 3.0, 0.71),
            (5, Shape::Bell, 2_000.0, -2.0, 1.5),
        ]);
        preview.read(&mut out);
        assert_eq!(out[2], Some(band(Shape::LowShelf, 120.0, 3.0, 0.71)));
        assert_eq!(out[5], Some(band(Shape::Bell, 2_000.0, -2.0, 1.5)));
        assert_eq!(out.iter().flatten().count(), 2);
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
