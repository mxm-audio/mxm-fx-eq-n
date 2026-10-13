//! State precision: the shipped `f64` SVF state against the same sections run with `f32` state, on
//! the research's low-frequency cases (`research:effects/parametric-equalizers.md` §4.5).
//!
//! ```text
//! cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_precision
//! ```
//!
//! Coefficients are designed in `f64` either way; only the state and the arithmetic differ. Two
//! readings per case: the gain at the band's frequency from a settled sine (against the design's
//! own), and the `f32` run's round-off on noise, as RMS error against the `f64` run in dB below
//! the output's RMS.

use mxm_fx_eq_n_dsp::{Design, Section, Shape, Slope, design};
use std::f64::consts::TAU;

/// The SVF tick in `f32`, coefficients rounded from the `f64` design.
struct Svf32 {
    sections: Vec<([f32; 6], [f32; 2])>,
}

impl Svf32 {
    fn new(sections: &[Section]) -> Svf32 {
        Svf32 {
            sections: sections
                .iter()
                .map(|section| match *section {
                    Section::Second { g, k, m0, m1, m2 } => {
                        let a1 = 1.0 / (1.0 + g * (g + k));
                        let a2 = g * a1;
                        let a3 = g * a2;
                        ([a1, a2, a3, m0, m1, m2].map(|c| c as f32), [0.0; 2])
                    }
                    Section::First { .. } => unreachable!("second-order cases only"),
                })
                .collect(),
        }
    }

    fn tick(&mut self, x: f32) -> f32 {
        let mut x = x;
        for ([a1, a2, a3, m0, m1, m2], [ic1, ic2]) in &mut self.sections {
            let v3 = x - *ic2;
            let v1 = *a1 * *ic1 + *a2 * v3;
            let v2 = *ic2 + *a2 * *ic1 + *a3 * v3;
            *ic1 = 2.0 * v1 - *ic1;
            *ic2 = 2.0 * v2 - *ic2;
            x = *m0 * x + *m1 * v1 + *m2 * v2;
        }
        x
    }
}

/// The same tick in `f64`, which is what the crate's filter runs.
struct Svf64 {
    sections: Vec<([f64; 6], [f64; 2])>,
}

impl Svf64 {
    fn new(sections: &[Section]) -> Svf64 {
        Svf64 {
            sections: sections
                .iter()
                .map(|section| match *section {
                    Section::Second { g, k, m0, m1, m2 } => {
                        let a1 = 1.0 / (1.0 + g * (g + k));
                        ([a1, g * a1, g * g * a1, m0, m1, m2], [0.0; 2])
                    }
                    Section::First { .. } => unreachable!("second-order cases only"),
                })
                .collect(),
        }
    }

    fn tick(&mut self, x: f64) -> f64 {
        let mut x = x;
        for ([a1, a2, a3, m0, m1, m2], [ic1, ic2]) in &mut self.sections {
            let v3 = x - *ic2;
            let v1 = *a1 * *ic1 + *a2 * v3;
            let v2 = *ic2 + *a2 * *ic1 + *a3 * v3;
            *ic1 = 2.0 * v1 - *ic1;
            *ic2 = 2.0 * v2 - *ic2;
            x = *m0 * x + *m1 * v1 + *m2 * v2;
        }
        x
    }
}

/// The gain at whole-hertz `freq` from a sine settled for `settle` seconds, over one second.
fn gain_db(mut system: impl FnMut(f64) -> f64, freq: f64, rate: f64, settle: usize) -> f64 {
    let n = rate as usize;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for i in 0..(settle + 1) * n {
        let phase = TAU * freq * i as f64 / rate;
        let y = system(0.5 * phase.sin());
        if i >= settle * n {
            re += y * phase.cos();
            im += y * phase.sin();
        }
    }
    20.0 * (2.0 * re.hypot(im) / n as f64 / 0.5).log10()
}

fn main() {
    println!(
        "{:<36} {:>10} {:>10} {:>10} {:>14}",
        "case", "designed", "f64 state", "f32 state", "f32 round-off"
    );
    let cases = [
        (
            "bell 20 Hz Q 10 +12 dB, 192 kHz",
            20.0,
            10.0,
            12.0,
            192_000.0,
            Shape::Bell,
        ),
        (
            "bell 10 Hz Q 20 +6 dB, 192 kHz",
            10.0,
            20.0,
            6.0,
            192_000.0,
            Shape::Bell,
        ),
        (
            "bell 20 Hz Q 4 -24 dB, 96 kHz",
            20.0,
            4.0,
            -24.0,
            96_000.0,
            Shape::Bell,
        ),
        (
            "bell 20 Hz Q 2 +12 dB, 48 kHz",
            20.0,
            2.0,
            12.0,
            48_000.0,
            Shape::Bell,
        ),
        (
            "bell 2 kHz Q 2 +12 dB, 48 kHz",
            2_000.0,
            2.0,
            12.0,
            48_000.0,
            Shape::Bell,
        ),
        (
            "notch 50 Hz Q 8, 48 kHz",
            50.0,
            8.0,
            0.0,
            48_000.0,
            Shape::Notch,
        ),
    ];
    for (label, freq, q, gain_db_set, rate, shape) in cases {
        let band = Design {
            shape,
            slope: Slope::Db12,
            freq,
            gain_db: gain_db_set,
            q,
        };
        let cascade = design(band, rate);
        // The notch is measured beside its zero, where there is something to measure.
        let probe = if shape == Shape::Notch {
            freq * 1.5
        } else {
            freq
        };
        let designed = cascade.response_db(probe, rate);
        let settle = 30;
        let mut wide = Svf64::new(cascade.sections());
        let heard_f64 = gain_db(|x| wide.tick(x), probe, rate, settle);
        let mut narrow = Svf32::new(cascade.sections());
        let heard_f32 = gain_db(|x| f64::from(narrow.tick(x as f32)), probe, rate, settle);

        // Round-off on noise: f32 state against f64 state, same coefficients.
        let mut wide = Svf64::new(cascade.sections());
        let mut narrow = Svf32::new(cascade.sections());
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let (mut error, mut signal) = (0.0f64, 0.0f64);
        for _ in 0..(rate as usize) * 10 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let x = ((seed >> 40) as f64 / (1u64 << 23) as f64 - 1.0) * 0.5;
            let x = f64::from(x as f32);
            let reference = wide.tick(x);
            let rounded = f64::from(narrow.tick(x as f32));
            error += (rounded - reference) * (rounded - reference);
            signal += reference * reference;
        }
        let round_off = 10.0 * (error / signal).log10();
        println!(
            "{label:<36} {designed:>10.4} {heard_f64:>10.4} {heard_f32:>10.4} {round_off:>11.1} dB"
        );
    }
}
