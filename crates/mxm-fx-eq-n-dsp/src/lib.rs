//! `mxm-fx-eq-n`'s DSP, framework-free: up to 24 bands in series, each a trapezoidal SVF whose
//! design is matched to its analogue prototype, with click-free transitions, band solo, a gain
//! scale and an output gain.
//!
//! The product contract is `plans/plan-mxm-fx-eq-n.md` §2–§3 and `docs/briefs/mxm-fx-eq-n.md` A1–A6;
//! the evidence base is `research:effects/parametric-equalizers.md`. No existing equaliser's code
//! was opened.
//!
//! - [`design`](mod@design): what a band's sections are, by shape, as the audio runs them and the graph draws
//!   them. The editor calls it for the curve, so the curve cannot differ from the sound.
//! - [`filter`]: the sections running, two channels at once.
//! - [`dynamics`]: a dynamic band's law and ballistics (plan §7.1).
//! - [`engine`]: the whole EQ, its values' smoothing, transitions, parking, solo, dynamics and
//!   activity.

pub mod design;
pub mod dynamics;
pub mod engine;
pub mod filter;

pub use design::{
    Cascade, Design, MAX_EFFECTIVE_GAIN_DB, MAX_SECTIONS, Method, Section, Shape, Slope,
    analogue_db, design, design_running, design_solo, design_with, max_frequency,
};
pub use dynamics::Dynamics;
pub use engine::{Band, Engine, Placement};

/// The band slots a host sees (decision 1).
pub const SLOTS: usize = 24;

/// A band's frequency range in hertz (brief A4); the top is also held below Nyquist.
pub const FREQ_MIN: f64 = 10.0;
pub const FREQ_MAX: f64 = 30_000.0;

/// A band's gain range, ±dB (brief A4).
pub const GAIN_RANGE: f64 = 30.0;

/// A band's Q range (brief A4).
pub const Q_MIN: f64 = 0.025;
pub const Q_MAX: f64 = 40.0;

/// The gain scale's range as a factor: 0 to 200 % (brief A4).
pub const GAIN_SCALE_MAX: f64 = 2.0;

/// The output gain's range, ±dB (brief A4).
pub const OUTPUT_GAIN_RANGE_DB: f64 = 24.0;

/// The lowest sample rate the engine is built for. Below it the frequency range would cross
/// itself; the plugin refuses to activate there, and the engine treats anything lower, or not
/// finite, as this.
pub const MIN_SAMPLE_RATE: f64 = 1_000.0;

/// The rate every public entry point works at: `sample_rate` itself when it is finite and at
/// least [`MIN_SAMPLE_RATE`], otherwise that minimum. One rule for the engine and for the designs
/// the editor draws from, so no caller can cross the frequency range's bounds.
pub fn usable_sample_rate(sample_rate: f64) -> f64 {
    if sample_rate.is_finite() && sample_rate >= MIN_SAMPLE_RATE {
        sample_rate
    } else {
        MIN_SAMPLE_RATE
    }
}
