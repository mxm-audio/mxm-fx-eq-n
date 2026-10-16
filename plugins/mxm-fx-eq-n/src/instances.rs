//! Other tracks (plan §7.8, decisions 41–43): every instance of this plugin in the process, so an
//! editor can draw another track's spectrum.
//!
//! - **One registry per process**, joined when an instance is made and left when it is dropped.
//!   Instances in another process (Bitwig's *Individually* hosting) are never seen.
//! - **The audio thread never waits on another instance.** Each instance's second tap, post-EQ
//!   only (`Tap::capture_post`), copies blocks into its own lock-free ring only while it is
//!   watched — an atomic count of watchers, the one thing its audio thread reads here.
//! - **Editors share what they read.** The ring has one consumer, so it lives in the instance's
//!   entry behind a mutex only editor threads take: a watching editor drains it into the entry's
//!   last [`HELD_BLOCKS`] blocks and takes the ones newer than it has seen ([`Instance::read`]),
//!   so two editors watching one track each see every block. When the count of watchers rises
//!   from none, the entry drops what it held and the tap starts a new generation, so audio
//!   buffered before a watch never reads as current.
//! - **Identity**: a random 64-bit id in the host's state ([`IdentityField`], a persisted field,
//!   never a preset's); of two instances holding one id (a duplicated track) the one that had it
//!   first keeps it, and the other gets a new one.
//! - **Names**: the host's track name (CLAP's track-info, through the editor), else "Track n" in
//!   the order instances were made; two of one name get " 2", " 3" ([`names`]).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use nice_plug::params::persist::PersistentField;

use crate::analyser::Block;

/// Blocks an entry keeps for its viewers: 0.34 s at 48 kHz, five frames' worth at 192 kHz.
pub const HELD_BLOCKS: usize = 64;
/// The share tap's ring, in blocks: 43 ms at 192 kHz, more than two frames.
pub const SHARE_BLOCKS: usize = 32;

static REGISTRY: Mutex<Vec<Weak<Instance>>> = Mutex::new(Vec::new());
static MADE: AtomicU64 = AtomicU64::new(0);

fn registry() -> MutexGuard<'static, Vec<Weak<Instance>>> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A new id: never 0, which the view settings read as "no track".
fn fresh_id() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    hasher.write_u128(now);
    hasher.write_u64(MADE.load(Ordering::Relaxed));
    hasher.finish().max(1)
}

/// The instance's id, held in the host's state beside the view settings: a persisted field, not
/// a parameter, so never in a preset, and untouched by Init and undo.
pub struct IdentityField {
    id: Arc<AtomicU64>,
}

impl Default for IdentityField {
    fn default() -> Self {
        Self {
            id: Arc::new(AtomicU64::new(fresh_id())),
        }
    }
}

impl IdentityField {
    pub fn get(&self) -> u64 {
        self.id.load(Ordering::Acquire)
    }

    /// The storage the instance's registry entry shares.
    pub fn shared(&self) -> Arc<AtomicU64> {
        self.id.clone()
    }
}

impl<'a> PersistentField<'a, u64> for IdentityField {
    /// A host's restore: the id saved with the project, unless a live instance already holds it.
    /// The registry stays locked from the check to the store, so two restores at once never both
    /// take one id.
    fn set(&self, new_value: u64) {
        let all = registry();
        let taken = new_value == 0
            || all.iter().filter_map(Weak::upgrade).any(|other| {
                !Arc::ptr_eq(&other.id, &self.id) && other.id.load(Ordering::Acquire) == new_value
            });
        let id = if taken { fresh_id() } else { new_value };
        self.id.store(id, Ordering::Release);
        drop(all);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&u64) -> R,
    {
        f(&self.get())
    }
}

/// One instance's entry.
pub struct Instance {
    id: Arc<AtomicU64>,
    /// The order it was made in, for "Track n".
    made: u64,
    track: Mutex<Option<String>>,
    watchers: AtomicU32,
    feed: Mutex<Feed>,
}

struct Feed {
    consumer: rtrb::Consumer<Block>,
    held: VecDeque<Block>,
}

impl Instance {
    /// A new instance joins the registry, its id in `id` (its [`IdentityField`]'s), the share
    /// tap's ring read through `consumer`.
    pub fn join(id: Arc<AtomicU64>, consumer: rtrb::Consumer<Block>) -> Arc<Instance> {
        let instance = Arc::new(Instance {
            id,
            made: MADE.fetch_add(1, Ordering::Relaxed),
            track: Mutex::new(None),
            watchers: AtomicU32::new(0),
            feed: Mutex::new(Feed {
                consumer,
                held: VecDeque::new(),
            }),
        });
        let mut all = registry();
        all.retain(|weak| weak.strong_count() > 0);
        all.push(Arc::downgrade(&instance));
        instance
    }

    /// An instance outside the registry, for an editor's tests.
    #[cfg(test)]
    pub fn detached() -> Arc<Instance> {
        let (_, consumer) = crate::analyser::ring_with(1);
        Arc::new(Instance {
            id: Arc::new(AtomicU64::new(fresh_id())),
            made: MADE.fetch_add(1, Ordering::Relaxed),
            track: Mutex::new(None),
            watchers: AtomicU32::new(0),
            feed: Mutex::new(Feed {
                consumer,
                held: VecDeque::new(),
            }),
        })
    }

    pub fn id(&self) -> u64 {
        self.id.load(Ordering::Acquire)
    }

    /// The host's name for this instance's track; an empty one is none.
    pub fn set_track(&self, name: &str) {
        *self.track.lock().unwrap_or_else(PoisonError::into_inner) =
            (!name.is_empty()).then(|| name.to_owned());
    }

    /// Whether an editor watches it: the audio thread's one read here.
    pub fn watched(&self) -> bool {
        self.watchers.load(Ordering::Acquire) > 0
    }

    /// A watch, held while an editor shows this track; dropped, the count goes back. The first
    /// drops whatever was buffered before it: the check, the clearing and the count all under the
    /// feed's lock, so a second watch at once never clears what the first has begun to read.
    pub fn watch(self: &Arc<Self>) -> Watch {
        let mut feed = self.feed();
        if self.watchers.load(Ordering::Acquire) == 0 {
            while feed.consumer.pop().is_ok() {}
            feed.held.clear();
        }
        self.watchers.fetch_add(1, Ordering::AcqRel);
        drop(feed);
        Watch {
            instance: Arc::downgrade(self),
        }
    }

    /// The blocks newer than `after`, in order, through `each`, after draining the ring into the
    /// held ones; `after` moves to the newest. A viewer that fell behind the held blocks gets them
    /// from the oldest, a gap its analyser restarts on.
    pub fn read(&self, after: &mut Option<u64>, mut each: impl FnMut(&Block)) {
        let mut feed = self.feed();
        let Feed { consumer, held } = &mut *feed;
        while let Ok(block) = consumer.pop() {
            if held.len() == HELD_BLOCKS {
                held.pop_front();
            }
            held.push_back(block);
        }
        for block in held.iter() {
            if after.is_none_or(|seen| block.seq > seen) {
                each(block);
                *after = Some(block.seq);
            }
        }
    }

    fn feed(&self) -> MutexGuard<'_, Feed> {
        self.feed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// An editor's watch of one instance; dropping it ends the watch.
pub struct Watch {
    instance: Weak<Instance>,
}

impl Drop for Watch {
    /// Under the feed's lock, as [`Instance::watch`] counts, so a last watch ending and a new one
    /// beginning never interleave: the new one either finds the old still counted or starts from
    /// none and clears.
    fn drop(&mut self) {
        if let Some(instance) = self.instance.upgrade() {
            let _feed = instance.feed();
            instance.watchers.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// The live instance with `id`.
pub fn find(id: u64) -> Option<Arc<Instance>> {
    registry()
        .iter()
        .filter_map(Weak::upgrade)
        .find(|instance| instance.id() == id)
}

/// A listed instance: its id, its name, and its entry, held weakly.
pub struct Listed {
    pub id: u64,
    pub name: String,
    pub entry: Weak<Instance>,
}

/// Every live instance's name, in the order they were made: the host's track name, else
/// "Track n" by that order; a name already shown gets the lowest free " 2", " 3", so no two read
/// alike.
pub fn names() -> Vec<Listed> {
    let mut live: Vec<Arc<Instance>> = registry().iter().filter_map(Weak::upgrade).collect();
    live.sort_by_key(|instance| instance.made);
    let mut listed: Vec<Listed> = Vec::with_capacity(live.len());
    for (place, instance) in live.iter().enumerate() {
        let base = instance
            .track
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| format!("Track {}", place + 1));
        let used = |name: &str| listed.iter().any(|other| other.name == name);
        let name = if used(&base) {
            (2..)
                .map(|n| format!("{base} {n}"))
                .find(|name| !used(name))
                .unwrap_or(base)
        } else {
            base
        };
        listed.push(Listed {
            id: instance.id(),
            name,
            entry: Arc::downgrade(instance),
        });
    }
    listed
}

/// The instances an editor lists: every one but `own`, alphabetically.
pub fn others(own: &Instance) -> Vec<Listed> {
    let mut listed: Vec<Listed> = names()
        .into_iter()
        .filter(|listed| {
            listed
                .entry
                .upgrade()
                .is_some_and(|entry| !Arc::ptr_eq(&entry.id, &own.id))
        })
        .collect();
    listed.sort_by_cached_key(|listed| listed.name.to_lowercase());
    listed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyser::{BLOCK_FRAMES, Tap, ring_with};

    fn joined() -> (Arc<Instance>, Tap, IdentityField) {
        let identity = IdentityField::default();
        let (tap, consumer) = ring_with(SHARE_BLOCKS);
        let instance = Instance::join(identity.shared(), consumer);
        (instance, tap, identity)
    }

    /// An instance joins the registry when it is made and leaves it when it is dropped.
    #[test]
    fn an_instance_joins_and_leaves() {
        let (instance, _tap, identity) = joined();
        let id = instance.id();
        assert_eq!(id, identity.get());
        assert!(find(id).is_some_and(|found| Arc::ptr_eq(&found, &instance)));
        drop(instance);
        assert!(find(id).is_none());
    }

    /// **A duplicated track's copy gets a new id** (plan §7.8): the instance that held it first
    /// keeps it; a free id restored is taken; 0, "no track", never is.
    #[test]
    fn a_duplicated_id_is_renewed_for_the_later_instance() {
        let (first, _a, _first_identity) = joined();
        let (second, _b, identity) = joined();
        PersistentField::set(&identity, first.id());
        assert_ne!(second.id(), first.id());
        assert_ne!(second.id(), 0);
        let free = second.id() ^ 0x5a5a_5a5a;
        PersistentField::set(&identity, free);
        assert_eq!(second.id(), free);
        PersistentField::set(&identity, 0);
        assert_ne!(second.id(), 0);
    }

    /// **A choice is found after a reload whichever instance loads first** (plan §7.8): the id
    /// comes back with the track's state.
    #[test]
    fn a_choice_is_found_after_a_reload_whichever_loads_first() {
        for viewer_first in [true, false] {
            let saved = joined().0.id();
            assert!(find(saved).is_none(), "gone with its project");
            let track = || {
                let (track, tap, identity) = joined();
                PersistentField::set(&identity, saved);
                (track, tap, identity)
            };
            let (viewer, track) = if viewer_first {
                let viewer = joined();
                (viewer, track())
            } else {
                let track = track();
                (joined(), track)
            };
            assert!(find(saved).is_some_and(|found| Arc::ptr_eq(&found, &track.0)));
            assert_ne!(viewer.0.id(), saved);
        }
    }

    /// **Names**: the host's track name, else "Track n"; two of one name get " 2"; a rename shows
    /// at once; an editor lists every other instance alphabetically, never its own.
    #[test]
    fn names_come_from_the_host_with_a_fallback_and_suffixes() {
        let (a, _ta, _ia) = joined();
        let (b, _tb, _ib) = joined();
        let (c, _tc, _ic) = joined();
        a.set_track("zz-names Bass");
        b.set_track("zz-names Bass");
        let name = |instance: &Arc<Instance>| {
            names()
                .into_iter()
                .find(|listed| listed.id == instance.id())
                .map(|listed| listed.name)
                .unwrap()
        };
        assert_eq!(name(&a), "zz-names Bass");
        assert_eq!(name(&b), "zz-names Bass 2");
        // A host's own "Drum 2" made first: the next two read "Drum" and "Drum 3", never alike.
        let (d, _td, _id) = joined();
        let (e, _te, _ie) = joined();
        let (f, _tf, _if) = joined();
        d.set_track("zz-names Drum 2");
        e.set_track("zz-names Drum");
        f.set_track("zz-names Drum");
        assert_eq!(
            [name(&d), name(&e), name(&f)],
            ["zz-names Drum 2", "zz-names Drum", "zz-names Drum 3"]
        );
        let fallback = name(&c);
        let number: usize = fallback.strip_prefix("Track ").unwrap().parse().unwrap();
        assert!(number >= 1, "{fallback}");
        b.set_track("zz-names Keys");
        assert_eq!(name(&b), "zz-names Keys");
        let listed = others(&a);
        assert!(listed.iter().all(|listed| listed.id != a.id()));
        assert!(listed.iter().any(|listed| listed.id == b.id()));
        let keys: Vec<String> = listed.iter().map(|l| l.name.to_lowercase()).collect();
        assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]), "{keys:?}");
    }

    /// **Restores at once never share an id** (plan §7.8): eight instances restoring the same id
    /// in parallel, exactly one holds it.
    #[test]
    fn restores_at_once_never_share_an_id() {
        let all: Vec<_> = (0..8).map(|_| joined()).collect();
        let wanted = fresh_id();
        let barrier = std::sync::Barrier::new(all.len());
        let identities: Vec<&IdentityField> = all.iter().map(|(_, _, identity)| identity).collect();
        std::thread::scope(|scope| {
            for identity in identities {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    PersistentField::set(identity, wanted);
                });
            }
        });
        let holders = all
            .iter()
            .filter(|(instance, _, _)| instance.id() == wanted)
            .count();
        assert_eq!(holders, 1);
    }

    /// **Watches made at once all count** (plan §7.8), and what comes after them reaches a viewer.
    #[test]
    fn watches_made_at_once_all_count() {
        let (instance, mut tap, _identity) = joined();
        let barrier = std::sync::Barrier::new(8);
        let watches: Vec<Watch> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let (instance, barrier) = (&instance, &barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        instance.watch()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });
        assert_eq!(instance.watchers.load(Ordering::Acquire), 8);
        let frames = [0.25f32; BLOCK_FRAMES];
        tap.set_wanted(true);
        for _ in 0..4 {
            tap.capture_post(2, [&frames, &frames]);
        }
        let mut seen = 0;
        instance.read(&mut None, |_| seen += 1);
        assert_eq!(seen, 4);
        drop(watches);
        assert!(!instance.watched());
    }

    /// **A watch ends under the feed's lock** (plan §7.8), as one begins: while the lock is held
    /// elsewhere, a watch dropped on another thread cannot lower the count, so a new watch can never
    /// see an old one's count and then lose it.
    #[test]
    fn a_watch_ends_under_the_feeds_lock() {
        let (instance, _tap, _identity) = joined();
        let watch = instance.watch();
        let feed = instance.feed();
        std::thread::scope(|scope| {
            let ending = scope.spawn(move || drop(watch));
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert!(!ending.is_finished(), "the drop waits for the lock");
            assert_eq!(instance.watchers.load(Ordering::Acquire), 1);
            drop(feed);
            ending.join().unwrap();
        });
        assert!(!instance.watched());
    }

    /// **A last watch ending as a new one begins** (plan §7.8): over many handoffs from two threads
    /// at once, the count is always the watches held.
    #[test]
    fn a_last_watch_ending_as_a_new_one_begins_keeps_count() {
        let (instance, _tap, _identity) = joined();
        for _ in 0..200 {
            let old = instance.watch();
            let barrier = std::sync::Barrier::new(2);
            let new = std::thread::scope(|scope| {
                let ending = scope.spawn(|| {
                    barrier.wait();
                    drop(old);
                });
                let beginning = scope.spawn(|| {
                    barrier.wait();
                    instance.watch()
                });
                ending.join().unwrap();
                beginning.join().unwrap()
            });
            assert_eq!(instance.watchers.load(Ordering::Acquire), 1);
            drop(new);
            assert!(!instance.watched());
        }
    }

    /// **Viewers share what they read** (plan §7.8): what was buffered before the first watch is
    /// dropped; two viewers each see every block; one that fell behind the held blocks gets them
    /// from the oldest, a gap; the watch ends with its guard.
    #[test]
    fn viewers_share_what_they_read() {
        let (instance, mut tap, _identity) = joined();
        let frames = [0.25f32; BLOCK_FRAMES];
        tap.set_wanted(true);
        for _ in 0..4 {
            tap.capture_post(2, [&frames, &frames]);
        }
        let watch = instance.watch();
        assert!(instance.watched());
        let (mut first, mut second) = (None, None);
        let (mut seen_first, mut seen_second) = (Vec::new(), Vec::new());
        instance.read(&mut first, |block| seen_first.push(block.seq));
        assert!(seen_first.is_empty(), "nothing from before the watch");
        for round in 0..10 {
            for _ in 0..8 {
                tap.capture_post(2, [&frames, &frames]);
            }
            instance.read(&mut first, |block| seen_first.push(block.seq));
            if round % 2 == 1 {
                instance.read(&mut second, |block| seen_second.push(block.seq));
            }
        }
        assert_eq!(seen_first.len(), 80);
        assert!(seen_first.windows(2).all(|pair| pair[1] == pair[0] + 1));
        assert_eq!(seen_second, seen_first);
        for _ in 0..12 {
            for _ in 0..8 {
                tap.capture_post(2, [&frames, &frames]);
            }
            instance.read(&mut first, |_| {});
        }
        let mut late = Vec::new();
        instance.read(&mut second, |block| late.push(block.seq));
        assert_eq!(late.len(), HELD_BLOCKS);
        assert!(late[0] > seen_second.last().unwrap() + 1, "a gap");
        drop(watch);
        assert!(!instance.watched());
    }
}
