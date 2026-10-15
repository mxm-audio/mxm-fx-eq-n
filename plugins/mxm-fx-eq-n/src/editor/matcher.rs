//! F5's EQ match (plan §7.6): the long-term spectra of the side chain, the reference, and of the
//! input before the EQ, their difference smoothed and scaled by an Amount, fitted to bands the
//! player applies. The editor's half alone, as the finder is: it reads the analyser's blocks, and
//! what it proposes the player applies as ordinary bands, so the DSP stays a plain band cascade.
//!
//! - **The long-term spectra** (`research:effects/parametric-equalizers.md` §10.1): each input's
//!   width frames on F4's rungs ([`finder::rung_specs`]), power means over every frame, in linear
//!   units per bin: the reference's channels averaged, the source's covariance (`|L|²`, `|R|²`,
//!   `L·conj(R)`), so the EQ is counted as heard, Mid and Side bands included. Every frame counts,
//!   so each rung's mean is a time average alike: a quiet stretch scales every rung the same, a
//!   constant the level takes out. Only the heard seconds are gated.
//! - **Smoothed** in dB by a Hann window in log-frequency as wide as the wider of a third of an
//!   octave and one ERB (§10.2), so neither a key's notes nor the bass notes are matched.
//! - **The target** `T = Amount × D − E − c`: `D` the difference, `E` what the EQ as heard does,
//!   `c` the level that keeps the source's K-weighted loudness; its error weighted by `w`, 1 over
//!   30 Hz – 16 kHz easing to 0 half an octave beyond, so the extremes ask for nothing.
//! - **The fit** ([`fit`]), Ramos & López's order with Abel & Berners' refinement (§10.4): the
//!   largest region of what is left past the deviation just audible seeds a bell or a shelf, every
//!   band is refined together by Levenberg–Marquardt, again until nothing left is audible, Detail
//!   is reached or the slots run out.

use mxm_fft::Planner;
use mxm_fx_eq_n_dsp::{Cascade, Design, Shape, Slope, design};

use super::finder::{
    self, Decimator, Frames, HeardEq, Job, LOWEST_HZ, RungSpec, WIDTH_POINTS, heard, usable_rate,
};
use crate::analyser::{BLOCK_FRAMES, Block};

/// Proposals appear once each input has been heard this long.
pub const LISTEN_SECONDS: f64 = 10.0;
/// Amount's default, in percent (decision 33): Ozone advises under 50 %, Melda 30–40 % (§10.3).
pub const DEFAULT_AMOUNT: u8 = 40;
/// Detail's default, at most how many bands (decision 33): Ramos & López's error was about 0.5 dB
/// by five to ten sections (§10.4).
pub const DEFAULT_DETAIL: u8 = 8;
/// Detail's most, the EQ's slots.
pub const MAX_DETAIL: u8 = 24;
/// The range matched; outside it the target eases to nothing over [`EASE_OCTAVES`].
pub const RANGE: (f32, f32) = (30.0, 16_000.0);
pub const EASE_OCTAVES: f32 = 0.5;
/// The fit's points per octave.
pub const POINTS_PER_OCTAVE: f32 = 24.0;
/// A proposal's gain is held inside ±this (Ramos & López's cap, §7.5's).
pub const MAX_GAIN_DB: f32 = 12.0;
/// A bell's Q is held inside this: narrower is detail the smoothing removed.
pub const Q_RANGE: (f32, f32) = (0.3, 4.0);
/// A shelf's Q.
pub const SHELF_Q: f32 = 0.71;
/// The refinement's iterations, each at most [`TRIES`] tries of its damping.
pub const ITERATIONS: usize = 16;
const TRIES: usize = 6;
/// The bisection for the level: its span either side of 0 dB and its halvings.
const LEVEL_SPAN_DB: f64 = 60.0;
const LEVEL_HALVINGS: usize = 60;

/// The cells the spectra are read on, 1/48 octave from [`LOWEST_HZ`], as F4's.
const CELLS_PER_OCTAVE: f32 = 48.0;
const CELLS: usize = 528;

pub(crate) fn cell_freq(cell: usize) -> f32 {
    LOWEST_HZ * 2f32.powf(cell as f32 / CELLS_PER_OCTAVE)
}

/// One input's rung as it listens.
struct ListenRung {
    rate: f32,
    decimator: Option<Decimator>,
    width: Frames,
}

/// What one rung has heard of both inputs, summed over each input's width frames, per width bin in
/// linear units: the source's covariance and the reference's channels averaged.
#[derive(Clone)]
struct RungSums {
    low: f32,
    high: f32,
    bin_hz: f32,
    /// Frames: the source's, then the reference's.
    frames: [u32; 2],
    ll: Vec<f64>,
    rr: Vec<f64>,
    lr: Vec<(f64, f64)>,
    reference: Vec<f64>,
}

/// Everything Match has heard. Plain data, so a fit runs on a copy away from the editor's frame.
#[derive(Clone)]
pub struct Spectra {
    channels: usize,
    rungs: Vec<RungSums>,
    /// Seconds heard: the source's, then the reference's.
    heard: [f64; 2],
}

impl Spectra {
    /// Whether each input has been heard long enough for proposals (plan §7.6: ten seconds).
    pub fn listened(&self) -> bool {
        self.heard.iter().all(|&seconds| seconds >= LISTEN_SECONDS)
    }

    /// The seconds heard: the input's, then the reference's.
    pub fn heard_seconds(&self) -> [f64; 2] {
        self.heard
    }
}

/// The source and the reference, `0` and `1` of each pair.
const SOURCE: usize = 0;
const REFERENCE: usize = 1;

/// The matcher: everything heard since Match was turned on.
pub struct Matcher {
    sample_rate: f32,
    /// Each input's rungs, fastest first, each fed from the one above.
    inputs: [Vec<ListenRung>; 2],
    expected_seq: Option<u64>,
    generation: Option<u32>,
    revision: u64,
    listen: u64,
    spectra: Spectra,
}

impl Matcher {
    /// A matcher for audio at `sample_rate`, any rate the plugin runs at.
    pub fn new(sample_rate: f32) -> Matcher {
        let sample_rate = usable_rate(sample_rate);
        let mut planner = Planner::new();
        let specs = finder::rung_specs(sample_rate);
        let input = |planner: &mut Planner<f32>| -> Vec<ListenRung> {
            specs
                .iter()
                .map(|spec| ListenRung {
                    rate: spec.rate,
                    decimator: spec
                        .decimation
                        .map(|step| Decimator::new(step, spec.rate * step as f32)),
                    width: Frames::new(planner, WIDTH_POINTS),
                })
                .collect()
        };
        let inputs = [input(&mut planner), input(&mut planner)];
        let bins = WIDTH_POINTS / 2 + 1;
        let rungs = specs
            .iter()
            .map(
                |&RungSpec {
                     rate, low, high, ..
                 }| RungSums {
                    low,
                    high,
                    bin_hz: rate / WIDTH_POINTS as f32,
                    frames: [0; 2],
                    ll: vec![0.0; bins],
                    rr: vec![0.0; bins],
                    lr: vec![(0.0, 0.0); bins],
                    reference: vec![0.0; bins],
                },
            )
            .collect();
        Matcher {
            sample_rate,
            inputs,
            expected_seq: None,
            generation: None,
            revision: 0,
            listen: 0,
            spectra: Spectra {
                channels: 2,
                rungs,
                heard: [0.0; 2],
            },
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// What it has heard, for a fit.
    pub fn spectra(&self) -> &Spectra {
        &self.spectra
    }

    /// Changes whenever what it has heard does.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Counts its listens, as the finder's do: a fit carries the one it was made in.
    pub fn listen(&self) -> u64 {
        self.listen
    }

    /// Forgets everything heard: Match turned on again, a reset, another rate.
    pub fn restart(&mut self) {
        self.restart_windows();
        self.expected_seq = None;
        self.generation = None;
        self.revision = self.revision.wrapping_add(1);
        self.listen = self.listen.wrapping_add(1);
        self.spectra.heard = [0.0; 2];
        for rung in &mut self.spectra.rungs {
            rung.frames = [0; 2];
            rung.ll.fill(0.0);
            rung.rr.fill(0.0);
            rung.lr.fill((0.0, 0.0));
            rung.reference.fill(0.0);
        }
    }

    /// A gap in the audio: the frames start again, never spliced across it.
    fn restart_windows(&mut self) {
        for input in &mut self.inputs {
            for rung in input {
                if let Some(decimator) = &mut rung.decimator {
                    decimator.restart();
                }
                rung.width.restart();
            }
        }
    }

    /// One block of the analyser's: its input before the EQ and its side chain. A block at another
    /// rate is not this matcher's; a new generation (a reset) starts the listen again; a dropped
    /// block restarts the frames.
    pub fn ingest(&mut self, block: &Block) {
        if block.sample_rate != self.sample_rate {
            return;
        }
        if self
            .generation
            .is_some_and(|generation| generation != block.generation)
        {
            self.restart();
        } else if self
            .expected_seq
            .is_some_and(|expected| block.seq != expected)
        {
            self.restart_windows();
        }
        self.generation = Some(block.generation);
        self.expected_seq = Some(block.seq.wrapping_add(1));
        self.spectra.channels = usize::from(block.channels.clamp(1, 2));
        let clean = |x: f32| if x.is_finite() { x } else { 0.0 };
        for index in 0..BLOCK_FRAMES {
            let source = [block.pre[0][index], block.pre[1][index]].map(clean);
            let reference = [block.key[0][index], block.key[1][index]].map(clean);
            self.push_rung(SOURCE, 0, source);
            self.push_rung(REFERENCE, 0, reference);
        }
    }

    fn push_rung(&mut self, input: usize, index: usize, frame: [f32; 2]) {
        let rung = &mut self.inputs[input][index];
        if rung.width.push(frame[0], frame[1]) {
            self.width_frame(input, index);
        }
        if index + 1 < self.inputs[input].len()
            && let Some(next) = self.inputs[input][index + 1]
                .decimator
                .as_mut()
                .and_then(|decimator| decimator.push(frame))
        {
            self.push_rung(input, index + 1, next);
        }
    }

    /// A width frame: its power joins the input's long-term sums, whatever its level, so every
    /// rung's mean is a time average alike (a frame gated as F4's would weigh a quiet passage
    /// differently on rungs of different lengths, a step at their edges); so every frame is
    /// something new. Only a heard one counts towards the seconds heard.
    fn width_frame(&mut self, input: usize, index: usize) {
        let channels = self.spectra.channels;
        let rung = &mut self.inputs[input][index];
        self.revision = self.revision.wrapping_add(1);
        if heard(rung.width.transform(channels)) && index == 0 {
            self.spectra.heard[input] += rung.width.hop as f64 / f64::from(rung.rate);
        }
        let sums = &mut self.spectra.rungs[index];
        sums.frames[input] += 1;
        if input == SOURCE {
            rung.width
                .add_covariance(channels, &mut sums.ll, &mut sums.rr, &mut sums.lr);
        } else {
            rung.width.add_power(channels, &mut sums.reference);
        }
    }
}

// ---- the curves ------------------------------------------------------------------------------

/// One cell's long-term densities, per hertz, in linear units: the source's channels averaged, the
/// reference's, and the source's through the EQ as heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellDensity {
    pub source: f64,
    pub reference: f64,
    pub through: f64,
}

/// The output power of the EQ's 2×2 transfer on the source's covariance, the channels averaged
/// (plan §7.6): the channel stage `diag(dl, dr)`, then mid/side's `diag(dm, ds)`, so
/// `y_L = a·dl·x_L + b·dr·x_R`, `y_R = b·dl·x_L + a·dr·x_R`, `a = (dm + ds)/2`, `b = (dm − ds)/2`.
pub fn through_power(stages: [(f64, f64); 4], ll: f64, rr: f64, lr: (f64, f64)) -> f64 {
    let [dl, dr, dm, ds] = stages;
    let times = |(a, b): (f64, f64), (c, d): (f64, f64)| (a * c - b * d, a * d + b * c);
    let a = (0.5 * (dm.0 + ds.0), 0.5 * (dm.1 + ds.1));
    let b = (0.5 * (dm.0 - ds.0), 0.5 * (dm.1 - ds.1));
    // E|p·x_L + q·x_R|² = |p|²·S_LL + |q|²·S_RR + 2·Re(p·conj(q)·S_LR).
    let power = |p: (f64, f64), q: (f64, f64)| {
        let pq = times(p, (q.0, -q.1));
        (p.0 * p.0 + p.1 * p.1) * ll
            + (q.0 * q.0 + q.1 * q.1) * rr
            + 2.0 * (pq.0 * lr.0 - pq.1 * lr.1)
    };
    let left = power(times(a, dl), times(b, dr));
    let right = power(times(b, dl), times(a, dr));
    (0.5 * (left + right)).max(0.0)
}

impl Spectra {
    /// Each cell's densities, `None` where it was not heard by both inputs: a cell belongs to the
    /// rung whose own frequencies hold its centre, and reads the bins inside its 1/48 octave, or
    /// where it is narrower than a bin, the bin nearest its centre.
    pub fn densities(&self, heard_eq: &HeardEq) -> Vec<Option<CellDensity>> {
        let half = 2f32.powf(1.0 / (2.0 * CELLS_PER_OCTAVE));
        let bands: Vec<_> = heard_eq.bands.iter().map(|(c, p)| (c, *p)).collect();
        let mut cells = vec![None; CELLS];
        for (cell, out) in cells.iter_mut().enumerate() {
            let centre = cell_freq(cell);
            let Some(rung) = self
                .rungs
                .iter()
                .find(|rung| centre >= rung.low && centre < rung.high)
            else {
                continue;
            };
            let [sources, references] = rung.frames;
            if sources == 0 || references == 0 {
                continue;
            }
            let bins = rung.ll.len();
            let first = ((centre / half / rung.bin_hz).ceil() as usize).max(1);
            let end = ((centre * half / rung.bin_hz).ceil() as usize).min(bins);
            let range = if first < end {
                first..end
            } else {
                let nearest = ((centre / rung.bin_hz).round() as usize).clamp(1, bins - 1);
                nearest..nearest + 1
            };
            let (n_source, n_reference) = (f64::from(sources), f64::from(references));
            let mut sum = CellDensity {
                source: 0.0,
                reference: 0.0,
                through: 0.0,
            };
            for bin in range.clone() {
                let (ll, rr) = (rung.ll[bin] / n_source, rung.rr[bin] / n_source);
                let lr = (rung.lr[bin].0 / n_source, rung.lr[bin].1 / n_source);
                let source = 0.5 * (ll + rr);
                sum.source += source;
                sum.reference += rung.reference[bin] / n_reference;
                sum.through += if bands.is_empty() {
                    source
                } else {
                    let stages = finder::stages(
                        bands.iter().copied(),
                        bin as f32 * rung.bin_hz,
                        heard_eq.sample_rate,
                    );
                    through_power(stages, ll, rr, lr)
                };
            }
            let per = (range.len() as f64 * f64::from(rung.bin_hz)).recip();
            *out = Some(CellDensity {
                source: sum.source * per,
                reference: sum.reference * per,
                through: sum.through * per,
            });
        }
        cells
    }
}

/// The smoothing's width at `freq`, in octaves (plan §7.6): the wider of a third of an octave and
/// one ERB, `24.7 + 0.108 f` Hz, as octaves `log2((f + ERB/2)/(f − ERB/2))`.
pub fn smoothing_octaves(freq: f32) -> f32 {
    let erb = 24.7 + 0.108 * freq;
    let below = freq - erb / 2.0;
    let erb_octaves = if below > 0.0 {
        ((freq + erb / 2.0) / below).log2()
    } else {
        f32::INFINITY
    };
    erb_octaves.clamp(1.0 / 3.0, 3.0)
}

/// `values` per cell smoothed in dB by a Hann window in log-frequency [`smoothing_octaves`] wide,
/// over the cells that have one; `None` stays `None`.
pub fn smooth(values: &[Option<f32>]) -> Vec<Option<f32>> {
    (0..values.len())
        .map(|cell| {
            values[cell]?;
            let width = smoothing_octaves(cell_freq(cell));
            let reach = (width / 2.0 * CELLS_PER_OCTAVE).floor() as usize;
            let (mut sum, mut weights) = (0.0f64, 0.0f64);
            let first = cell.saturating_sub(reach);
            let last = (cell + reach).min(values.len() - 1);
            for (other, value) in values.iter().enumerate().take(last + 1).skip(first) {
                let Some(value) = *value else {
                    continue;
                };
                let octaves = (other as f32 - cell as f32) / CELLS_PER_OCTAVE;
                let weight = 0.5 + 0.5 * f64::from((std::f32::consts::TAU * octaves / width).cos());
                sum += weight * f64::from(value);
                weights += weight;
            }
            (weights > 0.0).then(|| (sum / weights) as f32)
        })
        .collect()
}

/// What the fit is asked for in one cell: the smoothed difference `D`, the smoothed EQ as heard
/// `E`, and the source's power in the cell (linear, its whole 1/48 octave) for the loudness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellCurve {
    pub d: f32,
    pub e: f32,
    pub power: f64,
}

/// The cells' smoothed levels in dB — the source's, the reference's and the source's through the
/// EQ as heard — beside their densities.
pub struct Levels {
    pub densities: Vec<Option<CellDensity>>,
    pub source: Vec<Option<f32>>,
    pub reference: Vec<Option<f32>>,
    pub through: Vec<Option<f32>>,
}

/// [`Levels`] of what was heard.
pub fn levels(spectra: &Spectra, heard_eq: &HeardEq) -> Levels {
    let densities = spectra.densities(heard_eq);
    let db = |pick: fn(&CellDensity) -> f64| -> Vec<Option<f32>> {
        densities
            .iter()
            .map(|cell| cell.map(|cell| (10.0 * pick(&cell).max(1.0e-30).log10()) as f32))
            .collect()
    };
    Levels {
        source: smooth(&db(|cell| cell.source)),
        reference: smooth(&db(|cell| cell.reference)),
        through: smooth(&db(|cell| cell.through)),
        densities,
    }
}

/// The cells' curves (plan §7.6), `None` where a cell was not heard.
pub fn curves(spectra: &Spectra, heard_eq: &HeardEq) -> Vec<Option<CellCurve>> {
    let levels = levels(spectra, heard_eq);
    let width = 2f64.powf(1.0 / (2.0 * f64::from(CELLS_PER_OCTAVE)));
    (0..levels.densities.len())
        .map(|cell| {
            let density = levels.densities[cell]?;
            let centre = f64::from(cell_freq(cell));
            Some(CellCurve {
                d: levels.reference[cell]? - levels.source[cell]?,
                e: levels.through[cell]? - levels.source[cell]?,
                power: density.source * centre * (width - width.recip()),
            })
        })
        .collect()
}

/// The ease (plan §7.6), the weight of the fit's error: 1 over [`RANGE`], falling to 0 by a raised
/// cosine over [`EASE_OCTAVES`] beyond each end, so the extremes ask for nothing — neither the
/// difference measured there nor its absence — and a shelf may run on past an end.
pub fn ease(freq: f32) -> f32 {
    let beyond = if freq < RANGE.0 {
        (RANGE.0 / freq).log2()
    } else if freq > RANGE.1 {
        (freq / RANGE.1).log2()
    } else {
        return 1.0;
    };
    if beyond >= EASE_OCTAVES {
        0.0
    } else {
        0.5 + 0.5 * (std::f32::consts::PI * beyond / EASE_OCTAVES).cos()
    }
}

/// The K-weighting's power gain at `freq` (ITU-R BS.1770): its shelf and its high-pass, from their
/// analogue prototypes, so it holds at every rate.
pub fn k_weight(freq: f64) -> f64 {
    // The shelf: +4 dB from about 1.7 kHz.
    let (f0, gain_db, q) = (
        1_681.974_450_955_533,
        3.999_843_853_973_347,
        0.707_175_236_955_419_6,
    );
    let a = 10f64.powf(gain_db / 40.0);
    let w = freq / f0;
    // H(s) = A·(A·s² + (√A/Q)·s + 1)/(s² + (√A/Q)·s + A), s = jw.
    let num = (1.0 - a * w * w).powi(2) + (a.sqrt() / q * w).powi(2);
    let den = (a - w * w).powi(2) + (a.sqrt() / q * w).powi(2);
    let shelf = a * a * num / den;
    // The high-pass: about 38 Hz. H(s) = s²/(s² + s/Q + 1).
    let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let w = freq / f0;
    let high_pass = w.powi(4) / ((1.0 - w * w).powi(2) + (w / q).powi(2));
    shelf * high_pass
}

/// The level `c` (plan §7.6): the constant that, taken out after the Amount, leaves the source's
/// K-weighted power through the EQ and the target what it is through the EQ alone. By bisection,
/// the ease included.
pub fn level_offset(cells: &[Option<CellCurve>], amount: f32) -> f32 {
    let weighed: Vec<(f64, f64, f64, f64)> = cells
        .iter()
        .enumerate()
        .filter_map(|(cell, curve)| {
            let curve = (*curve)?;
            let freq = cell_freq(cell);
            let w = f64::from(ease(freq));
            (w > 0.0).then(|| {
                (
                    k_weight(f64::from(freq)) * curve.power,
                    w,
                    f64::from(amount) * f64::from(curve.d),
                    f64::from(curve.e),
                )
            })
        })
        .collect();
    let before: f64 = weighed
        .iter()
        .map(|&(power, _, _, e)| power * 10f64.powf(e / 10.0))
        .sum();
    if before.is_nan() || before <= 0.0 {
        return 0.0;
    }
    let after = |c: f64| -> f64 {
        weighed
            .iter()
            .map(|&(power, w, d, e)| power * 10f64.powf((e + w * (d - e - c)) / 10.0))
            .sum()
    };
    let (mut low, mut high) = (-LEVEL_SPAN_DB, LEVEL_SPAN_DB);
    for _ in 0..LEVEL_HALVINGS {
        let mid = 0.5 * (low + high);
        // More power after than before: the level comes out further.
        if after(mid) > before {
            low = mid;
        } else {
            high = mid;
        }
    }
    (0.5 * (low + high)) as f32
}

/// The fit's points (plan §7.6): a twenty-fourth of an octave apart over the range and its ease,
/// none above the DSP's `max_frequency` at `sample_rate`.
pub fn points(sample_rate: f64) -> Vec<f32> {
    let reach = 2f32.powf(EASE_OCTAVES);
    let (first, last) = (
        RANGE.0 / reach,
        (RANGE.1 * reach).min(mxm_fx_eq_n_dsp::max_frequency(sample_rate) as f32),
    );
    let step = 2f32.powf(1.0 / POINTS_PER_OCTAVE);
    std::iter::successors(Some(first), |&freq| Some(freq * step))
        .take_while(|&freq| freq <= last * (1.0 + 1.0e-6))
        .collect()
}

/// The target at `points` (plan §7.6), `Amount × D − E − c`, from the cells' curves read between
/// cells in log-frequency; 0 where nothing was heard.
pub fn target(cells: &[Option<CellCurve>], points: &[f32], amount: f32, level: f32) -> Vec<f32> {
    points
        .iter()
        .map(|&freq| {
            let at = (freq / LOWEST_HZ).log2() * CELLS_PER_OCTAVE;
            let below = at.floor().max(0.0) as usize;
            let t = at - below as f32;
            let value = |cell: usize| {
                cells
                    .get(cell)
                    .copied()
                    .flatten()
                    .map(|curve| amount * curve.d - curve.e - level)
            };
            match (value(below), value(below + 1)) {
                (Some(a), Some(b)) => a + (b - a) * t,
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => 0.0,
            }
        })
        .collect()
}

/// The fit's weights at `points`: the [`ease`].
pub fn weights(points: &[f32]) -> Vec<f32> {
    points.iter().map(|&freq| ease(freq)).collect()
}

/// What is left of `target` once `responses` are in, weighted: what the regions are read on.
pub fn left_weighted(target: &[f32], weights: &[f32], responses: &[Vec<f64>]) -> Vec<f32> {
    (0..target.len())
        .map(|at| {
            let reached: f64 = responses.iter().map(|r| r[at]).sum();
            weights[at] * (target[at] - reached as f32)
        })
        .collect()
}

// ---- the fit ---------------------------------------------------------------------------------

/// One band of a match: a bell or a shelf, its gain as heard, after the gain scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchBand {
    pub shape: Shape,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl MatchBand {
    pub fn cascade(&self, sample_rate: f64) -> Cascade {
        design(
            Design {
                shape: self.shape,
                slope: Slope::Db12,
                freq: f64::from(self.freq),
                gain_db: f64::from(self.gain_db),
                q: f64::from(self.q),
            },
            sample_rate,
        )
    }

    fn responses(&self, points: &[f32], sample_rate: f64) -> Vec<f64> {
        let cascade = self.cascade(sample_rate);
        points
            .iter()
            .map(|&freq| cascade.response_db(f64::from(freq), sample_rate))
            .collect()
    }

    /// Its parameters as the refinement moves them: log2 frequency, gain and, for a bell, log2 Q.
    fn parameters(&self) -> Vec<f64> {
        let mut p = vec![f64::from(self.freq.log2()), f64::from(self.gain_db)];
        if self.shape == Shape::Bell {
            p.push(f64::from(self.q.log2()));
        }
        p
    }

    /// The band at `p`, held inside its bounds.
    fn at(&self, p: &[f64], max_gain: f32) -> MatchBand {
        let mut band = *self;
        band.freq = 2f32.powf(p[0] as f32).clamp(RANGE.0, RANGE.1);
        band.gain_db = (p[1] as f32).clamp(-max_gain, max_gain);
        if self.shape == Shape::Bell {
            band.q = 2f32.powf(p[2] as f32).clamp(Q_RANGE.0, Q_RANGE.1);
        }
        band
    }
}

/// The forward differences' steps: log2 frequency, gain, log2 Q (F4's `fit_bell`'s).
const STEPS: [f64; 3] = [1.0e-4, 1.0e-3, 1.0e-3];

/// The deviation just audible on music for a bump of `q` (Toole & Olive, research §8.8), F4's.
pub fn just_audible(q: f32) -> f32 {
    1.5 * q.powf(0.3)
}

/// A region of what is left: its points, sign, extreme and the band it seeds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub start: usize,
    pub end: usize,
    pub area: f32,
    pub seed: MatchBand,
    /// Whether its extreme passes the deviation just audible for its width.
    pub counts: bool,
}

/// The regions of `left` at `points` (plan §7.6, step 1–2): maximal runs of one strict sign, each
/// with its extreme, half-height crossings, Q, area and the band it seeds.
pub fn regions(left: &[f32], points: &[f32], max_gain: f32) -> Vec<Region> {
    let logs: Vec<f32> = points.iter().map(|freq| freq.log2()).collect();
    let mut regions = Vec::new();
    let mut start = 0;
    while start < left.len() {
        let sign = left[start].signum();
        if left[start] == 0.0 {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < left.len() && left[end] != 0.0 && left[end].signum() == sign {
            end += 1;
        }
        // The extreme, the lowest on a tie.
        let mut extreme = start;
        for at in start..end {
            if left[at].abs() > left[extreme].abs() {
                extreme = at;
            }
        }
        let peak = left[extreme].abs();
        let half = peak / 2.0;
        // Walking out from the extreme, where it first falls to half, read on the region's sign so
        // the walk may pass the region's own end.
        let signed = |at: usize| sign * left[at];
        let crossing = |towards: &mut dyn Iterator<Item = usize>| -> Option<f32> {
            let mut before = extreme;
            for at in towards {
                if signed(at) <= half {
                    let (a, b) = (signed(before), signed(at));
                    let t = (a - half) / (a - b).max(f32::MIN_POSITIVE);
                    return Some(2f32.powf(logs[before] + (logs[at] - logs[before]) * t));
                }
                before = at;
            }
            None
        };
        let below = crossing(&mut (0..extreme).rev());
        let above = crossing(&mut (extreme + 1..left.len()));
        let low_end = below.is_none_or(|freq| freq < RANGE.0);
        let high_end = above.is_none_or(|freq| freq > RANGE.1);
        let at = points[extreme];
        let gain = (sign * peak).clamp(-max_gain, max_gain);
        let seed = if low_end || high_end {
            let shape = match (low_end, high_end) {
                (true, true) => {
                    if (at / RANGE.0).log2() <= (RANGE.1 / at).log2() {
                        Shape::LowShelf
                    } else {
                        Shape::HighShelf
                    }
                }
                (true, false) => Shape::LowShelf,
                _ => Shape::HighShelf,
            };
            let corner = match (shape, below, above) {
                (Shape::LowShelf, _, Some(above)) if !high_end => above,
                (Shape::HighShelf, Some(below), _) if !low_end => below,
                _ => at,
            };
            MatchBand {
                shape,
                freq: corner.clamp(RANGE.0, RANGE.1),
                gain_db: gain,
                q: SHELF_Q,
            }
        } else {
            let (below, above) = (below.unwrap_or(at), above.unwrap_or(at));
            let width = 2f32.powf((above / below).log2().max(1.0e-3));
            let q = (width.sqrt() / (width - 1.0)).clamp(Q_RANGE.0, Q_RANGE.1);
            MatchBand {
                shape: Shape::Bell,
                freq: at.clamp(RANGE.0, RANGE.1),
                gain_db: gain,
                q,
            }
        };
        regions.push(Region {
            start,
            end,
            area: left[start..end].iter().map(|v| v.abs()).sum::<f32>() / POINTS_PER_OCTAVE,
            seed,
            counts: peak > just_audible(seed.q),
        });
        start = end;
    }
    regions
}

/// The bands for `target` at `points` (plan §7.6), its error weighted by `weights`: seeded one at a
/// time from the largest region of what is left that counts, every band refined together after
/// each, until none counts or there are `max_bands`; `each` sees the bands after each one's
/// refinement.
pub fn fit_each(
    target: &[f32],
    weights: &[f32],
    points: &[f32],
    sample_rate: f64,
    max_bands: usize,
    max_gain: f32,
    mut each: impl FnMut(&[MatchBand]),
) -> Vec<MatchBand> {
    let max_gain = max_gain.min(MAX_GAIN_DB);
    let mut bands: Vec<MatchBand> = Vec::new();
    if max_gain <= 0.05 || points.is_empty() {
        return bands;
    }
    let mut responses: Vec<Vec<f64>> = Vec::new();
    while bands.len() < max_bands {
        let left = left_weighted(target, weights, &responses);
        let Some(region) = regions(&left, points, max_gain)
            .into_iter()
            .filter(|region| region.counts)
            .fold(None, |best: Option<Region>, region| match best {
                Some(best) if best.area >= region.area => Some(best),
                _ => Some(region),
            })
        else {
            break;
        };
        responses.push(region.seed.responses(points, sample_rate));
        bands.push(region.seed);
        refine(
            &mut bands,
            &mut responses,
            (target, weights),
            points,
            sample_rate,
            max_gain,
        );
        each(&bands);
    }
    bands
}

/// [`fit_each`], the bands alone.
pub fn fit(
    target: &[f32],
    weights: &[f32],
    points: &[f32],
    sample_rate: f64,
    max_bands: usize,
    max_gain: f32,
) -> Vec<MatchBand> {
    fit_each(
        target,
        weights,
        points,
        sample_rate,
        max_bands,
        max_gain,
        |_| {},
    )
}

/// The weighted squared error.
fn cost((target, weights): (&[f32], &[f32]), responses: &[Vec<f64>]) -> f64 {
    (0..target.len())
        .map(|at| {
            let error = f64::from(weights[at])
                * (f64::from(target[at]) - responses.iter().map(|r| r[at]).sum::<f64>());
            error * error
        })
        .sum()
}

/// Every band refined together (plan §7.6, step 3): Levenberg–Marquardt on the points' weighted dB
/// error,
/// F4's `fit_bell` scheme — forward differences at fixed steps, Marquardt's damping from 10⁻³
/// (×10 on a rejected step, ÷3 on an accepted one, [`TRIES`] tries an iteration), the damped
/// normal equations by Gaussian elimination with partial pivoting, each step held to the bounds —
/// [`ITERATIONS`] iterations, no early exit.
fn refine(
    bands: &mut [MatchBand],
    responses: &mut [Vec<f64>],
    goal: (&[f32], &[f32]),
    points: &[f32],
    sample_rate: f64,
    max_gain: f32,
) {
    let (target, weights) = goal;
    let mut lambda = 1.0e-3;
    let mut current = cost(goal, responses);
    for _ in 0..ITERATIONS {
        // The weighted residuals and Jacobian, a column a parameter.
        let left: Vec<f64> = (0..points.len())
            .map(|at| {
                f64::from(weights[at])
                    * (f64::from(target[at]) - responses.iter().map(|r| r[at]).sum::<f64>())
            })
            .collect();
        let mut columns: Vec<Vec<f64>> = Vec::new();
        let mut owners: Vec<(usize, usize)> = Vec::new();
        for (index, band) in bands.iter().enumerate() {
            let p = band.parameters();
            for k in 0..p.len() {
                let mut shifted = p.clone();
                shifted[k] += STEPS[k];
                let moved = band.at(&shifted, max_gain).responses(points, sample_rate);
                columns.push(
                    moved
                        .iter()
                        .zip(&responses[index])
                        .zip(weights)
                        .map(|((after, before), &w)| f64::from(w) * (after - before) / STEPS[k])
                        .collect(),
                );
                owners.push((index, k));
            }
        }
        let n = columns.len();
        let mut jtj = vec![vec![0.0f64; n]; n];
        let mut jtr = vec![0.0f64; n];
        for i in 0..n {
            jtr[i] = columns[i].iter().zip(&left).map(|(j, r)| j * r).sum();
            for k in i..n {
                let value: f64 = columns[i].iter().zip(&columns[k]).map(|(a, b)| a * b).sum();
                jtj[i][k] = value;
                jtj[k][i] = value;
            }
        }
        for _ in 0..TRIES {
            let mut system = jtj.clone();
            for (k, row) in system.iter_mut().enumerate() {
                row[k] += lambda * jtj[k][k].max(1.0e-9);
            }
            let Some(delta) = solve(system, jtr.clone()) else {
                lambda *= 10.0;
                continue;
            };
            let mut trial: Vec<MatchBand> = bands.to_vec();
            for (index, band) in bands.iter().enumerate() {
                let mut p = band.parameters();
                for (column, &(owner, k)) in owners.iter().enumerate() {
                    if owner == index {
                        p[k] += delta[column];
                    }
                }
                trial[index] = band.at(&p, max_gain);
            }
            let trial_responses: Vec<Vec<f64>> = trial
                .iter()
                .map(|band| band.responses(points, sample_rate))
                .collect();
            let trial_cost = cost(goal, &trial_responses);
            if trial_cost < current {
                bands.copy_from_slice(&trial);
                for (response, trial) in responses.iter_mut().zip(trial_responses) {
                    *response = trial;
                }
                current = trial_cost;
                lambda = (lambda / 3.0).max(1.0e-9);
                break;
            }
            lambda *= 10.0;
        }
    }
}

/// `system · x = rhs` by Gaussian elimination with partial pivoting; `None` when singular.
fn solve(mut system: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let n = rhs.len();
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|&a, &b| system[a][column].abs().total_cmp(&system[b][column].abs()))?;
        if system[pivot][column].abs() < 1.0e-30 {
            return None;
        }
        system.swap(column, pivot);
        rhs.swap(column, pivot);
        let (upper, lower) = system.split_at_mut(column + 1);
        let pivot_row = &upper[column];
        for (offset, row) in lower.iter_mut().enumerate() {
            let factor = row[column] / pivot_row[column];
            if factor != 0.0 {
                for (value, &above) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                    *value -= factor * above;
                }
                rhs[column + 1 + offset] -= factor * rhs[column];
            }
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| system[row][k] * x[k]).sum();
        x[row] = (rhs[row] - sum) / system[row][row];
    }
    x.iter().all(|value| value.is_finite()).then_some(x)
}

// ---- the job, away from the editor's frame ---------------------------------------------------

/// One fit for Match (plan §7.6), on `spectra`, a copy of what the matcher has heard.
pub struct Fit {
    pub spectra: Spectra,
    pub heard: HeardEq,
    /// Amount, 0 to 1.
    pub amount: f32,
    /// At most how many bands: Detail, or the free slots where fewer.
    pub max_bands: usize,
    /// The listen it was made in ([`Matcher::listen`]).
    pub listen: u64,
    /// What it was made on besides what was heard: the EQ, the free slots, Amount and Detail.
    pub basis: Vec<u32>,
}

/// What a fit proposes, in which listen and on what.
pub struct Fitted {
    pub bands: Vec<MatchBand>,
    pub listen: u64,
    pub basis: Vec<u32>,
}

impl Fit {
    /// Runs it, where it is.
    pub fn run(self) -> Fitted {
        let bands = if self.spectra.listened() {
            let cells = curves(&self.spectra, &self.heard);
            let level = level_offset(&cells, self.amount);
            let points = points(self.heard.sample_rate);
            let target = target(&cells, &points, self.amount, level);
            fit(
                &target,
                &weights(&points),
                &points,
                self.heard.sample_rate,
                self.max_bands,
                self.heard.deepest_db,
            )
        } else {
            Vec::new()
        };
        Fitted {
            bands,
            listen: self.listen,
            basis: self.basis,
        }
    }
}

impl Job for Fit {
    type Done = Fitted;
    fn run(self) -> Fitted {
        Fit::run(self)
    }
}

#[cfg(test)]
mod tests;
