//! A dynamic band's law and ballistics (plan §7.1; `docs/briefs/mxm-fx-eq-n.md` §C, signed by the
//! owner 2026-10-14): the controls are Pro-Q's, range only — Threshold, Range, Attack, Release — and
//! a band is static while its Range is 0.
//!
//! - **The law** ([`law`]): above Threshold, through a [`KNEE_DB`] soft knee, the gain moves at a
//!   fixed slope — [`CUT_SLOPE`] (3:1) for a cut, [`LIFT_SLOPE`] (1:1.5) for a lift — and eases into
//!   Range: with `a` the movement that slope asks for, the moving part is
//!   `Range · (1 − e^(−a/|Range|))`. So it never passes Range, a louder band never comes out quieter,
//!   and at any level more Range never means less action.
//! - **The ballistics** ([`Ballistics::step`]) smooth the moving part itself, signed, in dB: Attack
//!   while it moves away from rest, Release while it moves back toward rest or across it, so a lift
//!   is timed as a cut is. One pole per direction, so the state only ever lies between where it was
//!   and where the law asks. A release ends at exactly 0 below [`REST_DB`].
//!
//! The level the law reads is the band's region (`design::design_solo`), linked across channels,
//! as a peak over each control interval (`engine`).

/// Range's extent, ±dB (brief §C): below 0 cuts what gets loud, above 0 lifts it.
pub const RANGE_DB_MAX: f64 = 30.0;

/// Threshold's range in dB (brief §C).
pub const THRESHOLD_DB_MIN: f64 = -60.0;
pub const THRESHOLD_DB_MAX: f64 = 0.0;

/// Attack's and Release's ranges in milliseconds (brief §C).
pub const ATTACK_MS_MIN: f64 = 0.5;
pub const ATTACK_MS_MAX: f64 = 200.0;
pub const RELEASE_MS_MIN: f64 = 10.0;
pub const RELEASE_MS_MAX: f64 = 2_000.0;

/// The soft knee's width, centred on Threshold (brief §C).
pub const KNEE_DB: f64 = 6.0;

/// The gain's movement per dB of level above the knee: 3:1 for a cut (`1 − 1/3`) and 1:1.5 for a
/// lift (`1.5 − 1`) (brief §C).
pub const CUT_SLOPE: f64 = 2.0 / 3.0;
pub const LIFT_SLOPE: f64 = 0.5;

/// Below this a moving part released toward rest is at rest, exactly 0: a ten-thousandth of a dB,
/// far under anything audible, and the point after which a host may sleep the plugin.
pub const REST_DB: f64 = 1.0e-4;

/// A slot's dynamics as the host and the editor set them, plain values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dynamics {
    pub range_db: f32,
    pub threshold_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}

impl Dynamics {
    /// Static, with the brief's proposed defaults for the rest (§C).
    pub const STATIC: Dynamics = Dynamics {
        range_db: 0.0,
        threshold_db: -20.0,
        attack_ms: 10.0,
        release_ms: 150.0,
    };

    /// The settings held inside their ranges; a value that is not finite takes the static default.
    pub fn bounded(self) -> Dynamics {
        let held = |value: f32, fallback: f32, min: f64, max: f64| {
            let value = if value.is_finite() { value } else { fallback };
            value.clamp(min as f32, max as f32)
        };
        let fallback = Dynamics::STATIC;
        Dynamics {
            range_db: held(
                self.range_db,
                fallback.range_db,
                -RANGE_DB_MAX,
                RANGE_DB_MAX,
            ),
            threshold_db: held(
                self.threshold_db,
                fallback.threshold_db,
                THRESHOLD_DB_MIN,
                THRESHOLD_DB_MAX,
            ),
            attack_ms: held(
                self.attack_ms,
                fallback.attack_ms,
                ATTACK_MS_MIN,
                ATTACK_MS_MAX,
            ),
            release_ms: held(
                self.release_ms,
                fallback.release_ms,
                RELEASE_MS_MIN,
                RELEASE_MS_MAX,
            ),
        }
    }
}

/// The soft knee: 0 below `−KNEE_DB/2`, the level over Threshold above `KNEE_DB/2`, and between
/// them the quadratic that joins the two with matching slope.
pub fn knee(over_db: f64) -> f64 {
    let half = KNEE_DB / 2.0;
    if over_db <= -half {
        0.0
    } else if over_db >= half {
        over_db
    } else {
        (over_db + half).powi(2) / (2.0 * KNEE_DB)
    }
}

/// The moving part of a band's gain, in dB, for a level `level_db` against `threshold_db` with
/// `range_db`: 0 at Range 0, never past Range.
pub fn law(level_db: f64, threshold_db: f64, range_db: f64) -> f64 {
    if range_db == 0.0 || !level_db.is_finite() {
        return 0.0;
    }
    let slope = if range_db < 0.0 {
        CUT_SLOPE
    } else {
        LIFT_SLOPE
    };
    let asked = slope * knee(level_db - threshold_db);
    let extent = range_db.abs();
    range_db.signum() * extent * (1.0 - (-asked / extent).exp())
}

/// One band's ballistics: the one-pole factors for a control interval, from Attack and Release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ballistics {
    /// The share of the distance kept per interval while moving away from rest.
    pub attack_keep: f64,
    /// The share kept while moving back toward rest, or across it.
    pub release_keep: f64,
}

impl Ballistics {
    /// For `dynamics`' times at an interval of `interval_seconds`.
    pub fn new(dynamics: Dynamics, interval_seconds: f64) -> Ballistics {
        let keep = |ms: f32| (-interval_seconds / (f64::from(ms) / 1_000.0)).exp();
        let dynamics = dynamics.bounded();
        Ballistics {
            attack_keep: keep(dynamics.attack_ms),
            release_keep: keep(dynamics.release_ms),
        }
    }

    /// The moving part one interval on, from `moving` toward the law's `target`.
    pub fn step(&self, moving: f64, target: f64) -> f64 {
        let away = target != 0.0
            && (moving == 0.0 || moving.signum() == target.signum())
            && target.abs() > moving.abs();
        let keep = if away {
            self.attack_keep
        } else {
            self.release_keep
        };
        let next = target + (moving - target) * keep;
        if target == 0.0 && next.abs() < REST_DB {
            0.0
        } else {
            next
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANGES: [f64; 8] = [-30.0, -12.0, -6.0, -1.0, 1.0, 6.0, 12.0, 30.0];

    fn levels() -> impl Iterator<Item = f64> {
        (0..=1_200).map(|step| -80.0 + f64::from(step) * 0.1)
    }

    #[test]
    fn nothing_moves_below_the_knee_or_at_range_zero() {
        for range in RANGES {
            assert_eq!(law(-20.0 - KNEE_DB / 2.0, -20.0, range), 0.0);
            assert_eq!(law(-60.0, -20.0, range), 0.0);
        }
        for level in levels() {
            assert_eq!(law(level, -20.0, 0.0), 0.0);
        }
    }

    #[test]
    fn the_moving_part_never_passes_range_and_keeps_its_sign() {
        // Far enough over, `1 − e^(−a/|Range|)` rounds to 1: equal to Range, never past it.
        for range in RANGES {
            for level in levels() {
                let moving = law(level, -40.0, range);
                assert!(moving.abs() <= range.abs(), "{range} at {level}: {moving}");
                assert!(moving == 0.0 || moving.signum() == range.signum());
            }
        }
    }

    /// **A louder band never comes out quieter**: the output, level plus moving part, rises with
    /// the level at every Range, never steeper than the lift's 1.5 nor shallower than the cut's 1/3.
    #[test]
    fn the_law_is_monotonic_in_level_at_every_range() {
        for range in RANGES {
            let mut last: Option<(f64, f64)> = None;
            for level in levels() {
                let out = level + law(level, -40.0, range);
                if let Some((last_level, last_out)) = last {
                    let slope = (out - last_out) / (level - last_level);
                    assert!(
                        (1.0 / 3.0 - 1e-9..=1.5 + 1e-9).contains(&slope),
                        "{range} at {level}: slope {slope}"
                    );
                }
                last = Some((level, out));
            }
        }
    }

    /// **More Range never means less action** at a given level, and Range toward 0 takes the band
    /// back to static: the round-4 counterexample (−20 against −30 dB, 20 dB over) holds.
    #[test]
    fn the_law_is_monotonic_in_range_at_every_level() {
        for level in levels() {
            for sign in [-1.0, 1.0] {
                let mut last = 0.0f64;
                for step in 1..=300 {
                    let range = sign * f64::from(step) * 0.1;
                    let moving = law(level, -40.0, range).abs();
                    assert!(moving >= last - 1e-12, "{range} at {level}");
                    last = moving;
                }
            }
        }
        assert!(law(-20.0, -40.0, -30.0).abs() > law(-20.0, -40.0, -20.0).abs());
    }

    #[test]
    fn far_over_threshold_the_moving_part_approaches_range() {
        // 60 dB over: a cut has asked for 40 dB, a lift for 30, so 6 · (1 − e^(−40/6)) and
        // 6 · (1 − e^(−5)).
        assert!((law(0.0, -60.0, -6.0) + 5.9924).abs() < 1e-3);
        assert!((law(0.0, -60.0, 6.0) - 5.9596).abs() < 1e-3);
        // 12 dB over, Range −6: a cut has asked for 8 dB, so 6 · (1 − e^(−4/3)) ≈ 4.42 dB.
        assert!((law(-48.0, -60.0, -6.0) + 4.419).abs() < 0.01);
    }

    /// **A lift is timed as a cut is**: the same Attack takes either the same share of the way in
    /// an interval, and the same Release brings either back.
    #[test]
    fn a_lift_is_timed_as_a_cut_is() {
        let ballistics = Ballistics::new(
            Dynamics {
                range_db: -6.0,
                attack_ms: 10.0,
                release_ms: 150.0,
                ..Dynamics::STATIC
            },
            1.0 / 3_000.0,
        );
        let cut = ballistics.step(0.0, -6.0);
        let lift = ballistics.step(0.0, 6.0);
        assert_eq!(cut, -lift);
        assert!(
            (cut + 6.0 * (1.0 - ballistics.attack_keep)).abs() < 1e-12,
            "attack"
        );
        assert_eq!(ballistics.step(-6.0, 0.0), -ballistics.step(6.0, 0.0));
        assert!((ballistics.step(-6.0, 0.0) + 6.0 * ballistics.release_keep).abs() < 1e-12);
    }

    /// One pole per direction: the state lies between where it was and the target, so a Range
    /// change, sign included, never overshoots, and through a sign change it passes 0 continuously.
    #[test]
    fn a_step_never_overshoots_and_crosses_zero_continuously() {
        let ballistics = Ballistics::new(Dynamics::STATIC, 1.0 / 3_000.0);
        let mut moving = -12.0;
        let mut crossed = false;
        for _ in 0..20_000 {
            let next = ballistics.step(moving, 6.0);
            assert!(next >= moving && next <= 6.0, "{moving} to {next}");
            crossed |= moving < 0.0 && next >= 0.0;
            moving = next;
        }
        assert!(crossed && (moving - 6.0).abs() < 1e-9);
    }

    #[test]
    fn a_release_ends_at_exactly_zero() {
        let ballistics = Ballistics::new(
            Dynamics {
                release_ms: 2_000.0,
                ..Dynamics::STATIC
            },
            1.0 / 3_000.0,
        );
        let mut moving = -30.0;
        let mut intervals = 0;
        while moving != 0.0 {
            moving = ballistics.step(moving, 0.0);
            intervals += 1;
            assert!(intervals < 3_000 * 60, "never at rest");
        }
        // The longest release from all of Range: ln(30 / REST_DB) ≈ 12.6 time constants.
        let seconds = f64::from(intervals) / 3_000.0;
        assert!((20.0..30.0).contains(&seconds), "{seconds} s");
    }

    #[test]
    fn hostile_settings_are_held_in_range() {
        let wild = Dynamics {
            range_db: f32::NAN,
            threshold_db: 40.0,
            attack_ms: 0.0,
            release_ms: f32::INFINITY,
        }
        .bounded();
        assert_eq!(wild.range_db, 0.0);
        assert_eq!(wild.threshold_db, 0.0);
        assert_eq!(wild.attack_ms, ATTACK_MS_MIN as f32);
        assert_eq!(wild.release_ms, Dynamics::STATIC.release_ms);
        assert_eq!(law(f64::NEG_INFINITY, -20.0, -6.0), 0.0);
    }
}
