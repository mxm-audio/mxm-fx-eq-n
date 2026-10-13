//! The sections running: Simper's trapezoidal SVF tick and the trapezoidal one-pole, up to two
//! channels at once. Coefficients and state are `f64` (the precision measurement is in this
//! crate's `AGENTS.md`); only the engine's input and output are `f32`.
//!
//! **A moving section ramps, sample by sample.** The engine designs a band once per control
//! interval; stepping the coefficients there would be a staircase, and every step a small
//! discontinuity in the output — a zipper at the control rate. [`Filter::ramp_to`] instead moves
//! each section's own parameters, `g`, `k` and the mix, linearly across the interval and derives
//! Simper's `a1`, `a2`, `a3` from them every sample (plan §3.4). Interpolating the SVF's own
//! parameters keeps the shape sensible (Wishnick 2014; `research:effects/parametric-equalizers.md`
//! §4.4), and the SVF is stable along any path of `g > 0`, `k > 0`. A section at rest runs on its
//! precomputed `a`s and pays nothing for this.

use crate::design::{Cascade, MAX_SECTIONS, Section};

/// State below this is flushed to zero on every write, so a decaying band never runs on
/// subnormals and silence stays exactly silent.
const DENORMAL_FLOOR: f64 = 1.0e-30;

#[inline]
fn flush(x: f64) -> f64 {
    if x.abs() < DENORMAL_FLOOR { 0.0 } else { x }
}

/// A section's five parameters; a one-pole leaves `k` and `m2` at zero.
#[derive(Clone, Copy, Default, PartialEq)]
struct Params {
    g: f64,
    k: f64,
    m0: f64,
    m1: f64,
    m2: f64,
}

impl Params {
    fn of(section: &Section) -> Params {
        match *section {
            Section::Second { g, k, m0, m1, m2 } => Params { g, k, m0, m1, m2 },
            Section::First { g, m0, m1 } => Params {
                g,
                k: 0.0,
                m0,
                m1,
                m2: 0.0,
            },
        }
    }

    fn step_toward(self, target: Params, frames: f64) -> Params {
        Params {
            g: (target.g - self.g) / frames,
            k: (target.k - self.k) / frames,
            m0: (target.m0 - self.m0) / frames,
            m1: (target.m1 - self.m1) / frames,
            m2: (target.m2 - self.m2) / frames,
        }
    }

    fn add(&mut self, step: Params) {
        self.g += step.g;
        self.k += step.k;
        self.m0 += step.m0;
        self.m1 += step.m1;
        self.m2 += step.m2;
    }
}

/// One section with its state: an SVF, or a one-pole when `first`.
#[derive(Clone, Copy)]
struct Running {
    first: bool,
    /// The parameters running now, where a ramp ends, and the per-sample step between them.
    now: Params,
    target: Params,
    step: Params,
    /// Simper's precomputed coefficients for `now`: `a1 = 1/(1 + g(g + k))`, `a2 = g·a1`,
    /// `a3 = g·a2`; a one-pole uses `a1` as `g/(1 + g)`.
    a1: f64,
    a2: f64,
    a3: f64,
    /// Integrator states per channel; a one-pole uses `s1` alone.
    s1: [f64; 2],
    s2: [f64; 2],
}

impl Running {
    const CLEAR: Running = Running {
        first: false,
        now: Params {
            g: 0.0,
            k: 0.0,
            m0: 1.0,
            m1: 0.0,
            m2: 0.0,
        },
        target: Params {
            g: 0.0,
            k: 0.0,
            m0: 1.0,
            m1: 0.0,
            m2: 0.0,
        },
        step: Params {
            g: 0.0,
            k: 0.0,
            m0: 0.0,
            m1: 0.0,
            m2: 0.0,
        },
        a1: 0.0,
        a2: 0.0,
        a3: 0.0,
        s1: [0.0; 2],
        s2: [0.0; 2],
    };

    #[inline]
    fn derive(&mut self) {
        let Params { g, k, .. } = self.now;
        if self.first {
            self.a1 = g / (1.0 + g);
        } else {
            self.a1 = 1.0 / (1.0 + g * (g + k));
            self.a2 = g * self.a1;
            self.a3 = g * self.a2;
        }
    }

    fn set(&mut self, section: &Section) {
        self.first = matches!(section, Section::First { .. });
        self.now = Params::of(section);
        self.target = self.now;
        self.step = Params::default();
        self.derive();
    }

    #[inline]
    fn tick<const C: usize>(&mut self, value: &mut [f64; C]) {
        let Params { m0, m1, m2, .. } = self.now;
        if self.first {
            for (channel, x) in value.iter_mut().enumerate() {
                let v = (*x - self.s1[channel]) * self.a1;
                let lp = v + self.s1[channel];
                self.s1[channel] = flush(lp + v);
                *x = m0 * *x + m1 * lp;
            }
        } else {
            for (channel, x) in value.iter_mut().enumerate() {
                let v3 = *x - self.s2[channel];
                let v1 = self.a1 * self.s1[channel] + self.a2 * v3;
                let v2 = self.s2[channel] + self.a2 * self.s1[channel] + self.a3 * v3;
                self.s1[channel] = flush(2.0 * v1 - self.s1[channel]);
                self.s2[channel] = flush(2.0 * v2 - self.s2[channel]);
                *x = m0 * *x + m1 * v1 + m2 * v2;
            }
        }
    }
}

/// A band's running cascade. Empty, it is the identity.
#[derive(Clone, Copy)]
pub struct Filter {
    sections: [Running; MAX_SECTIONS],
    count: usize,
    /// Samples left in the ramp in progress.
    ramp: usize,
}

impl Filter {
    pub const EMPTY: Filter = Filter {
        sections: [Running::CLEAR; MAX_SECTIONS],
        count: 0,
        ramp: 0,
    };

    /// A filter for `cascade`, from silence.
    pub fn fresh(cascade: &Cascade) -> Filter {
        let mut filter = Filter::EMPTY;
        filter.load(cascade);
        filter
    }

    /// New coefficients at once, state kept; any ramp in progress ends. A section the cascade
    /// gains starts from silence.
    pub fn load(&mut self, cascade: &Cascade) {
        for (index, section) in cascade.sections().iter().enumerate() {
            if index >= self.count {
                self.sections[index] = Running::CLEAR;
            }
            self.sections[index].set(section);
        }
        self.count = cascade.sections().len();
        self.ramp = 0;
    }

    /// Moves to `cascade` linearly over the next `frames` samples, state kept. The engine only
    /// ramps between cascades of one layout, which a shape and slope fix; a section the cascade
    /// gains starts from silence at its target.
    pub fn ramp_to(&mut self, cascade: &Cascade, frames: usize) {
        let sections = cascade.sections();
        // Already there and at rest: nothing to move, and the section keeps its precomputed `a`s
        // rather than re-deriving them every sample of an empty ramp.
        let arrived = self.ramp == 0
            && sections.len() == self.count
            && sections
                .iter()
                .zip(&self.sections)
                .all(|(section, running)| Params::of(section) == running.now);
        if arrived {
            return;
        }
        if frames <= 1 {
            self.load(cascade);
            return;
        }
        for (index, section) in sections.iter().enumerate() {
            let running = &mut self.sections[index];
            let first = matches!(section, Section::First { .. });
            if index >= self.count || running.first != first {
                *running = Running::CLEAR;
                running.set(section);
                continue;
            }
            // Wherever a ramp in progress has reached, from there.
            running.target = Params::of(section);
            running.step = running.now.step_toward(running.target, frames as f64);
        }
        self.count = sections.len();
        self.ramp = frames;
    }

    /// Silence in every section, coefficients kept.
    pub fn clear(&mut self) {
        for section in &mut self.sections {
            section.s1 = [0.0; 2];
            section.s2 = [0.0; 2];
        }
    }

    /// The largest state magnitude: zero exactly when the filter holds nothing, and infinite if a
    /// state ever is not finite.
    pub fn largest_state(&self) -> f64 {
        self.sections[..self.count]
            .iter()
            .flat_map(|section| section.s1.iter().chain(&section.s2))
            .fold(0.0f64, |largest, value| {
                if value.is_finite() {
                    largest.max(value.abs())
                } else {
                    f64::INFINITY
                }
            })
    }

    /// Whether a ramp is in progress.
    pub fn is_ramping(&self) -> bool {
        self.ramp > 0
    }

    /// One frame through the cascade, `C` channels. **One or two**: each section keeps two lanes of
    /// state, and any other width fails to compile rather than panicking on the audio thread.
    ///
    /// ```
    /// let mut filter = mxm_fx_eq_n_dsp::filter::Filter::EMPTY;
    /// filter.process([0.0f64; 2]);
    /// filter.process([0.0f64; 1]);
    /// ```
    ///
    /// ```compile_fail
    /// let mut filter = mxm_fx_eq_n_dsp::filter::Filter::EMPTY;
    /// filter.process([0.0f64; 3]);
    /// ```
    ///
    /// **Every caller gets numeric recovery**, not only the engine: a non-finite input sample is
    /// taken as silence, and an output that is not finite (an overflowed state) clears every state
    /// and is returned as silence, so one bad sample never poisons the filter.
    #[inline]
    pub fn process<const C: usize>(&mut self, input: [f64; C]) -> [f64; C] {
        const { assert!(C == 1 || C == 2, "a filter runs one or two channels") };
        let mut value = input.map(|x| if x.is_finite() { x } else { 0.0 });
        value = self.run(value);
        if value.iter().all(|x| x.is_finite()) {
            value
        } else {
            self.clear();
            [0.0; C]
        }
    }

    #[inline]
    fn run<const C: usize>(&mut self, mut value: [f64; C]) -> [f64; C] {
        if self.ramp > 0 {
            self.ramp -= 1;
            let last = self.ramp == 0;
            // Every section's coefficients first, then the signal through them: the derivations
            // do not depend on the signal, so their divisions run side by side instead of each
            // waiting on the section before it.
            for section in &mut self.sections[..self.count] {
                if last {
                    section.now = section.target;
                } else {
                    section.now.add(section.step);
                }
                section.derive();
            }
        }
        for section in &mut self.sections[..self.count] {
            section.tick(&mut value);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Design, Shape, Slope, design};
    use std::f64::consts::PI;

    /// The steady-state gain of `cascade` at a whole-hertz `freq`, by correlating one second of the
    /// settled output: a whole number of cycles, so there is no leakage.
    fn measured_db(cascade: &Cascade, freq: f64, rate: f64) -> f64 {
        let mut filter = Filter::fresh(cascade);
        let settle = rate as usize;
        let measure = rate as usize;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for n in 0..settle + measure {
            let phase = 2.0 * PI * freq * n as f64 / rate;
            let [y] = filter.process([phase.sin()]);
            if n >= settle {
                re += y * phase.cos();
                im += y * phase.sin();
            }
        }
        20.0 * (2.0 * re.hypot(im) / measure as f64).log10()
    }

    #[test]
    fn the_running_sections_have_the_designed_response() {
        let rate = 48_000.0;
        for (shape, slope) in [
            (Shape::Bell, Slope::Db12),
            (Shape::LowShelf, Slope::Db12),
            (Shape::HighShelf, Slope::Db12),
            (Shape::LowCut, Slope::Db18),
            (Shape::HighCut, Slope::Db48),
            (Shape::Notch, Slope::Db12),
            (Shape::BandPass, Slope::Db12),
            (Shape::TiltShelf, Slope::Db12),
        ] {
            let cascade = design(
                Design {
                    shape,
                    slope,
                    freq: 2_000.0,
                    gain_db: -9.0,
                    q: 1.4,
                },
                rate,
            );
            for probe in [300.0, 1_700.0, 2_600.0, 9_000.0] {
                let drawn = cascade.response_db(probe, rate);
                let heard = measured_db(&cascade, probe, rate);
                if drawn > -60.0 {
                    assert!(
                        (drawn - heard).abs() < 0.01,
                        "{shape:?} at {probe}: {drawn} drawn, {heard} heard"
                    );
                }
            }
        }
    }

    #[test]
    fn a_bad_sample_never_poisons_a_first_or_second_order_section() {
        let rate = 48_000.0;
        for (shape, slope) in [(Shape::LowCut, Slope::Db18), (Shape::Bell, Slope::Db12)] {
            let cascade = design(
                Design {
                    shape,
                    slope,
                    freq: 500.0,
                    gain_db: 6.0,
                    q: 2.0,
                },
                rate,
            );
            let mut filter = Filter::fresh(&cascade);
            let mut reference = Filter::fresh(&cascade);
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.0e308] {
                filter.clear();
                reference.clear();
                for n in 0..50 {
                    let x = (n as f64 * 0.21).sin();
                    filter.process([x, -x]);
                }
                let [y, z] = filter.process([bad, bad]);
                assert!(y.is_finite() && z.is_finite(), "{shape:?} after {bad}");
                assert!(filter.largest_state().is_finite());
                // From silence the two filters are the same again on the next finite input.
                filter.clear();
                for n in 0..200 {
                    let x = (n as f64 * 0.37).cos();
                    assert_eq!(
                        filter.process([x])[0],
                        reference.process([x])[0],
                        "{shape:?}"
                    );
                }
            }
        }
    }

    // Only a 64-bit `usize` reaches past `u32`.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn a_ramp_longer_than_u32_counts_every_sample() {
        let rate = 48_000.0;
        let band = |freq| Design {
            shape: Shape::Bell,
            slope: Slope::Db12,
            freq,
            gain_db: 6.0,
            q: 1.0,
        };
        let mut filter = Filter::fresh(&design(band(1_000.0), rate));
        let frames = (1usize << 32) + 7;
        filter.ramp_to(&design(band(2_000.0), rate), frames);
        assert!(filter.is_ramping());
        assert_eq!(filter.ramp, frames);
        filter.process([0.0]);
        assert_eq!(filter.ramp, frames - 1);
    }

    #[test]
    fn a_ramp_to_where_the_filter_already_is_does_nothing() {
        let rate = 48_000.0;
        let cascade = design(
            Design {
                shape: Shape::HighCut,
                slope: Slope::Db96,
                freq: 4_000.0,
                gain_db: 0.0,
                q: 0.7,
            },
            rate,
        );
        let mut filter = Filter::fresh(&cascade);
        filter.ramp_to(&cascade, 16);
        assert!(!filter.is_ramping());
    }

    #[test]
    fn a_ramp_lands_exactly_on_its_target_and_matches_a_load_from_then_on() {
        let rate = 48_000.0;
        let band = |gain_db| Design {
            shape: Shape::HighShelf,
            slope: Slope::Db12,
            freq: 3_000.0,
            gain_db,
            q: 0.9,
        };
        let start = design(band(-6.0), rate);
        let end = design(band(9.0), rate);
        let mut ramped = Filter::fresh(&start);
        let mut loaded = Filter::fresh(&start);
        let input = |n: usize| (n as f64 * 0.37).sin();
        for n in 0..100 {
            ramped.process([input(n)]);
            loaded.process([input(n)]);
        }
        ramped.ramp_to(&end, 16);
        for n in 100..116 {
            ramped.process([input(n)]);
        }
        let mut reference = Filter::fresh(&end);
        reference.sections[..ramped.count]
            .iter_mut()
            .zip(&ramped.sections)
            .for_each(|(reference, ramped)| {
                reference.s1 = ramped.s1;
                reference.s2 = ramped.s2;
            });
        for n in 116..200 {
            let [a] = ramped.process([input(n)]);
            let [b] = reference.process([input(n)]);
            assert_eq!(a.to_bits(), b.to_bits(), "sample {n}");
        }
        let _ = loaded;
    }
}
