//! Band solo: the plugin's one transient command (plan §5.2; `plugins/AGENTS.md`, *Editor
//! contract*, the transient-command exception).
//!
//! **Why solo is not a parameter.** An audition is what somebody is listening to while they hold a
//! key or a button. Saved, a project would reopen soloed; automated, a render would play one band's
//! band pass instead of the EQ; recalled by a preset, it would change what the EQ does without
//! changing the EQ. So it has no id, no state and no preset value, and the DSP takes it as an
//! ordinary call (`Engine::set_solo`) that the shell makes once per block.
//!
//! **The word.** One atomic holds the band asked for and a count of the times the shell has ended a
//! solo. The editor asks with the count it last read, so a request made before an ending cannot
//! bring the solo back after it: the compare fails and the editor learns so. The shell ends a solo
//! on reset (which a state restore and an activation also run), on deactivation and when the editor
//! closes, and never waits to do it. A request to turn solo off can fail only when the shell has
//! already turned it off. Nothing allocates, locks or blocks.

use std::sync::atomic::{AtomicU64, Ordering};

use mxm_fx_eq_n_dsp::SLOTS;

#[derive(Default)]
pub struct Audition {
    word: AtomicU64,
}

impl Audition {
    /// The count of endings, and the band being auditioned.
    pub fn state(&self) -> (u32, Option<usize>) {
        unpack(self.word.load(Ordering::Acquire))
    }

    /// The band being auditioned.
    pub fn slot(&self) -> Option<usize> {
        self.state().1
    }

    /// The shell ends any solo. Never waits: a racing request fails rather than undoing it.
    pub fn end(&self) {
        let _ = self
            .word
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |word| {
                let (endings, _) = unpack(word);
                Some(pack(endings.wrapping_add(1), None))
            });
    }

    /// The editor asks for `slot` (or none), as of the count of endings it read. False, with
    /// nothing changed, if the shell has ended a solo since.
    pub fn request(&self, endings: u32, slot: Option<usize>) -> bool {
        let current = self.word.load(Ordering::Acquire);
        unpack(current).0 == endings
            && self
                .word
                .compare_exchange(
                    current,
                    pack(endings, slot.filter(|slot| *slot < SLOTS)),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
    }
}

fn pack(endings: u32, slot: Option<usize>) -> u64 {
    (u64::from(endings) << 32) | slot.map_or(0, |slot| slot as u64 + 1)
}

fn unpack(word: u64) -> (u32, Option<usize>) {
    let slot = (word & u64::from(u32::MAX)) as usize;
    (
        (word >> 32) as u32,
        slot.checked_sub(1).filter(|slot| *slot < SLOTS),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ending_beats_every_request_made_before_it() {
        let audition = Audition::default();
        let (endings, _) = audition.state();
        assert!(audition.request(endings, Some(3)));
        assert_eq!(audition.slot(), Some(3));
        audition.end();
        assert_eq!(audition.slot(), None);
        // The editor's request from before the ending cannot bring it back.
        assert!(!audition.request(endings, Some(3)));
        assert_eq!(audition.slot(), None);
        // Asking to turn off fails only because the shell already did.
        assert!(!audition.request(endings, None));
        assert_eq!(audition.slot(), None);
        // With the count read afresh, the editor solos again.
        let (endings, _) = audition.state();
        assert!(audition.request(endings, Some(5)));
        assert_eq!(audition.slot(), Some(5));
        assert!(audition.request(endings, None));
        assert_eq!(audition.slot(), None);
    }

    #[test]
    fn a_band_outside_the_slots_is_no_solo() {
        let audition = Audition::default();
        let (endings, _) = audition.state();
        assert!(audition.request(endings, Some(SLOTS)));
        assert_eq!(audition.slot(), None);
    }

    #[test]
    fn the_count_wraps_without_losing_an_ending() {
        let audition = Audition {
            word: AtomicU64::new(pack(u32::MAX, Some(2))),
        };
        audition.end();
        assert_eq!(audition.state(), (0, None));
        assert!(!audition.request(u32::MAX, Some(2)));
    }
}
