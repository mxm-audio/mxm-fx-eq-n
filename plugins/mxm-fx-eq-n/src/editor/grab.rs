//! F3's spectrum grab (plan §7.4): the two-second hold a freeze shows, the peaks it labels, and the
//! Q a grab gives the bell it makes. Pure functions of the display's own levels — the 1/12-octave
//! smoothed spectrum at the graph's frequencies — so what is labelled is what is drawn.

use std::collections::VecDeque;

/// How long the hold reaches back when Freeze is turned on.
pub const HOLD_SECONDS: f64 = 2.0;
/// How far a peak stands above the higher of its two key points to count.
pub const PROMINENCE_DB: f32 = 6.0;
/// How far each side a peak's key point is looked for.
pub const KEY_OCTAVES: f32 = 1.0 / 3.0;
/// How far under the frozen spectrum's loudest band a peak may lie.
pub const LEVEL_WINDOW_DB: f32 = 40.0;
/// The closest two labelled peaks may stand.
pub const SPACING_OCTAVES: f32 = 1.0 / 6.0;
/// The most peaks labelled.
pub const MAX_PEAKS: usize = 8;
/// A grabbed bell's Q, from the peak's 3 dB width, is held inside this.
pub const Q_RANGE: (f32, f32) = (0.5, 12.0);

/// The display's levels over the last [`HOLD_SECONDS`], frame by frame, so a freeze can hold each
/// band's highest of them. Its buffers are reused, so a running hold allocates nothing once full.
#[derive(Default)]
pub struct Hold {
    frames: VecDeque<(f64, Vec<f32>)>,
    spare: Vec<Vec<f32>>,
}

impl Hold {
    /// One frame of levels at time `now`, in seconds. A frame of another length (the graph was
    /// resized) starts the hold again.
    pub fn push(&mut self, now: f64, levels: &[f32]) {
        if self
            .frames
            .front()
            .is_some_and(|(_, frame)| frame.len() != levels.len())
        {
            self.clear();
        }
        while self
            .frames
            .front()
            .is_some_and(|(time, _)| now - time > HOLD_SECONDS)
        {
            if let Some((_, frame)) = self.frames.pop_front() {
                self.spare.push(frame);
            }
        }
        let mut frame = self.spare.pop().unwrap_or_default();
        frame.clear();
        frame.extend_from_slice(levels);
        self.frames.push_back((now, frame));
    }

    /// Each band's highest level over the frames held; empty with none.
    pub fn highest(&self) -> Vec<f32> {
        let mut frames = self.frames.iter().map(|(_, frame)| frame);
        let Some(first) = frames.next() else {
            return Vec::new();
        };
        let mut highest = first.clone();
        for frame in frames {
            for (high, level) in highest.iter_mut().zip(frame) {
                *high = high.max(*level);
            }
        }
        highest
    }

    pub fn clear(&mut self) {
        while let Some((_, frame)) = self.frames.pop_front() {
            self.spare.push(frame);
        }
    }
}

/// What levels were read under (plan §7.4): the analyser's settings and the audio's generation.
/// The hold mixes levels of one basis only, and a freeze ends when its basis changes, so held
/// levels are never shown as another source or under another transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    pub view: crate::view::ViewSettings,
    pub generation: Option<u32>,
}

/// A freeze (plan §7.4): one source and one representation, held until a setting, the size, a
/// reset or a restore moves under it. The editor's moment, never saved.
pub struct Frozen {
    /// What it was frozen under; any change ends it.
    pub basis: Basis,
    /// Whether the grabbed source is the post-EQ spectrum (else the pre-EQ one).
    pub source_post: bool,
    /// Each line as drawn while frozen: the source's two-second hold, and the other line's last
    /// frame when it was shown (empty otherwise).
    pub post: Vec<f32>,
    pub pre: Vec<f32>,
    /// The side chain's line as it was, held with the rest (empty with its view off).
    pub key: Vec<f32>,
    /// The source's peaks, strongest first.
    pub peaks: Vec<Peak>,
    /// The frequencies its lines were read at: the graph's grid when it was made.
    pub freqs: Vec<f32>,
}

impl Frozen {
    /// Whether `basis` still describes it and the graph still has its grid.
    pub fn holds(&self, basis: &Basis, freqs: &[f32]) -> bool {
        self.basis == *basis && self.drawn_on(freqs)
    }

    /// Whether the graph's grid is the one it was made on: any resize moves the grid.
    pub fn drawn_on(&self, freqs: &[f32]) -> bool {
        self.freqs.as_slice() == freqs
    }
}

/// Which labels are drawn (plan §7.4): `rects` strongest first, each kept unless it overlaps a
/// stronger one that was kept.
pub fn unobstructed(rects: &[egui::Rect]) -> Vec<bool> {
    let mut kept: Vec<egui::Rect> = Vec::new();
    rects
        .iter()
        .map(|rect| {
            let free = kept.iter().all(|other| !other.intersects(*rect));
            if free {
                kept.push(*rect);
            }
            free
        })
        .collect()
}

/// One labelled peak of a frozen spectrum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    /// Its place among the spectrum's frequencies.
    pub index: usize,
    pub freq: f32,
    pub db: f32,
    pub prominence: f32,
    /// The bell a grab makes of it: `f0 / (f_high − f_low)` at its 3 dB points, held in
    /// [`Q_RANGE`].
    pub q: f32,
}

/// The peaks of `db` at `freqs` (ascending), strongest first (plan §7.4): a local maximum standing
/// [`PROMINENCE_DB`] above the higher of its two key points within [`KEY_OCTAVES`] each side, so
/// the spectrum falls that far on both sides; within [`LEVEL_WINDOW_DB`] of the loudest band and
/// above `floor_db`, the bottom of the range the graph draws; [`SPACING_OCTAVES`] from a stronger
/// one; at most [`MAX_PEAKS`].
pub fn peaks(freqs: &[f32], db: &[f32], floor_db: f32) -> Vec<Peak> {
    let count = freqs.len().min(db.len());
    let Some(loudest) = db[..count].iter().copied().reduce(f32::max) else {
        return Vec::new();
    };
    let mut found: Vec<Peak> = Vec::new();
    for index in 0..count {
        let level = db[index];
        if level < loudest - LEVEL_WINDOW_DB || level <= floor_db {
            continue;
        }
        // A local maximum; the first band of a plateau stands for it.
        let left_lower = index == 0 || db[index - 1] < level;
        let right_not_higher = index + 1 >= count || db[index + 1] <= level;
        if !(left_lower && right_not_higher) {
            continue;
        }
        let (Some(left), Some(right)) = (
            key_point(freqs, db, index, Side::Below),
            key_point(freqs, db, index, Side::Above),
        ) else {
            continue;
        };
        let prominence = level - left.max(right);
        if prominence < PROMINENCE_DB {
            continue;
        }
        let Some(q) = q_of(freqs, db, index) else {
            continue;
        };
        found.push(Peak {
            index,
            freq: freqs[index],
            db: level,
            prominence,
            q,
        });
    }
    found.sort_by(|a, b| b.prominence.total_cmp(&a.prominence));
    let mut kept: Vec<Peak> = Vec::new();
    for peak in found {
        let near = kept
            .iter()
            .any(|other| (peak.freq / other.freq).log2().abs() < SPACING_OCTAVES);
        if !near {
            kept.push(peak);
        }
        if kept.len() == MAX_PEAKS {
            break;
        }
    }
    kept
}

#[derive(Clone, Copy)]
enum Side {
    Below,
    Above,
}

/// The lowest level between the peak at `index` and the next higher level on `side`, within
/// [`KEY_OCTAVES`]. Where the spectrum's edge comes first, the lowest seen counts once it is
/// already [`PROMINENCE_DB`] down (the unseen rest could only lower it); short of that, `None`.
fn key_point(freqs: &[f32], db: &[f32], index: usize, side: Side) -> Option<f32> {
    let level = db[index];
    let reach = 2f32.powf(KEY_OCTAVES);
    let mut lowest = level;
    let mut at = index;
    loop {
        let next = match side {
            Side::Below => at.checked_sub(1),
            Side::Above => Some(at + 1).filter(|&next| next < db.len().min(freqs.len())),
        };
        let Some(next) = next else {
            return (level - lowest >= PROMINENCE_DB).then_some(lowest);
        };
        let ratio = match side {
            Side::Below => freqs[index] / freqs[next],
            Side::Above => freqs[next] / freqs[index],
        };
        // A point a third of an octave away counts, whatever rounding puts its ratio a hair over.
        if ratio > reach * (1.0 + 1.0e-4) {
            return Some(lowest);
        }
        if db[next] > level {
            return Some(lowest);
        }
        lowest = lowest.min(db[next]);
        at = next;
    }
}

/// The Q of the bell a grab makes of the peak at `index`: its first 3 dB points each side,
/// interpolated in log-frequency, held in [`Q_RANGE`]. `None` if a side never falls 3 dB.
pub fn q_of(freqs: &[f32], db: &[f32], index: usize) -> Option<f32> {
    let target = db[index] - 3.0;
    let crossing = |side: Side| -> Option<f32> {
        let mut at = index;
        loop {
            let next = match side {
                Side::Below => at.checked_sub(1)?,
                Side::Above => Some(at + 1).filter(|&next| next < db.len().min(freqs.len()))?,
            };
            if db[next] <= target {
                // Between `at` (above the target) and `next` (at or below it), in log-frequency.
                let t = (db[at] - target) / (db[at] - db[next]).max(f32::MIN_POSITIVE);
                let (a, b) = (freqs[at].log2(), freqs[next].log2());
                return Some(2f32.powf(a + (b - a) * t));
            }
            at = next;
        }
    };
    let low = crossing(Side::Below)?;
    let high = crossing(Side::Above)?;
    let q = freqs[index] / (high - low).max(f32::MIN_POSITIVE);
    Some(q.clamp(Q_RANGE.0, Q_RANGE.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1/24-octave points from 20 Hz to 20 kHz, the graph's spacing near enough.
    fn axis() -> Vec<f32> {
        (0..=240)
            .map(|step| 20.0 * 2f32.powf(step as f32 / 24.0))
            .collect()
    }

    /// A flat floor at `base` with bells: (centre, height in dB, Q).
    fn spectrum(freqs: &[f32], base: f32, bells: &[(f32, f32, f32)]) -> Vec<f32> {
        freqs
            .iter()
            .map(|&f| {
                bells.iter().fold(base, |level, &(centre, height, q)| {
                    let x = (f / centre).log2() * q * 2.0;
                    level.max(base + height / (1.0 + x * x))
                })
            })
            .collect()
    }

    /// A second-order resonance of `q` at `centre`, `height` dB over a floor at `base`, in power:
    /// its 3 dB width is `centre / q` once it stands well clear of the floor.
    fn resonance(freqs: &[f32], base: f32, centre: f32, height: f32, q: f32) -> Vec<f32> {
        let (floor, top) = (10f32.powf(base / 10.0), 10f32.powf((base + height) / 10.0));
        freqs
            .iter()
            .map(|&f| {
                let detune = q * (f / centre - centre / f);
                10.0 * (floor + top / (1.0 + detune * detune)).log10()
            })
            .collect()
    }

    #[test]
    fn a_clear_peak_is_found_with_its_frequency_and_a_q_from_its_width() {
        let freqs = axis();
        // A peak must fall 6 dB within a third of an octave each side: a resonance does at a detune
        // of √3, so the rule sees Q above about 3.7, narrower than 0.4 octave at its 3 dB points.
        for q in [5.0, 7.0, 11.0] {
            let db = resonance(&freqs, -60.0, 1_000.0, 30.0, q);
            let found = peaks(&freqs, &db, -96.0);
            assert_eq!(found.len(), 1, "q {q}");
            let peak = found[0];
            assert!((peak.freq / 1_000.0).log2().abs() < 1.0 / 24.0);
            assert!(peak.prominence >= PROMINENCE_DB, "{}", peak.prominence);
            assert!(
                (peak.q / q - 1.0).abs() < 0.15,
                "built {q}, measured {}",
                peak.q
            );
        }
    }

    #[test]
    fn prominence_is_taken_against_the_higher_key_point() {
        let freqs = axis();
        // A shoulder on a slope: deep on its low side, hardly falling on its high side.
        let db: Vec<f32> = freqs
            .iter()
            .map(|&f| {
                let slope = -60.0 + 30.0 * (f / 20.0).log2() / 10.0;
                let x = (f / 1_000.0).log2() * 8.0;
                slope + 2.0 / (1.0 + x * x)
            })
            .collect();
        assert!(
            peaks(&freqs, &db, -96.0).is_empty(),
            "a shoulder is not a peak"
        );
    }

    #[test]
    fn a_maximum_at_the_edge_is_not_a_peak() {
        let freqs = axis();
        let db = spectrum(&freqs, -60.0, &[(20.0, 20.0, 1.0)]);
        assert!(
            peaks(&freqs, &db, -96.0)
                .iter()
                .all(|peak| peak.freq > 30.0)
        );
    }

    #[test]
    fn peaks_far_under_the_loudest_band_or_below_the_floor_are_not_labelled() {
        let freqs = axis();
        let db = spectrum(&freqs, -100.0, &[(200.0, 80.0, 4.0), (5_000.0, 30.0, 8.0)]);
        let found = peaks(&freqs, &db, -96.0);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!((found[0].freq / 200.0).log2().abs() < 0.1);
    }

    #[test]
    fn close_peaks_keep_the_stronger_and_at_most_eight_are_labelled() {
        let freqs = axis();
        let db = spectrum(
            &freqs,
            -60.0,
            &[(1_000.0, 20.0, 12.0), (1_080.0, 12.0, 12.0)],
        );
        let found = peaks(&freqs, &db, -96.0);
        assert_eq!(found.len(), 1, "a sixth of an octave apart at least");
        let many: Vec<(f32, f32, f32)> = (0..12)
            .map(|n| (50.0 * 1.6f32.powi(n), 10.0 + n as f32, 12.0))
            .collect();
        let db = spectrum(&freqs, -60.0, &many);
        let found = peaks(&freqs, &db, -96.0);
        assert_eq!(found.len(), MAX_PEAKS);
        for pair in found.windows(2) {
            assert!(pair[0].prominence >= pair[1].prominence, "strongest first");
        }
    }

    #[test]
    fn q_follows_the_width_and_is_held_in_its_range() {
        let freqs = axis();
        let narrow = spectrum(&freqs, -60.0, &[(1_000.0, 30.0, 40.0)]);
        let wide = spectrum(&freqs, -60.0, &[(1_000.0, 30.0, 0.3)]);
        let at = freqs.iter().position(|&f| f >= 1_000.0).unwrap();
        let (narrow_q, wide_q) = (
            q_of(&freqs, &narrow, at).unwrap(),
            q_of(&freqs, &wide, at).unwrap(),
        );
        assert!(narrow_q > wide_q);
        assert!(narrow_q <= Q_RANGE.1 && wide_q >= Q_RANGE.0);
    }

    #[test]
    fn the_hold_keeps_each_band_s_highest_of_the_last_two_seconds() {
        let mut hold = Hold::default();
        hold.push(0.0, &[-10.0, -50.0]);
        hold.push(1.0, &[-40.0, -20.0]);
        assert_eq!(hold.highest(), vec![-10.0, -20.0]);
        hold.push(2.5, &[-30.0, -30.0]);
        assert_eq!(
            hold.highest(),
            vec![-30.0, -20.0],
            "the first frame has aged out"
        );
        hold.push(2.6, &[-1.0]);
        assert_eq!(hold.highest(), vec![-1.0], "another length starts again");
    }

    /// **A label that would overlap a stronger one's is not drawn** (plan §7.4); one clear of it is.
    #[test]
    fn a_label_overlapping_a_stronger_one_is_dropped() {
        let at = |x: f32| egui::Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(40.0, 12.0));
        assert_eq!(
            unobstructed(&[at(100.0), at(120.0), at(150.0), at(130.0)]),
            [true, false, true, false],
            "the second overlaps the first, the fourth the third"
        );
    }

    /// **A peak near the edge counts when it falls 6 dB before the edge** (plan §7.4): a narrow
    /// resonance at 18 kHz or 22 Hz is a peak; a broad one there, cut off by the edge, is not.
    #[test]
    fn a_peak_near_the_edge_counts_when_it_falls_before_the_edge() {
        let freqs = axis();
        for (centre, q, found) in [
            (18_000.0, 20.0, true),
            (22.0, 20.0, true),
            (18_000.0, 1.5, false),
            (22.0, 1.5, false),
        ] {
            let db = resonance(&freqs, -60.0, centre, 30.0, q);
            let near = peaks(&freqs, &db, -96.0)
                .iter()
                .any(|peak| (peak.freq / centre).log2().abs() < 0.1);
            assert_eq!(near, found, "{centre} Hz, Q {q}");
        }
    }
}
