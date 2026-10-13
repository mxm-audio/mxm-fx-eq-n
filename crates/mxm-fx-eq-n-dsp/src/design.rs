//! Band design: a band's sections, from its values, as the audio runs them and the graph draws them.
//!
//! **Every section is Andy Simper's linear trapezoidal state-variable filter** (Simper,
//! *SvfLinearTrapOptimised2*; the same filter as Zavalishin's TPT SVF), or its one-pole sibling. A
//! second-order section is five numbers: the integrator gain `g`, the damping `k` and the output
//! mix `m0·x + m1·bp + m2·lp`. A first-order one is `g` and the mix `m0·x + m1·lp`.
//!
//! **The response is read through the warped variable.** The trapezoidal SVF is the bilinear
//! transform of its analogue prototype, so at `ω` radians per sample a second-order section is
//! `m0 + (m2 + j·m1·Ω) / (1 − Ω² + j·k·Ω)` with `Ω = tan(ω/2) / g`, and a first-order one is
//! `m0 + m1 / (1 + jΩ)`. Nothing cancels at low frequencies, which is why the graph and the
//! design both work in it.
//!
//! **Two methods, and only one ships.**
//! - [`Method::Bilinear`] is the collection's SVF table (`docs/filters/02-topologies.md` §2.2), every
//!   section prewarped at its frequency. It cramps: the response is forced to its analogue value at
//!   infinity *at Nyquist*, so a +12 dB, Q 1 bell at 15 kHz and 44.1 kHz is about 7.5 dB off its
//!   analogue curve below 20 kHz (`research:effects/parametric-equalizers.md` §4.2). It exists so
//!   the measurement that rejected it can be reproduced.
//! - [`Method::Matched`] is the shipped design. Each section's **poles are the impulse-invariant
//!   image of the analogue poles**, and its **numerator is fitted to the analogue magnitude**, after
//!   Martin Vicanek, *Matched Second Order Digital Filters* (2016). Vicanek gives the fit as a
//!   biquad numerator; here it is solved directly for the SVF's mix in the warped variable, which
//!   is better conditioned and keeps the SVF's proven stability under per-sample changes of `g`
//!   and `k` (Wishnick 2014; McClellan 2026). Any stable biquad is an SVF with some `g > 0` and
//!   `k > 0`, so nothing is lost. The fits, by shape:
//!   - a **bell boost** and the **band pass** match DC and their centre, with their extremum there
//!     (Vicanek's peaking conditions), so the peak is on the band's frequency;
//!   - a **shelf** matches DC, its midpoint (half its dB gain at its frequency) and the **fit
//!     point**, 20 kHz or halfway to Nyquist for a band near the top (see `At`);
//!   - **low-pass cut sections** match DC, their corner and the fit point; **high-pass sections**
//!     keep their exact double zero at DC and match their corner; the **notch** keeps its exact
//!     zeros at its centre and unity at DC;
//!   - the **first-order low pass and the tilt** match DC, their frequency and the fit point; the
//!     **first-order high pass** its corner and its stopband asymptote. A one-pole has three
//!     degrees of freedom, so its pole comes from that fit rather than from impulse invariance
//!     (derived here).
//!
//! **A cut is the exact inverse of the equal boost.** The bell's Q is the half-gain-in-dB
//! convention (brief A5), under which the analogue cut is `1 / boost`. A bell or low shelf cut is
//! therefore the boost with the opposite gain, inverted; a high shelf *boost* is the inverted cut.
//! The rule is which side's poles lie at or below the band's frequency: impulse invariance needs
//! poles below Nyquist, and a high shelf boost's sit √A above its frequency. Every impulse-invariant
//! pole in this file is therefore at or below the band's frequency, so there is no regime boundary
//! to cross under frequency automation.

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use crate::{FREQ_MAX, FREQ_MIN, GAIN_RANGE, GAIN_SCALE_MAX, Q_MAX, Q_MIN, usable_sample_rate};

/// The most sections a band has: a 96 dB/octave cut is order 16, eight second-order sections.
pub const MAX_SECTIONS: usize = 8;

/// The largest gain a band can be designed with: ±30 dB at the gain scale's 200 %.
pub const MAX_EFFECTIVE_GAIN_DB: f64 = GAIN_RANGE * GAIN_SCALE_MAX;

/// The highest frequency a band reaches at this sample rate: the brief's 30 kHz, held below
/// Nyquist (brief A4).
pub fn max_frequency(sample_rate: f64) -> f64 {
    (0.49 * usable_sample_rate(sample_rate)).min(FREQ_MAX)
}

/// The eight shapes of v1 (brief A3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Bell,
    LowShelf,
    HighShelf,
    LowCut,
    HighCut,
    Notch,
    BandPass,
    TiltShelf,
}

impl Shape {
    pub const ALL: [Shape; 8] = [
        Shape::Bell,
        Shape::LowShelf,
        Shape::HighShelf,
        Shape::LowCut,
        Shape::HighCut,
        Shape::Notch,
        Shape::BandPass,
        Shape::TiltShelf,
    ];

    /// Bells, shelves and the tilt have a gain; the gain scale scales it, and at 0 dB they are the
    /// identity. Cuts, the notch and the band pass have none (brief A5).
    pub fn has_gain(self) -> bool {
        matches!(
            self,
            Shape::Bell | Shape::LowShelf | Shape::HighShelf | Shape::TiltShelf
        )
    }

    pub fn has_slope(self) -> bool {
        matches!(self, Shape::LowCut | Shape::HighCut)
    }

    /// Whether Q shapes this band: not the tilt, whose Q is fixed, and not a 6 dB/octave cut,
    /// which is first order.
    pub fn has_q(self, slope: Slope) -> bool {
        match self {
            Shape::TiltShelf => false,
            Shape::LowCut | Shape::HighCut => slope != Slope::Db6,
            _ => true,
        }
    }

    /// Solo is offered for every shape but the tilt, which changes the whole spectrum (brief A5).
    pub fn offers_solo(self) -> bool {
        self != Shape::TiltShelf
    }
}

/// A cut's slope (brief A3). Each step is an order of 6 dB/octave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slope {
    Db6,
    Db12,
    Db18,
    Db24,
    Db36,
    Db48,
    Db72,
    Db96,
}

impl Slope {
    pub const ALL: [Slope; 8] = [
        Slope::Db6,
        Slope::Db12,
        Slope::Db18,
        Slope::Db24,
        Slope::Db36,
        Slope::Db48,
        Slope::Db72,
        Slope::Db96,
    ];

    pub fn db_per_octave(self) -> u16 {
        match self {
            Slope::Db6 => 6,
            Slope::Db12 => 12,
            Slope::Db18 => 18,
            Slope::Db24 => 24,
            Slope::Db36 => 36,
            Slope::Db48 => 48,
            Slope::Db72 => 72,
            Slope::Db96 => 96,
        }
    }

    /// The filter order, one per 6 dB/octave.
    pub fn order(self) -> usize {
        usize::from(self.db_per_octave() / 6)
    }
}

/// Which design the sections come from. Only [`Method::Matched`] ships; see the module comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Bilinear,
    Matched,
}

/// What a band is designed from. `gain_db` is the gain as heard: the band's gain times the gain
/// scale, and times its entry or exit fade. A gainless shape ignores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    pub shape: Shape,
    pub slope: Slope,
    pub freq: f64,
    pub gain_db: f64,
    pub q: f64,
}

impl Design {
    /// The design held inside every range at this sample rate, whatever a corrupt state, a host or
    /// a race supplied. A non-finite value takes a neutral default.
    pub fn bounded(self, sample_rate: f64) -> Design {
        let finite_or = |value: f64, fallback: f64| {
            if value.is_finite() { value } else { fallback }
        };
        Design {
            freq: finite_or(self.freq, 1_000.0).clamp(FREQ_MIN, max_frequency(sample_rate)),
            gain_db: finite_or(self.gain_db, 0.0)
                .clamp(-MAX_EFFECTIVE_GAIN_DB, MAX_EFFECTIVE_GAIN_DB),
            q: finite_or(self.q, 1.0).clamp(Q_MIN, Q_MAX),
            ..self
        }
    }

    /// A gain shape at exactly 0 dB is the identity, and has no sections at all.
    pub fn is_identity(&self) -> bool {
        self.shape.has_gain() && self.gain_db == 0.0
    }
}

/// One section: the five numbers the SVF runs (or three for a one-pole).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Section {
    /// `y = m0·x + m1·bp + m2·lp` of a trapezoidal SVF with integrator gain `g`, damping `k`.
    Second {
        g: f64,
        k: f64,
        m0: f64,
        m1: f64,
        m2: f64,
    },
    /// `y = m0·x + m1·lp` of a trapezoidal one-pole with integrator gain `g`.
    First { g: f64, m0: f64, m1: f64 },
}

impl Section {
    /// `|H|²` at the frequency whose `tan(ω/2)` is `tan_half`; `f64::INFINITY` is Nyquist.
    pub fn power(&self, tan_half: f64) -> f64 {
        match *self {
            Section::Second { g, k, m0, m1, m2 } => {
                if tan_half.is_infinite() {
                    return m0 * m0;
                }
                let u = (tan_half / g) * (tan_half / g);
                let real = m0 * (1.0 - u) + m2;
                let imag = m0 * k + m1;
                let numerator = real * real + u * imag * imag;
                let denominator = (1.0 - u) * (1.0 - u) + k * k * u;
                numerator / denominator.max(f64::MIN_POSITIVE)
            }
            Section::First { g, m0, m1 } => {
                if tan_half.is_infinite() {
                    return m0 * m0;
                }
                let u = (tan_half / g) * (tan_half / g);
                ((m0 + m1) * (m0 + m1) + m0 * m0 * u) / (1.0 + u)
            }
        }
    }
}

/// A band's sections, in the order they run. A first-order section, when there is one, is first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cascade {
    sections: [Section; MAX_SECTIONS],
    count: usize,
}

impl Cascade {
    /// No sections: the identity.
    pub const IDENTITY: Cascade = Cascade {
        sections: [Section::First {
            g: 1.0,
            m0: 1.0,
            m1: 0.0,
        }; MAX_SECTIONS],
        count: 0,
    };

    fn single(section: Section) -> Cascade {
        let mut cascade = Cascade::IDENTITY;
        cascade.push(section);
        cascade
    }

    /// A cascade of hand-made sections, for measuring a candidate design against the shipped
    /// one. At most [`MAX_SECTIONS`] are kept.
    pub fn from_sections(sections: &[Section]) -> Cascade {
        let mut cascade = Cascade::IDENTITY;
        for section in sections {
            cascade.push(*section);
        }
        cascade
    }

    fn push(&mut self, section: Section) {
        if self.count < MAX_SECTIONS {
            self.sections[self.count] = section;
            self.count += 1;
        }
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections[..self.count]
    }

    /// The magnitude response in dB at `freq`, from the numbers the audio runs. At or above
    /// Nyquist it is the Nyquist value.
    pub fn response_db(&self, freq: f64, sample_rate: f64) -> f64 {
        let tan_half = tan_half(freq, usable_sample_rate(sample_rate));
        self.sections()
            .iter()
            .map(|section| 10.0 * section.power(tan_half).max(1.0e-30).log10())
            .sum()
    }
}

/// `tan(ω/2)` for `freq` at `sample_rate`; infinity at and above Nyquist, zero at and below DC.
pub fn tan_half(freq: f64, sample_rate: f64) -> f64 {
    let half = PI * freq / usable_sample_rate(sample_rate);
    if half.is_nan() || half <= 0.0 {
        0.0
    } else if half >= 0.5 * PI {
        f64::INFINITY
    } else {
        half.tan()
    }
}

/// The band's sections: what the audio runs and the graph draws.
pub fn design(band: Design, sample_rate: f64) -> Cascade {
    design_with(Method::Matched, band, sample_rate)
}

/// The band's sections by either method. Product code calls [`design`].
pub fn design_with(method: Method, band: Design, sample_rate: f64) -> Cascade {
    let sample_rate = usable_sample_rate(sample_rate);
    let band = band.bounded(sample_rate);
    if band.is_identity() {
        return Cascade::IDENTITY;
    }
    sections(method, band, sample_rate)
}

/// What the engine runs: [`design`]'s sections, except that a gain shape at 0 dB keeps its
/// section, at the identity mix, rather than having none. The section count of a shape and slope
/// is then fixed, so a gain fading out to 0 dB, coming in from it or gliding through it moves
/// continuously instead of dropping or gaining a section mid-signal. The response is the same.
pub fn design_running(band: Design, sample_rate: f64) -> Cascade {
    let sample_rate = usable_sample_rate(sample_rate);
    sections(Method::Matched, band.bounded(sample_rate), sample_rate)
}

#[cfg(test)]
thread_local! {
    /// How many band and solo designs this thread has made, so a unit test can prove a path
    /// designs nothing.
    pub(crate) static DESIGNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(crate) static SOLO_DESIGNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn count_design() {
    #[cfg(test)]
    DESIGNS.with(|designs| designs.set(designs.get() + 1));
}

fn count_solo_design() {
    #[cfg(test)]
    SOLO_DESIGNS.with(|designs| designs.set(designs.get() + 1));
}

fn sections(method: Method, band: Design, sample_rate: f64) -> Cascade {
    count_design();
    let at = At::new(band.freq, sample_rate);
    let Design {
        shape,
        slope,
        gain_db,
        q,
        ..
    } = band;
    match shape {
        Shape::Bell => Cascade::single(bell(method, at, q, gain_db)),
        Shape::LowShelf => Cascade::single(low_shelf(method, at, q, gain_db)),
        Shape::HighShelf => Cascade::single(high_shelf(method, at, q, gain_db)),
        Shape::LowCut => cut(method, true, at, q, slope),
        Shape::HighCut => cut(method, false, at, q, slope),
        Shape::Notch => Cascade::single(notch(method, at, q)),
        Shape::BandPass => Cascade::single(band_pass(method, at, q)),
        Shape::TiltShelf => Cascade::single(tilt(method, at, gain_db)),
    }
}

/// Whether a shape's audition hears its Q: only the band pass a bell, notch or band pass auditions
/// as. A shelf's and a cut's auditions run at Q 0.707, so their Q moving changes nothing heard.
pub fn solo_hears_q(shape: Shape) -> bool {
    matches!(shape, Shape::Bell | Shape::Notch | Shape::BandPass)
}

/// What solo plays (brief A5): a band pass at the band's frequency and Q for a bell, notch or band
/// pass; the region a shelf changes (a low or high pass at its frequency, Q 0.707); the mirrored
/// cut at a cut's corner and slope; nothing for the tilt.
///
/// The mirrored cut rather than the input minus the cut: a steep Butterworth's phase turns that
/// difference into a boost of up to 6 dB where the cut passes.
pub fn design_solo(band: Design, sample_rate: f64) -> Option<Cascade> {
    count_solo_design();
    let sample_rate = usable_sample_rate(sample_rate);
    let band = band.bounded(sample_rate);
    let at = At::new(band.freq, sample_rate);
    let method = Method::Matched;
    match band.shape {
        Shape::Bell | Shape::Notch | Shape::BandPass => {
            Some(Cascade::single(band_pass(method, at, band.q)))
        }
        Shape::LowShelf => Some(Cascade::single(low_pass(method, at, FRAC_1_SQRT_2))),
        Shape::HighShelf => Some(Cascade::single(high_pass(method, at, FRAC_1_SQRT_2))),
        Shape::LowCut => Some(cut(method, false, at, FRAC_1_SQRT_2, band.slope)),
        Shape::HighCut => Some(cut(method, true, at, FRAC_1_SQRT_2, band.slope)),
        Shape::TiltShelf => None,
    }
}

/// The analogue prototype's response in dB at `freq`, for the band as [`design`] bounds it at
/// `sample_rate`: what the matched design is measured against.
pub fn analogue_db(band: Design, freq: f64, sample_rate: f64) -> f64 {
    let band = band.bounded(usable_sample_rate(sample_rate));
    if band.is_identity() {
        return 0.0;
    }
    let x = freq / band.freq;
    let power = match band.shape {
        Shape::Bell => analogue::bell(x, band.q, band.gain_db),
        Shape::LowShelf => analogue::low_shelf(x, band.q, band.gain_db),
        Shape::HighShelf => analogue::high_shelf(x, band.q, band.gain_db),
        Shape::Notch => analogue::notch(x, band.q),
        Shape::BandPass => analogue::band_pass(x, band.q),
        Shape::TiltShelf => analogue::tilt(x, band.gain_db),
        Shape::LowCut | Shape::HighCut => {
            let high = band.shape == Shape::LowCut;
            let order = band.slope.order();
            let mut power = if !order.is_multiple_of(2) {
                if high {
                    analogue::high_pass_first(x)
                } else {
                    analogue::low_pass_first(x)
                }
            } else {
                1.0
            };
            for q in butterworth_qs(order, band.q) {
                power *= if high {
                    analogue::high_pass(x, q)
                } else {
                    analogue::low_pass(x, q)
                };
            }
            power
        }
    };
    10.0 * power.max(1.0e-30).log10()
}

/// The second-order sections' Qs of an order-`order` Butterworth cut whose least damped pair is
/// scaled by `q / 0.707`. Q = 1 / (2 cos θ), θ = (2k − 1)π/2N for even N and kπ/N for odd N.
fn butterworth_qs(order: usize, q: f64) -> impl Iterator<Item = f64> {
    let pairs = order / 2;
    (1..=pairs).map(move |pair| {
        let theta = if order.is_multiple_of(2) {
            (2 * pair - 1) as f64 * PI / (2 * order) as f64
        } else {
            pair as f64 * PI / order as f64
        };
        let section_q = 0.5 / theta.cos();
        if pair == pairs {
            section_q * q / FRAC_1_SQRT_2
        } else {
            section_q
        }
    })
}

// ---- The analogue prototypes ---------------------------------------------------------------------

/// The analogue prototypes' power responses, at `x` = frequency over the band's frequency. The
/// matched designs fit to these, and the tests measure the shipped design against them.
pub mod analogue {
    /// `10^(dB/40)`, the cookbook's `A`; the bell's and shelves' linear gain is `A²`.
    pub fn shelf_a(gain_db: f64) -> f64 {
        10f64.powf(gain_db / 40.0)
    }

    /// `(s² + (A/Q)s + 1) / (s² + s/(AQ) + 1)`: the half-gain-in-dB bell.
    pub fn bell(x: f64, q: f64, gain_db: f64) -> f64 {
        let a = shelf_a(gain_db);
        let real = (1.0 - x * x) * (1.0 - x * x);
        (real + (a / q) * (a / q) * x * x) / (real + (x / (a * q)) * (x / (a * q)))
    }

    /// `A (s² + (√A/Q)s + A) / (A s² + (√A/Q)s + 1)`: `A²` at DC, 1 above, `A` at `x = 1`.
    pub fn low_shelf(x: f64, q: f64, gain_db: f64) -> f64 {
        let a = shelf_a(gain_db);
        let cross = a / (q * q) * x * x;
        a * a * ((a - x * x) * (a - x * x) + cross)
            / ((1.0 - a * x * x) * (1.0 - a * x * x) + cross)
    }

    /// `A (A s² + (√A/Q)s + 1) / (s² + (√A/Q)s + A)`: 1 at DC, `A²` above, `A` at `x = 1`.
    pub fn high_shelf(x: f64, q: f64, gain_db: f64) -> f64 {
        let a = shelf_a(gain_db);
        let cross = a / (q * q) * x * x;
        a * a * ((1.0 - a * x * x) * (1.0 - a * x * x) + cross)
            / ((a - x * x) * (a - x * x) + cross)
    }

    /// `1 / (s² + s/Q + 1)`.
    pub fn low_pass(x: f64, q: f64) -> f64 {
        1.0 / ((1.0 - x * x) * (1.0 - x * x) + (x / q) * (x / q))
    }

    /// `s² / (s² + s/Q + 1)`.
    pub fn high_pass(x: f64, q: f64) -> f64 {
        x.powi(4) * low_pass(x, q)
    }

    /// `1 / (s + 1)`.
    pub fn low_pass_first(x: f64) -> f64 {
        1.0 / (1.0 + x * x)
    }

    /// `s / (s + 1)`.
    pub fn high_pass_first(x: f64) -> f64 {
        x * x / (1.0 + x * x)
    }

    /// `(s² + 1) / (s² + s/Q + 1)`.
    pub fn notch(x: f64, q: f64) -> f64 {
        (1.0 - x * x) * (1.0 - x * x) * low_pass(x, q)
    }

    /// `(s/Q) / (s² + s/Q + 1)`: 0 dB at its centre.
    pub fn band_pass(x: f64, q: f64) -> f64 {
        (x / q) * (x / q) * low_pass(x, q)
    }

    /// `(G s + 1) / (s + G)`, `G = 10^(dB/20)`: `1/G` at DC, `G` above, 0 dB at the pivot.
    pub fn tilt(x: f64, gain_db: f64) -> f64 {
        let g = 10f64.powf(gain_db / 20.0);
        (g * g * x * x + 1.0) / (x * x + g * g)
    }
}

// ---- Poles ---------------------------------------------------------------------------------------

/// The SVF's `(g, k)` whose poles are the impulse-invariant image of `s² + 2ζω·s + ω²`, `w` in
/// radians per sample and below π.
///
/// A trapezoidal SVF's denominator is `1 + a1 z⁻¹ + a2 z⁻²` with `g² = (1 + a1 + a2) / (1 − a1 + a2)`
/// and `k·g = 2(1 − a2) / (1 − a1 + a2)`. Those three sums are formed here without cancellation,
/// from `r = e^{−ζw}` and the pole angle, so a 10 Hz pole at 192 kHz keeps full precision.
fn matched_poles(w: f64, zeta: f64) -> (f64, f64) {
    let (sum, alternating, end) = if zeta < 1.0 {
        let sigma = zeta * w;
        let one_minus_r = -(-sigma).exp_m1();
        let r = 1.0 - one_minus_r;
        let (sin, cos) = (0.5 * w * (1.0 - zeta * zeta).sqrt()).sin_cos();
        (
            one_minus_r * one_minus_r + 4.0 * r * sin * sin,
            one_minus_r * one_minus_r + 4.0 * r * cos * cos,
            one_minus_r * (1.0 + r),
        )
    } else {
        // Two real poles, at w·(ζ ∓ √(ζ² − 1)); the slow one written so it does not cancel.
        let root = (zeta * zeta - 1.0).sqrt();
        let slow = w / (zeta + root);
        let fast = w * (zeta + root);
        let one_minus_slow = -(-slow).exp_m1();
        let one_minus_fast = -(-fast).exp_m1();
        (
            one_minus_slow * one_minus_fast,
            (2.0 - one_minus_slow) * (2.0 - one_minus_fast),
            -(-(slow + fast)).exp_m1(),
        )
    };
    let g = (sum / alternating).sqrt();
    (g, 2.0 * end / (alternating * g))
}

/// Where a band is, in radians per sample: its own frequency `w`, and the **fit point** `top`,
/// where a matched design meets its analogue prototype a last time.
///
/// The fit point is 20 kHz, the top of what the error is measured to, or halfway from the band to
/// Nyquist when the band is above 18 kHz or so; at Nyquist, `top` is π. Measured against fitting
/// at Nyquist itself (`eq_n_cramping`, and this crate's `AGENTS.md`), it keeps a cut's passband
/// within a fraction of a dB where fitting at Nyquist left every section a decibel high and a
/// 96 dB/octave cascade 8 dB high. The rule is continuous in `w`, so a frequency sweep never steps
/// the fit.
///
/// It also carries `tan(w/2)` and `tan(top/2)`, which every section of a band shares: a 96 dB/octave
/// cut's eight sections take them once.
#[derive(Clone, Copy)]
struct At {
    w: f64,
    top: f64,
    tan_w: f64,
    tan_top: f64,
}

impl At {
    fn new(freq: f64, sample_rate: f64) -> At {
        let w = 2.0 * PI * freq / sample_rate;
        let top = (2.0 * PI * 20_000.0 / sample_rate)
            .max(0.5 * (w + PI))
            .min(PI);
        At {
            w,
            top,
            tan_w: (0.5 * w).tan(),
            tan_top: if top >= PI {
                f64::INFINITY
            } else {
                (0.5 * top).tan()
            },
        }
    }

    /// The same band with its fit point at Nyquist.
    fn at_nyquist(self) -> At {
        At {
            top: PI,
            tan_top: f64::INFINITY,
            ..self
        }
    }

    /// The fit point in units of the band's frequency, for evaluating an analogue prototype.
    fn top_ratio(self) -> f64 {
        self.top / self.w
    }

    /// `Ω²` at the band's frequency for integrator gain `g`.
    fn u(self, g: f64) -> f64 {
        let omega = self.tan_w / g;
        omega * omega
    }

    /// `Ω²` at the fit point for integrator gain `g`.
    fn u_top(self, g: f64) -> f64 {
        let omega = self.tan_top / g;
        omega * omega
    }
}

// ---- Second-order fits ---------------------------------------------------------------------------

/// The mix for poles `(g, k)` that meets `|H(0)| = dc` and the powers `power` at `at.w` and
/// `top_power` at `at.top`.
///
/// In the warped variable the numerator's power is `(dc − m0·u)² + c·u` with `c = (m0·k + m1)²`,
/// that is `dc² + β·u + γ·u²` with `γ = m0²` and `β = c − 2·dc·m0`: linear in `(β, γ)`, so two
/// points fix it. Where no real mix meets them, the nearest one does, which is continuous across
/// the edge.
fn three_point(g: f64, k: f64, dc: f64, at: At, power: f64, top_power: f64) -> Section {
    fit(g, k, dc, at, power, top_power, None)
}

/// [`three_point`] for a section that will also be inverted, so its zeros must stay strictly
/// inside the left half-plane: its Nyquist gain no more than 6 dB below `nyquist_power`'s, and
/// its zeros no less damped than Q 40's. Only an extreme resonant shelf reaches either floor,
/// where its analogue zeros sit at the top of the band (a +60 dB, Q 6 low shelf at 3.6 kHz puts
/// a −15 dB dip at 20 kHz); without them the fit there put a zero on Nyquist, and the cut built
/// as its inverse a pole on the unit circle.
fn invertible_fit(
    g: f64,
    k: f64,
    dc: f64,
    at: At,
    power: f64,
    top_power: f64,
    nyquist_power: f64,
) -> Section {
    fit(g, k, dc, at, power, top_power, Some(nyquist_power))
}

fn fit(
    g: f64,
    k: f64,
    dc: f64,
    at: At,
    power: f64,
    top_power: f64,
    nyquist_floor: Option<f64>,
) -> Section {
    let denominator = |u: f64| (1.0 - u) * (1.0 - u) + k * k * u;
    let u = at.u(g);
    let r = power * denominator(u) - dc * dc;
    let gamma = if at.top >= PI {
        top_power
    } else {
        // Cramer's rule with the top's equation divided through by u_top², so it tends to the
        // Nyquist form as the fit point does.
        let v = at.u_top(g);
        let r_top = top_power * denominator(v) - dc * dc;
        (r_top / (v * v) - r / (u * v)) / (1.0 - u / v)
    };
    let gamma = match nyquist_floor {
        Some(nyquist_power) => gamma.max(0.25 * nyquist_power),
        None => gamma.max(0.0),
    };
    let m0 = gamma.sqrt();
    let beta = (r - gamma * u * u) / u;
    let c_floor = match nyquist_floor {
        Some(_) => INVERSE_DAMPING_FLOOR * INVERSE_DAMPING_FLOOR * m0 * dc,
        None => 0.0,
    };
    let c = (beta + 2.0 * dc * m0).max(c_floor);
    Section::Second {
        g,
        k,
        m0,
        m1: c.sqrt() - m0 * k,
        m2: dc - m0,
    }
}

/// The least damping an inverted section's poles may have: Q 40's, the top of the range.
const INVERSE_DAMPING_FLOOR: f64 = 1.0 / Q_MAX;

/// `1 / section`: the exact inverse, which is stable because every section inverted here has its
/// zeros in the left half-plane (`m0 > 0`, `m0 + m2 > 0`, `m0·k + m1 > 0`).
fn inverse(section: Section) -> Section {
    match section {
        Section::Second { g, k, m0, m1, m2 } => {
            // The numerator m0·s² + (m0·k + m1)·s + (m0 + m2), renormalised as a denominator.
            let natural_squared = (m0 + m2) / m0;
            let natural = natural_squared.sqrt();
            let k_inverse = (m0 * k + m1) / m0 / natural;
            Section::Second {
                g: g * natural,
                k: k_inverse,
                m0: 1.0 / m0,
                m1: (k / natural - k_inverse) / m0,
                m2: (1.0 / natural_squared - 1.0) / m0,
            }
        }
        Section::First { g, m0, m1 } => {
            // m0 + m1/(1 + s) = (m0·s + m0 + m1) / (s + 1); its inverse has its pole at
            // (m0 + m1)/m0.
            let pole = (m0 + m1) / m0;
            Section::First {
                g: g * pole,
                m0: 1.0 / m0,
                m1: (1.0 / pole - 1.0) / m0,
            }
        }
    }
}

/// The bell: centre `at.w`, half-gain-in-dB `q`, `gain_db` at the centre.
fn bell(method: Method, at: At, q: f64, gain_db: f64) -> Section {
    let a = analogue::shelf_a(gain_db);
    match method {
        Method::Bilinear => {
            let k = 1.0 / (q * a);
            Section::Second {
                g: at.tan_w,
                k,
                m0: 1.0,
                m1: k * (a * a - 1.0),
                m2: 0.0,
            }
        }
        Method::Matched if gain_db < 0.0 => inverse(bell(method, at, q, -gain_db)),
        Method::Matched => {
            // The boost's poles: s² + s/(QA) + 1. Vicanek's peaking fit: DC 1, gain G = A² at the
            // centre and the extremum there, which gives m0² = (G²(u² − 1) + 1) / u²; the
            // centre's value then fixes the rest.
            let (g, k) = matched_poles(at.w, 0.5 / (q * a));
            let u = at.u(g);
            let power = a.powi(4);
            let m0_squared = ((power * (u * u - 1.0) + 1.0) / (u * u)).max(0.0);
            let nyquist = at.at_nyquist();
            let analogue_nyquist = analogue::bell(PI / at.w, q, gain_db);
            invertible_fit(g, k, 1.0, nyquist, power, m0_squared, analogue_nyquist)
        }
    }
}

/// The low shelf: midpoint `at.w`, knee `q`, `gain_db` below.
fn low_shelf(method: Method, at: At, q: f64, gain_db: f64) -> Section {
    let a = analogue::shelf_a(gain_db);
    match method {
        Method::Bilinear => {
            let k = 1.0 / q;
            Section::Second {
                g: at.tan_w / a.sqrt(),
                k,
                m0: 1.0,
                m1: k * (a - 1.0),
                m2: a * a - 1.0,
            }
        }
        Method::Matched if gain_db < 0.0 => inverse(low_shelf(method, at, q, -gain_db)),
        Method::Matched => {
            // The boost's poles sit at w/√A.
            let (g, k) = matched_poles(at.w / a.sqrt(), 0.5 / q);
            let top = analogue::low_shelf(at.top_ratio(), q, gain_db);
            let nyquist = analogue::low_shelf(PI / at.w, q, gain_db);
            invertible_fit(g, k, a * a, at, a * a, top, nyquist)
        }
    }
}

/// The high shelf: midpoint `at.w`, knee `q`, `gain_db` above.
fn high_shelf(method: Method, at: At, q: f64, gain_db: f64) -> Section {
    let a = analogue::shelf_a(gain_db);
    match method {
        Method::Bilinear => {
            let k = 1.0 / q;
            Section::Second {
                g: at.tan_w * a.sqrt(),
                k,
                m0: a * a,
                m1: k * (1.0 - a) * a,
                m2: 1.0 - a * a,
            }
        }
        Method::Matched if gain_db > 0.0 => inverse(high_shelf(method, at, q, -gain_db)),
        Method::Matched => {
            // The cut's poles sit at w·√A, below w.
            let (g, k) = matched_poles(at.w * a.sqrt(), 0.5 / q);
            let top = analogue::high_shelf(at.top_ratio(), q, gain_db);
            let nyquist = analogue::high_shelf(PI / at.w, q, gain_db);
            invertible_fit(g, k, 1.0, at, a * a, top, nyquist)
        }
    }
}

/// The notch: exact zeros at `at.w`, −3 dB width `w/q`, unity at DC.
fn notch(method: Method, at: At, q: f64) -> Section {
    let (g, k) = match method {
        Method::Bilinear => (at.tan_w, 1.0 / q),
        Method::Matched => matched_poles(at.w, 0.5 / q),
    };
    // Zeros at ±jΩ(w): the numerator is m0 (s² + Ω²), with m0 + m2 = 1.
    let m0 = 1.0 / at.u(g);
    Section::Second {
        g,
        k,
        m0,
        m1: -m0 * k,
        m2: 1.0 - m0,
    }
}

/// The band pass: unity at `at.w` with its extremum there, −3 dB width `w/q`, an exact zero at DC.
fn band_pass(method: Method, at: At, q: f64) -> Section {
    match method {
        Method::Bilinear => {
            let k = 1.0 / q;
            Section::Second {
                g: at.tan_w,
                k,
                m0: 0.0,
                m1: k,
                m2: 0.0,
            }
        }
        Method::Matched => {
            let (g, k) = matched_poles(at.w, 0.5 / q);
            let u = at.u(g);
            // DC zero (m2 = −m0), unity at the centre and the extremum there: m0² = 1 − 1/u².
            let m0 = (1.0 - 1.0 / (u * u)).max(0.0).sqrt();
            let c = ((1.0 - u) * (1.0 - u) + k * k * u - m0 * m0 * u * u) / u;
            Section::Second {
                g,
                k,
                m0,
                m1: c.max(0.0).sqrt() - m0 * k,
                m2: -m0,
            }
        }
    }
}

/// A second-order low pass at `at.w` with resonance `q`: DC 1, gain `q` at its corner, its
/// analogue value at the fit point.
fn low_pass(method: Method, at: At, q: f64) -> Section {
    match method {
        Method::Bilinear => Section::Second {
            g: at.tan_w,
            k: 1.0 / q,
            m0: 0.0,
            m1: 0.0,
            m2: 1.0,
        },
        Method::Matched => {
            let (g, k) = matched_poles(at.w, 0.5 / q);
            let top = analogue::low_pass(at.top_ratio(), q);
            three_point(g, k, 1.0, at, q * q, top)
        }
    }
}

/// A second-order high pass at `at.w` with resonance `q`: an exact double zero at DC and gain `q`
/// at its corner. With both zeros fixed one degree of freedom is left, spent on the corner.
fn high_pass(method: Method, at: At, q: f64) -> Section {
    let (g, k) = match method {
        Method::Bilinear => (at.tan_w, 1.0 / q),
        Method::Matched => matched_poles(at.w, 0.5 / q),
    };
    let m0 = match method {
        Method::Bilinear => 1.0,
        Method::Matched => {
            let u = at.u(g);
            q * ((1.0 - u) * (1.0 - u) + k * k * u).sqrt() / u
        }
    };
    Section::Second {
        g,
        k,
        m0,
        m1: -m0 * k,
        m2: -m0,
    }
}

// ---- First-order fits ----------------------------------------------------------------------------

/// The one-pole through `|H(0)| = dc` and the powers `power` at `at.w` and `top_power` at
/// `at.top`. A one-pole's power is `(dc² + m0²Ω²) / (1 + Ω²)`, so each point gives
/// `m0² = P + (P − dc²)·g²/tan²(ω/2)`, and two points fix `g` and `m0`. Its pole comes from the fit,
/// not from impulse invariance: a one-pole has exactly three degrees of freedom.
fn first_order_fit(dc: f64, at: At, power: f64, top_power: f64) -> Section {
    let t = at.tan_w;
    let slope = (power - dc * dc) / (t * t);
    let g_squared = if at.top >= PI {
        (top_power - power) / slope
    } else {
        let t_top = at.tan_top;
        (top_power - power) / (slope - (top_power - dc * dc) / (t_top * t_top))
    };
    let g = g_squared.max(f64::MIN_POSITIVE).sqrt();
    let m0 = (power + slope * g_squared).max(0.0).sqrt();
    Section::First { g, m0, m1: dc - m0 }
}

/// A first-order low pass with its corner at `at.w`: exact at DC, at the corner (−3 dB) and at the
/// fit point.
fn first_low_pass(method: Method, at: At) -> Section {
    if method == Method::Bilinear {
        return Section::First {
            g: at.tan_w,
            m0: 0.0,
            m1: 1.0,
        };
    }
    first_order_fit(1.0, at, 0.5, analogue::low_pass_first(at.top_ratio()))
}

/// A first-order high pass with its corner at `at.w`: an exact zero at DC, −3 dB at the corner, and
/// its stopband on the analogue asymptote `ω/w`, which leaves the top free where the analogue
/// curve is already flat. `m0 = 2g/w` sets the asymptote, and the corner then gives
/// `g = w·t / √(8t² − w²)` with `t = tan(w/2)`.
fn first_high_pass(method: Method, at: At) -> Section {
    let t = at.tan_w;
    let (g, m0) = match method {
        Method::Bilinear => (t, 1.0),
        Method::Matched => {
            let g = at.w * t / (8.0 * t * t - at.w * at.w).sqrt();
            (g, 2.0 * g / at.w)
        }
    };
    Section::First { g, m0, m1: -m0 }
}

/// The tilt shelf about the pivot `at.w`: `1/G` at DC, 0 dB at the pivot, `G` toward the top, with
/// `G = 10^(dB/20)`. Matched, it is exact at DC, the pivot and the fit point.
///
/// [`first_order_fit`] would divide 0 by 0 at 0 dB, so the tilt's own fit is written with the
/// `G² − 1` that every term shares cancelled by hand: with `x` the fit point over the pivot and
/// `t` the two points' `tan(ω/2)`, `g² = G²(x² − 1) / ((x² + G²)/t_w² − x²(1 + G²)/t_top²)` and
/// `m0² = 1 + (1 − 1/G²)·g²/t_w²`. At 0 dB that is the identity mix, so the tilt glides through it.
fn tilt(method: Method, at: At, gain_db: f64) -> Section {
    let gain = 10f64.powf(gain_db / 20.0);
    if method == Method::Bilinear {
        return Section::First {
            g: gain * at.tan_w,
            m0: gain,
            m1: 1.0 / gain - gain,
        };
    }
    let x = at.top_ratio();
    let t = at.tan_w;
    let far = if at.top >= PI {
        0.0
    } else {
        let t_top = at.tan_top;
        x * x * (1.0 + gain * gain) / (t_top * t_top)
    };
    let g_squared = gain * gain * (x * x - 1.0) / ((x * x + gain * gain) / (t * t) - far);
    let m0 = (1.0 + (1.0 - 1.0 / (gain * gain)) * g_squared / (t * t))
        .max(0.0)
        .sqrt();
    Section::First {
        g: g_squared.sqrt(),
        m0,
        m1: 1.0 / gain - m0,
    }
}

// ---- Cuts ----------------------------------------------------------------------------------------

/// A Butterworth low cut (`high_pass_cut`) or high cut of the slope's order, corner at `at.w`. Q
/// scales the least damped pair, so 0.707 is exactly Butterworth and the corner's gain is Q itself
/// for every order above the first (0.707 is −3 dB). A first-order cut has no Q.
fn cut(method: Method, high_pass_cut: bool, at: At, q: f64, slope: Slope) -> Cascade {
    let order = slope.order();
    let mut cascade = Cascade::IDENTITY;
    if !order.is_multiple_of(2) {
        cascade.push(if high_pass_cut {
            first_high_pass(method, at)
        } else {
            first_low_pass(method, at)
        });
    }
    for section_q in butterworth_qs(order, q) {
        cascade.push(if high_pass_cut {
            high_pass(method, at, section_q)
        } else {
            low_pass(method, at, section_q)
        });
    }
    cascade
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matched_poles_agree_with_the_direct_impulse_invariant_denominator() {
        for (w, zeta) in [
            (0.3, 0.2),
            (1.2, 0.05),
            (2.9, 0.4),
            (0.5, 1.0),
            (0.4, 3.0),
            (2.0, 20.0),
        ] {
            let (g, k) = matched_poles(w, zeta);
            // Direct: the analogue poles through z = e^{s}.
            let (a1, a2) = if zeta < 1.0 {
                let r = (-zeta * w).exp();
                (-2.0 * r * (w * (1.0 - zeta * zeta).sqrt()).cos(), r * r)
            } else {
                let root = (zeta * zeta - 1.0).sqrt();
                let (r1, r2) = ((-w * (zeta - root)).exp(), (-w * (zeta + root)).exp());
                (-(r1 + r2), r1 * r2)
            };
            // The SVF's own denominator from (g, k).
            let d = 1.0 + k * g + g * g;
            let (b1, b2) = (2.0 * (g * g - 1.0) / d, (1.0 - k * g + g * g) / d);
            assert!(
                (a1 - b1).abs() < 1.0e-12 && (a2 - b2).abs() < 1.0e-12,
                "{w} {zeta}"
            );
        }
    }

    #[test]
    fn the_inverse_of_a_section_cancels_it() {
        // A band at `w` radians: at a 2π Hz rate its fit point is Nyquist.
        let at = |w: f64| At::new(w, 2.0 * PI);
        let sections = [
            bell(Method::Matched, at(0.7), 2.0, 12.0),
            low_shelf(Method::Matched, at(0.2), 0.707, 9.0),
            high_shelf(Method::Matched, at(1.9), 1.5, -15.0),
            tilt(Method::Matched, at(1.0), 7.0),
        ];
        for section in sections {
            let inverse = inverse(section);
            for t in [0.0, 0.01, 0.3, 1.0, 4.0, 50.0, f64::INFINITY] {
                let product = section.power(t) * inverse.power(t);
                assert!(
                    (product - 1.0).abs() < 1.0e-9,
                    "{section:?} at tan {t}: {product}"
                );
            }
        }
    }
}
