//! Auto gain (plan §7.8, decisions 38 and 39): the loudness change the drawn curve makes on an
//! average mix, undone at the output. **An estimate from the curve**, as Pro-Q's is, never a
//! measurement of the signal: no latency, no pumping, and a bounce the same every time.
//!
//! At the points `f_k = 20·2^(k/24)` Hz up to the lower of 20 kHz and 0.45 of the rate, the
//! curve's power gain is weighed by:
//! - **the average mix's power per hertz**, the research's corpus mean
//!   (`research:effects/parametric-equalizers.md` §10.6): per FFT bin falling 5.79 dB an octave
//!   above 94 Hz (Elowsson & Friberg's line), and below it falling 12 dB an octave, a model of
//!   Pestana's rise to 90–100 Hz and low cut round 60 Hz, not a measurement;
//! - **BS.1770's K-weighting**, its 48 kHz biquads' response at `ω = 2π·f/48,000` whatever the
//!   session rate (`research:listening/perceptual-measures.md` §9): it weighs, it filters nothing;
//! - **the point's share of the axis**, f, the points being log-spaced.
//!
//! Placed bands count by their share of an uncorrelated stereo signal: with the channel stage's
//! powers l, r and the mid/side stage's m, s, the power gain is `((l + r)/2)·((m + s)/2)`; in mono
//! only Stereo bands are in the curve, and it is their power.

use crate::OUTPUT_GAIN_RANGE_DB;
use crate::design::tan_half;
use crate::linear::{Heard, stage_powers_at};

/// The make-up's range: the output gain's own, ±24 dB.
pub const MAKE_UP_RANGE_DB: f64 = OUTPUT_GAIN_RANGE_DB;

const LOWEST_HZ: f64 = 20.0;
const HIGHEST_HZ: f64 = 20_000.0;
/// The highest point's share of the rate, below the top octave's cramping.
const HIGHEST_SHARE: f64 = 0.45;
const POINTS_PER_OCTAVE: f64 = 24.0;
/// Where the corpus mix's per-bin level turns, and its falls either side, in dB an octave.
const TURN_HZ: f64 = 94.0;
const FALL_ABOVE_DB: f64 = 5.79;
const FALL_BELOW_DB: f64 = 12.0;
/// BS.1770-5's K-weighting at 48 kHz, the shelf then the high pass, as `[b0, b1, b2, a1, a2]`.
const K_STAGES: [[f64; 5]; 2] = [
    [
        1.535_124_859_586_97,
        -2.691_696_189_406_38,
        1.198_392_810_852_85,
        -1.690_659_293_182_41,
        0.732_480_774_215_85,
    ],
    [1.0, -2.0, 1.0, -1.990_047_454_833_98, 0.990_072_250_366_21],
];
const K_RATE: f64 = 48_000.0;

/// The points and their weights for one session rate, made when the engine is built.
pub struct AutoGain {
    /// Each point's `tan(π·f/fs)` at the session rate, and its weight.
    points: Vec<(f64, f64)>,
    total: f64,
}

impl AutoGain {
    pub fn new(sample_rate: f64) -> AutoGain {
        let sample_rate = crate::usable_sample_rate(sample_rate);
        let top = HIGHEST_HZ.min(HIGHEST_SHARE * sample_rate);
        let count = if top > LOWEST_HZ {
            (POINTS_PER_OCTAVE * (top / LOWEST_HZ).log2()).floor() as usize + 1
        } else {
            1
        };
        let points: Vec<(f64, f64)> = (0..count)
            .map(|k| {
                let freq = LOWEST_HZ * (k as f64 / POINTS_PER_OCTAVE).exp2();
                (tan_half(freq, sample_rate), weight(freq))
            })
            .collect();
        let total = points.iter().map(|(_, weight)| weight).sum();
        AutoGain { points, total }
    }

    /// How many points the estimate reads.
    pub fn points(&self) -> usize {
        self.points.len()
    }

    /// The make-up for `heard`, in dB, held inside ±[`MAKE_UP_RANGE_DB`]: exactly 0 for an empty
    /// EQ, which sums nothing.
    pub fn make_up_db(&self, heard: &Heard) -> f64 {
        if heard.bands.iter().all(Option::is_none) {
            return 0.0;
        }
        self.make_up_for(|t| {
            let [l, r, m, s] = stage_powers_at(heard, t);
            0.25 * (l + r) * (m + s)
        })
    }

    /// The make-up for a curve whose power gain at the point whose `tan(π·f/fs)` is `t` is
    /// `power(t)`, held inside ±[`MAKE_UP_RANGE_DB`].
    pub fn make_up_for(&self, power: impl Fn(f64) -> f64) -> f64 {
        let sum: f64 = self
            .points
            .iter()
            .map(|&(t, weight)| weight * power(t))
            .sum();
        let db = -10.0 * (sum / self.total).log10();
        if db.is_nan() {
            0.0
        } else {
            db.clamp(-MAKE_UP_RANGE_DB, MAKE_UP_RANGE_DB)
        }
    }
}

/// A point's weight: the corpus mix's power per hertz, K-weighted, times the point's share of the
/// log axis.
fn weight(freq: f64) -> f64 {
    let octaves = (freq / TURN_HZ).log2();
    let level_db = if octaves >= 0.0 {
        -FALL_ABOVE_DB * octaves
    } else {
        FALL_BELOW_DB * octaves
    };
    10f64.powf(level_db / 10.0) * k_weighting_power(freq) * freq
}

/// BS.1770's K-weighting as a power gain at `freq`.
pub fn k_weighting_power(freq: f64) -> f64 {
    let w = std::f64::consts::TAU * freq / K_RATE;
    let (c1, s1) = (w.cos(), -w.sin());
    let (c2, s2) = ((2.0 * w).cos(), -(2.0 * w).sin());
    K_STAGES
        .iter()
        .map(|[b0, b1, b2, a1, a2]| {
            let (nr, ni) = (b0 + b1 * c1 + b2 * c2, b1 * s1 + b2 * s2);
            let (dr, di) = (1.0 + a1 * c1 + a2 * c2, a1 * s1 + a2 * s2);
            (nr * nr + ni * ni) / (dr * dr + di * di)
        })
        .product()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BS.1770's K-weighting as the research reads it (§9): −13.3 dB at 20 Hz, −3.9 at 50, −1.1
    /// at 100, +0.69 at 1 kHz, +4.0 from 4 kHz up.
    #[test]
    fn the_k_weighting_is_the_standards() {
        let db = |freq: f64| 10.0 * k_weighting_power(freq).log10();
        for (freq, want) in [
            (20.0, -13.28),
            (50.0, -3.93),
            (100.0, -1.13),
            (997.0, 0.691),
            (4_000.0, 3.97),
        ] {
            assert!((db(freq) - want).abs() < 0.02, "{freq} Hz: {}", db(freq));
        }
    }

    /// The grid (plan §7.8): 239 points at 44.1 kHz, 240 from 48 kHz up.
    #[test]
    fn the_points_follow_the_rate() {
        assert_eq!(AutoGain::new(44_100.0).points(), 239);
        for rate in [48_000.0, 96_000.0, 192_000.0] {
            assert_eq!(AutoGain::new(rate).points(), 240);
        }
    }
}
