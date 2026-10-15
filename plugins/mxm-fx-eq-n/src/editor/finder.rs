//! F4's resonance finder (plan §7.5): a long listen to the input before the EQ that proposes cut
//! bells at the peaks that stay put while the music moves. The editor's half alone, as the
//! analyser is: it reads the analyser's pre-EQ blocks, and what it proposes the player applies as
//! ordinary bands, so the DSP stays a plain band cascade (plan §4).
//!
//! - **Three rungs** (plan §7.5's table), each a frequency's only home: the input decimated to at
//!   most 64 kHz, then a quarter and a sixteenth of that, read from 1.6 kHz, from 400 Hz, and below.
//!   Each decimation is the display's anti-alias low pass, run on left and right; mid and side are
//!   formed from their spectra, a linear step.
//! - **Two kinds of evidence** (`research:effects/parametric-equalizers.md` §8.6): 8,192-point
//!   frames for width, since a measured width is near the true one only once a frame is about 4/B
//!   seconds, and for movement about the shortest frame that tells notes a semitone apart in each
//!   octave — 2,048 points, and 1,024 in the lowest rung's top octave. Hann windows, a hop of half a
//!   frame; only frames whose input stands at −60 dBFS or above count for anything.
//! - **Five spectra**: left, right, mid, side and the channels' power average.
//! - **The long-term spectrum is robust to the notes' lines**, since a resonance is the shape the
//!   sound passes through, not the notes on it: every heard width frame's level is averaged in dB
//!   (a geometric mean, not Welch's mean power), each bin clipped at its running mean plus
//!   [`CLIP_DB`], so a harmonic there for less than half the frames stands within a dB or two of the
//!   floor while a resonance on broadband sound keeps its full height. So a resonance is measured
//!   where broadband sound — noise, breath, reverb, a drum — reaches it; one only notes ever reach
//!   is not seen.
//! - **Candidates** are its peaks by F3's prominence rule (decision 24), the leakage bins beside a
//!   peak skipped (van Waterschoot & Moonen 2010, PNPR), each found again by the cookbook bell
//!   fitted to it ([`fit_bell`]): its centre, its height `P` and its `Q`, so that at 100 % the cut
//!   is its exact inverse (research §9.2).
//! - **Persistence decides** (research §8.2–§8.3, after the feedback suppressors' IPMP): in at least
//!   half of the movement frames that could show it, a candidate stands out over its surroundings
//!   ([`stands_out`]), and under at least two excitations, what is playing elsewhere in the rung.
//!   The counts are kept per 1/48 octave as the frames come, so a candidate found later is judged
//!   on everything heard.
//! - **Proposals** ([`propose`]): what the EQ as heard has not already taken off, at `Amount × P`,
//!   the depths solved together by projected Gauss–Seidel on the bells' actual responses.

use mxm_fft::{Complex, Planner, RealFft};
use mxm_fx_eq_n_dsp::{Cascade, Design, Placement, Shape, Slope, design};

use super::grab::{KEY_OCTAVES, LEVEL_WINDOW_DB, PROMINENCE_DB, SPACING_OCTAVES};
use super::spectrum::{BUTTERWORTH_8, LONG_CUTOFF, LowPass};
use crate::analyser::{BLOCK_FRAMES, Block};

/// The width frames' length, in points of their rung.
pub const WIDTH_POINTS: usize = 8_192;
/// The movement frames' length.
pub const MOVEMENT_POINTS: usize = 2_048;
/// Rung 0 is the input decimated by the smallest power of two that brings it to at most this.
pub const FIRST_RATE_MAX: f32 = 64_000.0;
/// Each rung after the first is this much slower than the one before.
pub const RUNG_STEP: usize = 4;
/// The rungs' own frequencies, half-open: rung 2 below the first edge, rung 1 up to the second,
/// rung 0 from it. Fixed at every rate.
pub const RUNG_EDGES: [f32; 2] = [400.0, 1_600.0];
/// The lowest frequency the finder reads.
pub const LOWEST_HZ: f32 = 20.0;
/// A rung reads up to this share of its rate, its anti-alias filter's corner.
const TOP_SHARE: f32 = LONG_CUTOFF as f32;
/// A frame counts only if its input, both channels' power averaged, is at least this.
pub const HEARD_DBFS: f32 = -60.0;
/// Proposals appear after this much heard input.
pub const LISTEN_SECONDS: f64 = 4.0;
/// How often the candidates are found again, in the editor's seconds.
pub const UPDATE_SECONDS: f64 = 0.5;
/// Half a semitone, in octaves: how close two peaks are to be one.
pub const HALF_SEMITONE: f32 = 1.0 / 24.0;
/// A semitone: how far apart two excitations are to be two.
const SEMITONE: f32 = 1.0 / 12.0;
/// A frame could show a candidate when the third of an octave around it is within this of the
/// frame's loudest third.
pub const ELIGIBLE_DB: f32 = 40.0;
/// The most of the frames where a peak stands out that one excitation may account for, for the
/// peak to have been heard under two.
pub const ONE_EXCITATION: f32 = 2.0 / 3.0;
/// A candidate needs this many heard width frames in its rung …
pub const MIN_WIDTH_FRAMES: u32 = 2;
/// … and this many movement frames that could show it.
pub const MIN_MOVEMENT_FRAMES: u32 = 8;
/// A peak in both channels, prominences further apart than this, is a Left and a Right band.
pub const PAIR_DB: f32 = 3.0;
/// Mid only when the side's level at the peak is within this of the mid's: otherwise the side has
/// nothing there to leave untouched, and the band is Stereo.
pub const SIDE_WITHIN_DB: f32 = 20.0;
/// The most candidates proposed.
pub const MAX_CANDIDATES: usize = 8;
/// The deepest a proposal cuts (Ramos & López's cap, research §8.4).
pub const MAX_CUT_DB: f32 = 12.0;
/// A proposal's Q, from the width at half its prominence, is held inside this (Corbach's Q 20 at
/// the top, research §8.4).
pub const Q_RANGE: (f32, f32) = (0.5, 20.0);
/// Amount's default, in percent: Grachten's professionals' median (research §8.5).
pub const DEFAULT_AMOUNT: u8 = 20;
/// A candidate is treated when what is left to take is under this …
pub const TREATED_DB: f32 = 0.5;
/// … and a proposal already shown goes only under this, so updates do not flicker.
pub const RELEASE_DB: f32 = 0.25;
/// The depths' sweeps of projected Gauss–Seidel.
pub const SWEEPS: usize = 32;
/// A solved depth shallower than this is no cut, and is not proposed.
const NO_CUT_DB: f32 = 0.05;

/// The quietest level a width frame's bin enters the long-term mean at, so a bin silent in one
/// frame does not drag the mean without bound.
const LEVEL_FLOOR_DB: f32 = -160.0;
/// How far above its mean so far a bin enters the long-term spectrum at most.
pub const CLIP_DB: f32 = 3.0;

/// The 1/48-octave cells persistence is counted in, from [`LOWEST_HZ`].
const CELLS_PER_OCTAVE: f32 = 48.0;
const CELLS: usize = 480;

/// One of the five spectra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Left,
    Right,
    Mid,
    Side,
    /// The channels' power average: what a Stereo band is judged on, and a mono input's only one.
    Average,
}

impl Domain {
    pub const ALL: [Domain; 5] = [
        Domain::Left,
        Domain::Right,
        Domain::Mid,
        Domain::Side,
        Domain::Average,
    ];

    fn index(self) -> usize {
        self as usize
    }

    /// The placement of a band that works on this spectrum.
    pub fn placement(self) -> Placement {
        match self {
            Domain::Left => Placement::Left,
            Domain::Right => Placement::Right,
            Domain::Mid => Placement::Mid,
            Domain::Side => Placement::Side,
            Domain::Average => Placement::Stereo,
        }
    }

    /// The spectra a layout has: a mono input only the one.
    fn of(channels: usize) -> &'static [Domain] {
        if channels == 1 {
            &[Domain::Average]
        } else {
            &Domain::ALL
        }
    }
}

/// One frame length on one rung: a ring of left and right, its plan and its buffers.
struct Frames {
    len: usize,
    hop: usize,
    history: [Vec<f32>; 2],
    write: usize,
    contiguous: usize,
    since: usize,
    fft: RealFft<f32>,
    window: Vec<f32>,
    /// `4 / (N · Σw²)`: a full-scale sine's lobe sums to 1, as the display's band power does.
    scale: f32,
    input: Vec<f32>,
    spectra: [Vec<Complex<f32>>; 2],
    scratch: Vec<Complex<f32>>,
}

impl Frames {
    fn new(planner: &mut Planner<f32>, len: usize) -> Frames {
        let fft = planner.real(len);
        let window: Vec<f32> = (0..len)
            .map(|n| 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / len as f32).cos())
            .collect();
        let energy: f32 = window.iter().map(|w| w * w).sum();
        Frames {
            len,
            hop: len / 2,
            history: [vec![0.0; len], vec![0.0; len]],
            write: 0,
            contiguous: 0,
            since: 0,
            scratch: fft.make_scratch(),
            spectra: [
                vec![Complex::new(0.0, 0.0); fft.spectrum_len()],
                vec![Complex::new(0.0, 0.0); fft.spectrum_len()],
            ],
            input: vec![0.0; len],
            fft,
            window,
            scale: 4.0 / (len as f32 * energy),
        }
    }

    fn bins(&self) -> usize {
        self.fft.spectrum_len()
    }

    fn restart(&mut self) {
        self.contiguous = 0;
        self.since = 0;
    }

    /// One sample of each channel; whether a frame is due.
    fn push(&mut self, left: f32, right: f32) -> bool {
        self.history[0][self.write] = left;
        self.history[1][self.write] = right;
        self.write = (self.write + 1) % self.len;
        self.contiguous = (self.contiguous + 1).min(self.len);
        self.since += 1;
        if self.contiguous >= self.len && self.since >= self.hop {
            self.since = 0;
            true
        } else {
            false
        }
    }

    /// Windows and transforms the frame, each channel the layout has, and returns its input's mean
    /// power, the channels averaged.
    fn transform(&mut self, channels: usize) -> f32 {
        let mut power = 0.0f64;
        for channel in 0..channels {
            let history = &self.history[channel];
            for n in 0..self.len {
                let x = history[(self.write + n) % self.len];
                power += f64::from(x * x);
                self.input[n] = x * self.window[n];
            }
            if self
                .fft
                .forward(
                    &mut self.input,
                    &mut self.spectra[channel],
                    &mut self.scratch,
                )
                .is_err()
            {
                return 0.0;
            }
        }
        (power / (self.len * channels) as f64) as f32
    }

    /// Each domain's power per bin, from the channels' spectra: mid and side are linear in them.
    fn power(&self, domain: Domain, channels: usize, out: &mut [f32]) {
        let [left, right] = &self.spectra;
        for (bin, out) in out.iter_mut().enumerate() {
            let (l, r) = if channels == 1 {
                (left[bin], left[bin])
            } else {
                (left[bin], right[bin])
            };
            *out = self.scale
                * match domain {
                    Domain::Left => l.norm_sqr(),
                    Domain::Right => r.norm_sqr(),
                    Domain::Mid => ((l + r) * 0.5).norm_sqr(),
                    Domain::Side => ((l - r) * 0.5).norm_sqr(),
                    Domain::Average => 0.5 * (l.norm_sqr() + r.norm_sqr()),
                };
        }
    }
}

/// A decimator: the anti-alias low pass on each channel, then every `factor`-th sample.
struct Decimator {
    factor: usize,
    phase: usize,
    filters: [[LowPass; 4]; 2],
}

impl Decimator {
    fn new(factor: usize, rate_in: f32) -> Decimator {
        let cutoff = LONG_CUTOFF * f64::from(rate_in) / factor as f64;
        let section = |q| LowPass::new(cutoff, f64::from(rate_in), q);
        Decimator {
            factor,
            phase: 0,
            filters: std::array::from_fn(|_| BUTTERWORTH_8.map(section)),
        }
    }

    fn restart(&mut self) {
        self.phase = 0;
        for channel in &mut self.filters {
            for section in channel {
                section.clear();
            }
        }
    }

    fn push(&mut self, frame: [f32; 2]) -> Option<[f32; 2]> {
        let mut out = [0.0f32; 2];
        for (channel, value) in frame.into_iter().enumerate() {
            let mut x = f64::from(value);
            for section in &mut self.filters[channel] {
                x = section.tick(x);
            }
            out[channel] = x as f32;
        }
        self.phase += 1;
        (self.phase == self.factor).then(|| {
            self.phase = 0;
            out
        })
    }
}

/// One rung as it listens: its rate, the highest it reads, its decimator and its frames.
struct Rung {
    rate: f32,
    top: f32,
    /// What feeds it from the rung before (or from the input, for rung 0 above 64 kHz).
    decimator: Option<Decimator>,
    width: Frames,
    /// Its movement frames: one length, or on the lowest rung two, each read in its own octaves.
    movements: Vec<Movement>,
}

/// What one rung has heard: its own frequencies, half-open, the highest it reads, its width bins'
/// spacing, and each domain's level per width bin in dB, summed over the heard width frames.
#[derive(Clone)]
struct RungEvidence {
    low: f32,
    high: f32,
    top: f32,
    bin_hz: f32,
    sums: [Vec<f64>; 5],
    heard_frames: u32,
}

/// Everything the finder has heard: each rung's long-term sums, the persistence counts, the layout
/// and how long it has listened. Plain data, so a search runs on a copy away from the editor's
/// frame ([`Worker`]).
#[derive(Clone)]
pub struct Evidence {
    rungs: Vec<RungEvidence>,
    cells: [Vec<Cell>; 5],
    channels: usize,
    heard_seconds: f64,
}

/// One length of movement frames on a rung, and the frequencies it is read in, half-open.
struct Movement {
    frames: Frames,
    low: f32,
    high: f32,
}

/// How far `cell` stands out among `levels` in one movement frame (plan §7.5): F3's prominence,
/// its highest level within half a semitone over the median of its surroundings — the cells from
/// a twelfth to a third of an octave either side — rather than over their lowest point, since one
/// short frame's noise has deep dips everywhere and would put a "peak" within half a semitone of
/// anything. `None` where its surroundings are not all read.
fn stands_out(
    levels: &[f32],
    read: std::ops::Range<usize>,
    cell: usize,
    flank: &mut Vec<f32>,
) -> Option<f32> {
    let per = CELLS_PER_OCTAVE as usize;
    let (centre, near, far) = (per / 24, per / 12, per / 3);
    if cell < read.start + far || cell + far >= read.end {
        return None;
    }
    flank.clear();
    flank.extend(
        (cell - far..=cell + far)
            .filter(|&other| other.abs_diff(cell) > near)
            .map(|other| levels[other]),
    );
    let middle = flank.len() / 2;
    let (_, median, _) = flank.select_nth_unstable_by(middle, f32::total_cmp);
    let median = *median;
    let top = (cell - centre..=cell + centre)
        .map(|other| levels[other])
        .fold(f32::NEG_INFINITY, f32::max);
    Some(top - median)
}

/// One cell's persistence counts on one spectrum.
#[derive(Clone, Debug, Default)]
struct Cell {
    /// Movement frames that could show a peak here: its third within [`ELIGIBLE_DB`] of the
    /// frame's loudest third.
    eligible: u32,
    /// Those of them where it stands out ([`stands_out`]).
    present: u32,
    /// Those, by their excitation — the semitone of the loudest of the rung's other frequencies,
    /// outside this cell's third — and how many each.
    excitations: Vec<(u16, u32)>,
}

impl Cell {
    /// Whether it stood out under at least two excitations: the commonest accounts for at most
    /// [`ONE_EXCITATION`] of the frames, so one odd frame does not make a second.
    fn varied(&self) -> bool {
        let commonest = self
            .excitations
            .iter()
            .map(|&(_, count)| count)
            .max()
            .unwrap_or(0);
        self.present > 0 && commonest as f32 <= ONE_EXCITATION * self.present as f32
    }

    fn excited_by(&mut self, semitone: u16) {
        match self
            .excitations
            .iter_mut()
            .find(|(other, _)| *other == semitone)
        {
            Some((_, count)) => *count += 1,
            None => self.excitations.push((semitone, 1)),
        }
    }
}

/// A peak of one frame's or one long-term spectrum, on its bins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BinPeak {
    pub bin: usize,
    pub freq: f32,
    pub db: f32,
    pub prominence: f32,
}

/// A candidate's measurement on one spectrum's long-term spectrum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub domain: Domain,
    pub rung: usize,
    /// The parabola's centre and level.
    pub freq: f32,
    pub db: f32,
    pub prominence: f32,
    /// `f0 / Δf` at half the prominence, not yet held in [`Q_RANGE`].
    pub q: f32,
}

impl Measured {
    /// How far its prominence stands above what is just audible on music: 1.5·Q^0.3 dB (Toole &
    /// Olive 1988, research §8.8).
    pub fn audibility(&self) -> f32 {
        self.prominence - 1.5 * self.q.max(0.1).powf(0.3)
    }
}

/// A resonance the finder would cut: one band, or a Left and a Right band at one centre.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub freq: f32,
    pub bands: Vec<Measured>,
    pub audibility: f32,
}

/// The finder: everything heard since Find was turned on.
pub struct Finder {
    sample_rate: f32,
    /// What brings the input to rung 0's rate, when it is above [`FIRST_RATE_MAX`].
    first: Option<Decimator>,
    rungs: Vec<Rung>,
    expected_seq: Option<u64>,
    generation: Option<u32>,
    /// Counts every heard frame and every restart: what the finder knows changed.
    revision: u64,
    evidence: Evidence,
    /// Working buffers for one frame, so a frame allocates nothing.
    power: Vec<f32>,
    prefix: Vec<f64>,
    levels: Vec<f32>,
    thirds: Vec<f32>,
    flank: Vec<f32>,
}

impl Finder {
    pub fn new(sample_rate: f32) -> Finder {
        let sample_rate = if sample_rate.is_finite() && sample_rate >= 8_000.0 {
            sample_rate
        } else {
            48_000.0
        };
        let mut factor = 1usize;
        while sample_rate / factor as f32 > FIRST_RATE_MAX {
            factor *= 2;
        }
        let first = (factor > 1).then(|| Decimator::new(factor, sample_rate));
        let mut planner = Planner::new();
        let mut rate = sample_rate / factor as f32;
        let mut rungs = Vec::new();
        let mut heard = Vec::new();
        for index in 0..3 {
            let decimator = (index > 0).then(|| Decimator::new(RUNG_STEP, rate * RUNG_STEP as f32));
            let width = Frames::new(&mut planner, WIDTH_POINTS);
            let bins = width.bins();
            let (low, high) = match index {
                0 => (RUNG_EDGES[1], f32::INFINITY),
                1 => (RUNG_EDGES[0], RUNG_EDGES[1]),
                _ => (LOWEST_HZ, RUNG_EDGES[0]),
            };
            // About the shortest frame that tells notes a semitone apart in each octave (plan
            // §7.5): on the lowest rung its top octave takes half the length.
            let mut movements = vec![Movement {
                frames: Frames::new(&mut planner, MOVEMENT_POINTS),
                low,
                high,
            }];
            if index == 2 {
                let split = RUNG_EDGES[0] / 2.0;
                movements[0].high = split;
                movements.push(Movement {
                    frames: Frames::new(&mut planner, MOVEMENT_POINTS / 2),
                    low: split,
                    high,
                });
            }
            heard.push(RungEvidence {
                low,
                high,
                top: TOP_SHARE * rate,
                bin_hz: rate / width.len as f32,
                sums: std::array::from_fn(|_| vec![0.0; bins]),
                heard_frames: 0,
            });
            rungs.push(Rung {
                rate,
                top: TOP_SHARE * rate,
                decimator,
                width,
                movements,
            });
            rate /= RUNG_STEP as f32;
        }
        let most_bins = WIDTH_POINTS / 2 + 1;
        Finder {
            sample_rate,
            first,
            rungs,
            expected_seq: None,
            generation: None,
            revision: 0,
            evidence: Evidence {
                rungs: heard,
                cells: std::array::from_fn(|_| vec![Cell::default(); CELLS]),
                channels: 2,
                heard_seconds: 0.0,
            },
            power: vec![0.0; most_bins],
            prefix: vec![0.0; most_bins + 1],
            levels: vec![0.0; CELLS],
            thirds: vec![0.0; CELLS],
            flank: Vec::with_capacity(CELLS_PER_OCTAVE as usize),
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The seconds of heard input so far.
    pub fn heard_seconds(&self) -> f64 {
        self.evidence.heard_seconds
    }

    /// What it has heard, for a search ([`Evidence::candidates`], [`propose`]).
    pub fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    /// [`Evidence::candidates`] of what it has heard.
    pub fn candidates(&self) -> Vec<Candidate> {
        self.evidence.candidates()
    }

    /// [`Evidence::measured`] of what it has heard.
    pub fn measured(&self) -> Vec<Measured> {
        self.evidence.measured()
    }

    /// [`Evidence::persists`] in what it has heard.
    pub fn persists(&self, domain: Domain, freq: f32) -> bool {
        self.evidence.persists(domain, freq)
    }

    /// Changes whenever what the finder has heard does: nothing new, no new candidates.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether enough has been heard for proposals (plan §7.5: four seconds).
    pub fn listened(&self) -> bool {
        self.evidence.listened()
    }

    /// Forgets everything heard: Find turned on again, a reset, another rate.
    pub fn restart(&mut self) {
        self.restart_windows();
        self.expected_seq = None;
        self.generation = None;
        self.revision = self.revision.wrapping_add(1);
        let evidence = &mut self.evidence;
        evidence.heard_seconds = 0.0;
        for rung in &mut evidence.rungs {
            for sums in &mut rung.sums {
                sums.fill(0.0);
            }
            rung.heard_frames = 0;
        }
        for cells in &mut evidence.cells {
            cells.fill(Cell::default());
        }
    }

    /// A gap in the audio: the frames start again, never spliced across it, and what was heard
    /// before it stays.
    fn restart_windows(&mut self) {
        if let Some(first) = &mut self.first {
            first.restart();
        }
        for rung in &mut self.rungs {
            if let Some(decimator) = &mut rung.decimator {
                decimator.restart();
            }
            rung.width.restart();
            for movement in &mut rung.movements {
                movement.frames.restart();
            }
        }
    }

    /// One block of the analyser's: its pre-EQ audio. A block at another rate is not this
    /// finder's; a new generation (a reset) starts the listen again; a dropped block restarts the
    /// frames.
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
        self.evidence.channels = usize::from(block.channels.clamp(1, 2));
        for index in 0..BLOCK_FRAMES {
            let frame = [block.pre[0][index], block.pre[1][index]]
                .map(|x| if x.is_finite() { x } else { 0.0 });
            let frame = match &mut self.first {
                Some(first) => match first.push(frame) {
                    Some(frame) => frame,
                    None => continue,
                },
                None => frame,
            };
            self.push_rung(0, frame);
        }
    }

    fn push_rung(&mut self, index: usize, frame: [f32; 2]) {
        let rung = &mut self.rungs[index];
        let width_due = rung.width.push(frame[0], frame[1]);
        let mut due = [false; 2];
        for (due, movement) in due.iter_mut().zip(&mut rung.movements) {
            *due = movement.frames.push(frame[0], frame[1]);
        }
        if width_due {
            self.width_frame(index);
        }
        for (which, due) in due.into_iter().enumerate() {
            if due {
                self.movement_frame(index, which);
            }
        }
        if index + 1 < self.rungs.len()
            && let Some(next) = self.rungs[index + 1]
                .decimator
                .as_mut()
                .and_then(|decimator| decimator.push(frame))
        {
            self.push_rung(index + 1, next);
        }
    }

    /// A width frame: if heard, each domain's power joins its long-term sum.
    fn width_frame(&mut self, index: usize) {
        let channels = self.evidence.channels;
        let rung = &mut self.rungs[index];
        let mean = rung.width.transform(channels);
        if !heard(mean) {
            return;
        }
        let heard = &mut self.evidence.rungs[index];
        heard.heard_frames += 1;
        self.revision = self.revision.wrapping_add(1);
        let bins = rung.width.bins();
        for &domain in Domain::of(channels) {
            rung.width.power(domain, channels, &mut self.power[..bins]);
            // The frames before this one, for the clip: from the third frame on, a bin enters at
            // most CLIP_DB above its mean so far.
            let before = f64::from(heard.heard_frames - 1);
            for (sum, power) in heard.sums[domain.index()]
                .iter_mut()
                .zip(&self.power[..bins])
            {
                let level = f64::from(to_db(*power).max(LEVEL_FLOOR_DB));
                let level = if before >= 2.0 {
                    level.min(*sum / before + f64::from(CLIP_DB))
                } else {
                    level
                };
                *sum += level;
            }
        }
    }

    /// A movement frame: if heard, each cell of its own frequencies counts whether it could show a
    /// peak, whether one is there, and under what excitation.
    fn movement_frame(&mut self, index: usize, which: usize) {
        let channels = self.evidence.channels;
        let rung = &mut self.rungs[index];
        let (top, rate) = (rung.top, rung.rate);
        let movement = &mut rung.movements[which];
        let mean = movement.frames.transform(channels);
        if !heard(mean) {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        if index == 0 {
            self.evidence.heard_seconds += movement.frames.hop as f64 / f64::from(rate);
        }
        let frames = &movement.frames;
        let bins = frames.bins();
        let bin_hz = rate / frames.len as f32;
        let own = own_cells(movement.low, movement.high.min(top));
        // The cells read: its own, and a third of an octave either side for their surroundings,
        // inside what the rung reads.
        let read = own_cells(LOWEST_HZ, top);
        let reach = CELLS_PER_OCTAVE as usize / 3;
        let read = own.start.saturating_sub(reach).max(read.start)..(own.end + reach).min(read.end);
        let cell_half = 2f32.powf(1.0 / (2.0 * CELLS_PER_OCTAVE));
        let half_third = 2f32.powf(KEY_OCTAVES / 2.0);
        let sixth = CELLS_PER_OCTAVE as usize / 6;
        for &domain in Domain::of(channels) {
            frames.power(domain, channels, &mut self.power[..bins]);
            self.prefix[0] = 0.0;
            for bin in 0..bins {
                self.prefix[bin + 1] = self.prefix[bin] + f64::from(self.power[bin]);
            }
            let prefix = &self.prefix[..=bins];
            // Each cell's level, the power density over its 1/48 octave (or its bin, where that is
            // narrower), and the power in the third of an octave around it.
            let mut loudest_third = f32::NEG_INFINITY;
            for cell in read.clone() {
                let freq = cell_freq(cell);
                let (low, high) = (freq / cell_half, freq * cell_half);
                let width = (high - low) / bin_hz;
                self.levels[cell] = if width <= 1.0 {
                    let bin = ((freq / bin_hz).round() as usize).min(bins - 1);
                    to_db(self.power[bin])
                } else {
                    to_db((band(prefix, bin_hz, low, high) / f64::from(width)) as f32)
                };
                if own.contains(&cell) {
                    let third =
                        to_db(band(prefix, bin_hz, freq / half_third, freq * half_third) as f32);
                    self.thirds[cell] = third;
                    loudest_third = loudest_third.max(third);
                }
            }
            // What is playing: the loudest of the frame's own frequencies, and the loudest a sixth
            // of an octave or more from it, for a cell near the first.
            let loudest = |away: Option<usize>| {
                own.clone()
                    .filter(|&cell| away.is_none_or(|from| cell.abs_diff(from) > sixth))
                    .max_by(|&a, &b| self.levels[a].total_cmp(&self.levels[b]))
            };
            let Some(first) = loudest(None) else {
                continue;
            };
            let second = loudest(Some(first)).unwrap_or(first);
            let cells = &mut self.evidence.cells[domain.index()];
            for cell in own.clone() {
                if self.thirds[cell] < loudest_third - ELIGIBLE_DB {
                    continue;
                }
                let state = &mut cells[cell];
                state.eligible += 1;
                let Some(excess) = stands_out(&self.levels, read.clone(), cell, &mut self.flank)
                else {
                    continue;
                };
                if excess < PROMINENCE_DB {
                    continue;
                }
                state.present += 1;
                // Its excitation: what is playing outside its own third of an octave, by semitone.
                let playing = if cell.abs_diff(first) > sixth {
                    first
                } else {
                    second
                };
                state.excited_by(semitone(cell_freq(playing)));
            }
        }
    }
}

impl Evidence {
    /// Whether enough has been heard for proposals (plan §7.5: four seconds).
    pub fn listened(&self) -> bool {
        self.heard_seconds >= LISTEN_SECONDS
    }

    /// The long-term spectrum of `domain` on `rung`, in dB per width bin, into `out`: the frames'
    /// clipped mean level. `None` before [`MIN_WIDTH_FRAMES`] heard frames.
    fn long_term(&self, rung: usize, domain: Domain, out: &mut Vec<f32>) -> Option<f32> {
        let rung = &self.rungs[rung];
        if rung.heard_frames < MIN_WIDTH_FRAMES {
            return None;
        }
        let count = f64::from(rung.heard_frames);
        out.clear();
        out.extend(
            rung.sums[domain.index()]
                .iter()
                .map(|sum| (sum / count) as f32),
        );
        Some(rung.bin_hz)
    }

    /// Each spectrum's long-term peaks, measured, in every rung's own frequencies.
    pub fn measured(&self) -> Vec<Measured> {
        self.measured_where(&mut Vec::new(), |_, _| true)
    }

    /// [`measured`](Self::measured), only the peaks `keep(domain, freq)` admits, checked before
    /// they are measured; the long-term spectra read are left in `spectra`, by rung and domain.
    fn measured_where(
        &self,
        spectra: &mut Vec<Option<(f32, Vec<f32>)>>,
        keep: impl Fn(Domain, f32) -> bool,
    ) -> Vec<Measured> {
        let mut found = Vec::new();
        let mut peaks = Vec::new();
        spectra.clear();
        spectra.resize(self.rungs.len() * 5, None);
        for (index, rung) in self.rungs.iter().enumerate() {
            for &domain in Domain::of(self.channels) {
                let mut db = Vec::new();
                let Some(bin_hz) = self.long_term(index, domain, &mut db) else {
                    continue;
                };
                let bins = db.len();
                let search = bin_range(LOWEST_HZ, rung.top, bin_hz, bins);
                let own = bin_range(rung.low, rung.high.min(rung.top), bin_hz, bins);
                if let Some(loudest) = db[own.0..own.1].iter().copied().reduce(f32::max) {
                    bin_peaks(
                        &db,
                        bin_hz,
                        own,
                        search,
                        loudest - LEVEL_WINDOW_DB,
                        |freq| keep(domain, freq),
                        &mut peaks,
                    );
                    for peak in &peaks {
                        if let Some(measured) = measure(&db, bin_hz, peak, search) {
                            found.push(Measured {
                                domain,
                                rung: index,
                                ..measured
                            });
                        }
                    }
                }
                spectra[index * 5 + domain.index()] = Some((bin_hz, db));
            }
        }
        found
    }

    /// The long-term level of `domain` around `freq`, in dB: its power within half a semitone.
    fn level_at(&self, domain: Domain, freq: f32) -> f32 {
        let Some(index) = self.rung_of(freq) else {
            return f32::NEG_INFINITY;
        };
        let rung = &self.rungs[index];
        if rung.heard_frames == 0 {
            return f32::NEG_INFINITY;
        }
        let sums = &rung.sums[domain.index()];
        let bin_hz = rung.bin_hz;
        let ratio = 2f32.powf(HALF_SEMITONE);
        let (low, high) = bin_range(freq / ratio, freq * ratio, bin_hz, sums.len());
        let count = f64::from(rung.heard_frames);
        let power: f64 = sums[low..high.max(low + 1).min(sums.len())]
            .iter()
            .map(|sum| 10f64.powf(sum / count / 10.0))
            .sum();
        to_db(power as f32)
    }

    /// The rung whose own frequencies hold `freq`.
    fn rung_of(&self, freq: f32) -> Option<usize> {
        self.rungs
            .iter()
            .position(|rung| freq >= rung.low && freq < rung.high.min(rung.top))
    }

    /// Whether the peak at `freq` on `domain` persists (plan §7.5): present in at least half of at
    /// least [`MIN_MOVEMENT_FRAMES`] frames that could show it, under two excitations.
    pub fn persists(&self, domain: Domain, freq: f32) -> bool {
        let Some(cell) = cell_of(freq) else {
            return false;
        };
        let state = &self.cells[domain.index()][cell];
        state.eligible >= MIN_MOVEMENT_FRAMES
            && 2 * state.present >= state.eligible
            && state.varied()
    }

    /// The candidates (plan §7.5): the five spectra's peaks merged, each placed where its excess
    /// is, kept if it persists, ranked by audibility, at most [`MAX_CANDIDATES`] a sixth of an
    /// octave apart. Empty until [`listened`](Self::listened).
    pub fn candidates(&self) -> Vec<Candidate> {
        if !self.listened() {
            return Vec::new();
        }
        // Only a peak that persists on its spectrum is that spectrum's: a note's line that
        // happens to stand near a resonance on another channel does not make it that channel's.
        let mut spectra = Vec::new();
        let mut measured =
            self.measured_where(&mut spectra, |domain, freq| self.persists(domain, freq));
        measured.sort_by(|a, b| b.prominence.total_cmp(&a.prominence));
        // Merged within half a semitone: the most prominent stands for the group, and each
        // spectrum keeps its own most prominent there.
        let mut groups: Vec<(f32, [Option<Measured>; 5])> = Vec::new();
        for peak in measured {
            match groups
                .iter_mut()
                .find(|(freq, _)| octaves(peak.freq, *freq).abs() <= HALF_SEMITONE)
            {
                Some((_, by_domain)) => {
                    let slot = &mut by_domain[peak.domain.index()];
                    if slot.is_none() {
                        *slot = Some(peak);
                    }
                }
                None => {
                    let mut by_domain = [None; 5];
                    by_domain[peak.domain.index()] = Some(peak);
                    groups.push((peak.freq, by_domain));
                }
            }
        }
        rank(
            groups
                .into_iter()
                .filter_map(|(freq, by_domain)| self.place(freq, &by_domain, &spectra))
                .collect(),
        )
    }

    /// Where a group's excess is (plan §7.5, the first rule that holds), and whether it persists
    /// there.
    fn place(
        &self,
        freq: f32,
        by: &[Option<Measured>; 5],
        spectra: &[Option<(f32, Vec<f32>)>],
    ) -> Option<Candidate> {
        let at = |domain: Domain| by[domain.index()];
        let mut bands: Vec<Measured> = Vec::new();
        if self.channels == 1 {
            bands.extend(at(Domain::Average));
        } else {
            let (left, right) = (at(Domain::Left), at(Domain::Right));
            let (mid, side) = (at(Domain::Mid), at(Domain::Side));
            // With the peak in both channels, their heights are compared as fitted.
            let fitted = |peak: Option<Measured>| {
                peak.map(|peak| self.fit_in(&peak, spectra).unwrap_or(peak))
            };
            let (left, right) = match (left, right) {
                (Some(_), Some(_)) => (fitted(left), fitted(right)),
                pair => pair,
            };
            match (left, right) {
                (Some(one), None) | (None, Some(one)) => bands.push(one),
                (Some(left), Some(right))
                    if (left.prominence - right.prominence).abs() > PAIR_DB =>
                {
                    bands.push(left);
                    bands.push(right);
                }
                _ => match (mid, side) {
                    (None, Some(side)) => bands.push(side),
                    (Some(mid), None)
                        if self.level_at(Domain::Side, mid.freq)
                            >= self.level_at(Domain::Mid, mid.freq) - SIDE_WITHIN_DB =>
                    {
                        bands.push(mid);
                    }
                    _ => {
                        // Stereo, on the power average; where the average shows no peak of its
                        // own, the most prominent spectrum's measurement stands for it.
                        let stand = at(Domain::Average).or_else(|| {
                            by.iter()
                                .flatten()
                                .copied()
                                .reduce(|a, b| if b.prominence > a.prominence { b } else { a })
                        })?;
                        bands.push(Measured {
                            domain: Domain::Average,
                            ..stand
                        });
                    }
                },
            }
        }
        // The Stereo band persists on the power average too; each band measured again by the
        // bell fitted to it ([`fit_bell`]), where the fit holds.
        bands.retain(|band| self.persists(band.domain, band.freq));
        for band in &mut bands {
            if let Some(fitted) = self.fit_in(band, spectra) {
                *band = fitted;
            }
        }
        let audibility = bands.iter().map(Measured::audibility).reduce(f32::max)?;
        let centre = bands.iter().map(|band| band.freq).next().unwrap_or(freq);
        Some(Candidate {
            freq: centre,
            bands,
            audibility,
        })
    }

    /// `peak` measured by the cookbook bell fitted to its spectrum, from `spectra` as
    /// [`measured_where`](Self::measured_where) left them; `None` where the fit does not hold.
    fn fit_in(&self, peak: &Measured, spectra: &[Option<(f32, Vec<f32>)>]) -> Option<Measured> {
        let (bin_hz, db) = spectra.get(peak.rung * 5 + peak.domain.index())?.as_ref()?;
        let rung = &self.rungs[peak.rung];
        let search = bin_range(LOWEST_HZ, rung.top, *bin_hz, db.len());
        let (freq, gain, q) = fit_bell(db, *bin_hz, search, peak)?;
        (gain > 0.0).then_some(Measured {
            freq,
            prominence: gain,
            q,
            ..*peak
        })
    }

    /// The long-term spectrum `measured` came from, in dB, its bin width, and the bins its key
    /// points may reach: what [`propose`] reads a heard prominence on.
    fn spectrum_of(
        &self,
        measured: &Measured,
        out: &mut Vec<f32>,
    ) -> Option<(f32, (usize, usize))> {
        let bin_hz = self.long_term(measured.rung, measured.domain, out)?;
        let rung = &self.rungs[measured.rung];
        Some((bin_hz, bin_range(LOWEST_HZ, rung.top, bin_hz, out.len())))
    }
}

/// The cookbook bell's magnitude in dB at `ratio = f / f0` (its analogue prototype, Q in the
/// half-gain-in-dB convention): the shape a resonance is measured as and its cut inverts.
#[cfg(test)]
fn bell_db(ratio: f64, gain_db: f64, q: f64) -> f64 {
    let a = 10f64.powf(gain_db / 40.0);
    let d = 1.0 - ratio * ratio;
    let num = d * d + (a * ratio / q).powi(2);
    let den = d * d + (ratio / (a * q)).powi(2);
    10.0 * (num / den).max(1.0e-30).log10()
}

/// The points a fit reads at most: the bins of a third of an octave either side, thinned
/// evenly in log-frequency.
const FIT_POINTS: usize = 64;
/// Huber's threshold for the fit's residuals: one further off counts as if it were this far, so a
/// note's line riding on a bump pulls the fit little.
pub const HUBER_DB: f64 = 1.5;

/// The fit's model at every point: the floor at the guess, its slope per octave, and the bell
/// (`p` is ln f0, gain, ln Q, floor, slope), into `out`. The bell's `A` is worked out once.
fn predict(p: &[f64; 5], points: &[(f64, f64, f64)], out: &mut [f64]) {
    let a = 10f64.powf(p[1] / 40.0);
    let q = p[2].exp();
    let (high, low) = (a / q, 1.0 / (a * q));
    for (value, &(ln_freq, octave, _)) in out.iter_mut().zip(points) {
        let ratio = (ln_freq - p[0]).exp();
        let d = 1.0 - ratio * ratio;
        let num = d * d + (high * ratio).powi(2);
        let den = d * d + (low * ratio).powi(2);
        *value = p[3] + p[4] * octave + 10.0 * (num / den).max(1.0e-30).log10();
    }
}

/// The cookbook bell fitted to `db` around `guess` over a third of an octave either side, on a
/// sloping floor: its centre, gain and Q. Levenberg–Marquardt, its residuals weighted by Huber's
/// rule ([`HUBER_DB`]) and the fit run again twice (iteratively reweighted least squares), so the
/// lines riding on a bump count for little. A fit reads the whole bump, where one point — a
/// parabola's top, a half-height crossing — follows whichever line is highest; and the bell it
/// finds is the shape whose exact inverse the cut is (research §9.2). `None` when it does not
/// settle on a bump within a twelfth of an octave of the guess.
pub fn fit_bell(
    db: &[f32],
    bin_hz: f32,
    search: (usize, usize),
    guess: &Measured,
) -> Option<(f32, f32, f32)> {
    let reach = 2f32.powf(KEY_OCTAVES);
    let (low, high) = bin_range(guess.freq / reach, guess.freq * reach, bin_hz, db.len());
    let (low, high) = (low.max(search.0), high.min(search.1));
    if high <= low + 8 {
        return None;
    }
    let mut bins: Vec<usize> = if high - low <= FIT_POINTS {
        (low..high).collect()
    } else {
        (0..FIT_POINTS)
            .map(|at| {
                let t = at as f64 / (FIT_POINTS - 1) as f64;
                (low as f64 * (high as f64 / low as f64).powf(t)).round() as usize
            })
            .collect()
    };
    bins.dedup();
    let reference = f64::from(guess.freq);
    // Each point's ln f, its octaves from the guess and its level.
    let points: Vec<(f64, f64, f64)> = bins
        .iter()
        .map(|&bin| {
            let freq = f64::from(bin as f32 * bin_hz);
            (freq.ln(), (freq / reference).log2(), f64::from(db[bin]))
        })
        .collect();
    let count = points.len();
    let mut weights = vec![1.0f64; count];
    let mut model = vec![0.0f64; count];
    let mut moved = vec![0.0f64; count];
    let mut columns = vec![[0.0f64; 5]; count];
    let cost = |p: &[f64; 5], weights: &[f64], scratch: &mut [f64]| {
        predict(p, &points, scratch);
        points
            .iter()
            .zip(scratch.iter())
            .zip(weights)
            .map(|((point, value), weight)| weight * (point.2 - value).powi(2))
            .sum::<f64>()
    };
    let mut p = [
        reference.ln(),
        f64::from(guess.prominence),
        f64::from(guess.q.clamp(0.3, 100.0)).ln(),
        f64::from(guess.db - guess.prominence),
        0.0,
    ];
    let steps = [1.0e-4, 1.0e-3, 1.0e-3, 1.0e-3, 1.0e-3];
    for round in 0..3 {
        if round > 0 {
            predict(&p, &points, &mut model);
            for ((weight, point), value) in weights.iter_mut().zip(&points).zip(&model) {
                let residual = (point.2 - value).abs();
                *weight = if residual <= HUBER_DB {
                    1.0
                } else {
                    HUBER_DB / residual
                };
            }
        }
        let mut current = cost(&p, &weights, &mut model);
        let mut lambda = 1.0e-3;
        for _ in 0..12 {
            // The Jacobian by forward differences, then (JᵀJ + λ·diag) δ = Jᵀr.
            predict(&p, &points, &mut model);
            for k in 0..5 {
                let mut shifted = p;
                shifted[k] += steps[k];
                predict(&shifted, &points, &mut moved);
                for (column, (after, before)) in columns.iter_mut().zip(moved.iter().zip(&model)) {
                    column[k] = (after - before) / steps[k];
                }
            }
            let mut jtj = [[0.0f64; 5]; 5];
            let mut jtr = [0.0f64; 5];
            for ((column, point), (value, weight)) in
                columns.iter().zip(&points).zip(model.iter().zip(&weights))
            {
                let residual = (point.2 - value) * weight;
                for a in 0..5 {
                    jtr[a] += column[a] * residual;
                    for b in 0..5 {
                        jtj[a][b] += column[a] * column[b] * weight;
                    }
                }
            }
            let mut settled = false;
            for _ in 0..6 {
                let mut system = jtj;
                for (k, row) in system.iter_mut().enumerate() {
                    row[k] += lambda * jtj[k][k].max(1.0e-9);
                }
                let Some(delta) = solve5(system, jtr) else {
                    lambda *= 10.0;
                    continue;
                };
                let mut next = p;
                for k in 0..5 {
                    next[k] += delta[k];
                }
                next[2] = next[2].clamp(0.2f64.ln(), 200f64.ln());
                let tried = cost(&next, &weights, &mut moved);
                if tried.is_finite() && tried <= current {
                    settled = delta.iter().map(|d| d.abs()).fold(0.0, f64::max) < 1.0e-5;
                    p = next;
                    current = tried;
                    lambda = (lambda / 3.0).max(1.0e-9);
                    break;
                }
                lambda *= 10.0;
            }
            if settled {
                break;
            }
        }
    }
    let freq = p[0].exp() as f32;
    let (gain, q) = (p[1] as f32, p[2].exp() as f32);
    let near = octaves(freq, guess.freq).abs() <= 1.0 / 12.0;
    (near && gain.is_finite() && q.is_finite()).then_some((freq, gain, q))
}

/// `system · x = rhs` for five unknowns, by Gaussian elimination with partial pivoting.
fn solve5(mut system: [[f64; 5]; 5], mut rhs: [f64; 5]) -> Option<[f64; 5]> {
    for column in 0..5 {
        let pivot = (column..5)
            .max_by(|&a, &b| system[a][column].abs().total_cmp(&system[b][column].abs()))?;
        if system[pivot][column].abs() < 1.0e-12 {
            return None;
        }
        system.swap(column, pivot);
        rhs.swap(column, pivot);
        let pivot_row = system[column];
        for row in column + 1..5 {
            let factor = system[row][column] / pivot_row[column];
            for (value, above) in system[row].iter_mut().zip(&pivot_row).skip(column) {
                *value -= factor * above;
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    let mut x = [0.0f64; 5];
    for row in (0..5).rev() {
        let known: f64 = (row + 1..5).map(|k| system[row][k] * x[k]).sum();
        x[row] = (rhs[row] - known) / system[row][row];
    }
    x.iter().all(|value| value.is_finite()).then_some(x)
}

/// Candidates ranked by audibility (plan §7.5): at most [`MAX_CANDIDATES`], a sixth of an
/// octave apart, the more audible kept.
pub fn rank(mut candidates: Vec<Candidate>) -> Vec<Candidate> {
    candidates.sort_by(|a, b| b.audibility.total_cmp(&a.audibility));
    let mut kept: Vec<Candidate> = Vec::new();
    for candidate in candidates {
        let near = kept
            .iter()
            .any(|other| octaves(candidate.freq, other.freq).abs() < SPACING_OCTAVES);
        if !near {
            kept.push(candidate);
        }
        if kept.len() == MAX_CANDIDATES {
            break;
        }
    }
    kept
}

fn heard(mean_power: f32) -> bool {
    mean_power >= 10f32.powf(HEARD_DBFS / 10.0)
}

fn to_db(power: f32) -> f32 {
    10.0 * power.max(1.0e-20).log10()
}

/// `a` against `b`, in octaves.
fn octaves(a: f32, b: f32) -> f32 {
    (a / b).log2()
}

/// The semitone `freq` falls in, counted from [`LOWEST_HZ`]: two excitations in different ones
/// stand about a semitone or more apart.
fn semitone(freq: f32) -> u16 {
    (octaves(freq, LOWEST_HZ) / SEMITONE)
        .round()
        .clamp(0.0, f32::from(u16::MAX)) as u16
}

fn cell_freq(cell: usize) -> f32 {
    LOWEST_HZ * 2f32.powf(cell as f32 / CELLS_PER_OCTAVE)
}

fn cell_of(freq: f32) -> Option<usize> {
    let cell = (CELLS_PER_OCTAVE * (freq / LOWEST_HZ).log2()).round();
    (cell >= 0.0 && (cell as usize) < CELLS).then_some(cell as usize)
}

/// The cells whose centres lie in `[low, high)`.
fn own_cells(low: f32, high: f32) -> std::ops::Range<usize> {
    let first = (CELLS_PER_OCTAVE * (low / LOWEST_HZ).log2())
        .ceil()
        .max(0.0) as usize;
    let end = (CELLS_PER_OCTAVE * (high / LOWEST_HZ).log2())
        .ceil()
        .max(0.0) as usize;
    first.min(CELLS)..end.min(CELLS)
}

/// The bins from `low` up to (not including) `high` hertz, never bin 0 and never past `bins`.
fn bin_range(low: f32, high: f32, bin_hz: f32, bins: usize) -> (usize, usize) {
    let first = ((low / bin_hz).ceil() as usize).max(1);
    let end = if high.is_finite() {
        (high / bin_hz).ceil() as usize
    } else {
        bins
    };
    (first.min(bins), end.min(bins).max(first.min(bins)))
}

/// The power between `low` and `high` hertz from prefix sums over bins, fractions at the edges.
fn band(prefix: &[f64], bin_hz: f32, low: f32, high: f32) -> f64 {
    let last = (prefix.len() - 1) as f64;
    let edge = |hz: f32| (f64::from(hz / bin_hz) + 0.5).clamp(0.0, last);
    let cumulative = |at: f64| {
        let whole = (at.floor() as usize).min(prefix.len() - 1);
        let next = (whole + 1).min(prefix.len() - 1);
        prefix[whole] + (at - whole as f64) * (prefix[next] - prefix[whole])
    };
    (cumulative(edge(high)) - cumulative(edge(low))).max(0.0)
}

#[derive(Clone, Copy)]
enum Side {
    Below,
    Above,
}

/// F3's key point on bins (decision 24): the lowest level between the peak at `bin` and the next
/// higher level on `side`, within a third of an octave, the leakage bin beside the peak skipped.
/// Where the search's edge comes first, the lowest seen counts once it is already
/// [`PROMINENCE_DB`] down; short of that, `None`.
fn key_point(db: &[f32], bin: usize, side: Side, search: (usize, usize)) -> Option<f32> {
    let level = db[bin];
    let reach = 2f32.powf(KEY_OCTAVES);
    let beside = match side {
        Side::Below => bin.checked_sub(1).map(|b| db[b]),
        Side::Above => db.get(bin + 1).copied(),
    };
    let mut lowest = f32::INFINITY;
    let mut at = match side {
        Side::Below => bin.checked_sub(1),
        Side::Above => Some(bin + 1),
    };
    loop {
        let next = match side {
            Side::Below => at
                .and_then(|at| at.checked_sub(1))
                .filter(|&next| next >= search.0),
            Side::Above => at.map(|at| at + 1).filter(|&next| next < search.1),
        };
        let Some(next) = next else {
            let lowest = lowest.min(beside.unwrap_or(level));
            return (level - lowest >= PROMINENCE_DB).then_some(lowest);
        };
        let ratio = match side {
            Side::Below => bin as f32 / next as f32,
            Side::Above => next as f32 / bin as f32,
        };
        if ratio > reach * (1.0 + 1.0e-4) || db[next] > level {
            // Nothing past the leakage bin before a higher level: the leakage bin is all there is.
            return Some(if lowest.is_finite() {
                lowest
            } else {
                beside.unwrap_or(level)
            });
        }
        lowest = lowest.min(db[next]);
        at = Some(next);
    }
}

/// The peaks of `db` (per bin, `bin_hz` apart) whose maxima lie in `own`, their key points sought
/// in `search`, by F3's rule (decision 24): a local maximum at or above `floor` standing
/// [`PROMINENCE_DB`] over the higher of its key points, and one `keep` admits by its frequency,
/// asked before the walks. Into `out`, ascending.
pub fn bin_peaks(
    db: &[f32],
    bin_hz: f32,
    own: (usize, usize),
    search: (usize, usize),
    floor: f32,
    keep: impl Fn(f32) -> bool,
    out: &mut Vec<BinPeak>,
) {
    out.clear();
    let start = own.0.max(search.0 + 1).max(1);
    let end = own
        .1
        .min(search.1.saturating_sub(1))
        .min(db.len().saturating_sub(1));
    for bin in start..end {
        let level = db[bin];
        if level < floor || db[bin - 1] >= level || db[bin + 1] > level {
            continue;
        }
        if !keep(bin as f32 * bin_hz) {
            continue;
        }
        let (Some(below), Some(above)) = (
            key_point(db, bin, Side::Below, search),
            key_point(db, bin, Side::Above, search),
        ) else {
            continue;
        };
        let prominence = level - below.max(above);
        if prominence >= PROMINENCE_DB {
            out.push(BinPeak {
                bin,
                freq: bin as f32 * bin_hz,
                db: level,
                prominence,
            });
        }
    }
}

/// A long-term peak measured (plan §7.5): its centre and level by a three-point parabola on its dB
/// (Smith, *SASP*), its prominence over the same key point, and `f0 / Δf` at half the prominence,
/// the crossings interpolated in log-frequency.
fn measure(db: &[f32], bin_hz: f32, peak: &BinPeak, search: (usize, usize)) -> Option<Measured> {
    let bin = peak.bin;
    let (a, b, c) = (db[bin - 1], db[bin], db[bin + 1]);
    let curve = a - 2.0 * b + c;
    let offset = if curve < 0.0 {
        (0.5 * (a - c) / curve).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let freq = (bin as f32 + offset) * bin_hz;
    let level = b - 0.25 * (a - c) * offset;
    let prominence = peak.prominence + (level - b);
    let target = level - prominence / 2.0;
    let crossing = |side: Side| -> Option<f32> {
        let mut at = bin;
        loop {
            let next = match side {
                Side::Below => at.checked_sub(1).filter(|&next| next >= search.0)?,
                Side::Above => Some(at + 1).filter(|&next| next < search.1)?,
            };
            if db[next] <= target {
                // Between `at` (above the target) and `next` (at or below it), in log-frequency.
                let t = (db[at] - target) / (db[at] - db[next]).max(f32::MIN_POSITIVE);
                let (x, y) = ((at as f32 * bin_hz).log2(), (next as f32 * bin_hz).log2());
                return Some(2f32.powf(x + (y - x) * t.clamp(0.0, 1.0)));
            }
            at = next;
        }
    };
    let low = crossing(Side::Below)?;
    let high = crossing(Side::Above)?;
    // The centre midway between them in log-frequency, where the cookbook bell's symmetry puts
    // it: on a broad top the parabola follows whatever of the noise is highest.
    let centre = (low * high).sqrt();
    let freq = if centre.is_finite() { centre } else { freq };
    let q = freq / (high - low).max(f32::MIN_POSITIVE);
    Some(Measured {
        domain: Domain::Average,
        rung: 0,
        freq,
        db: level,
        prominence,
        q,
    })
}

// ---- proposals -------------------------------------------------------------------------------

/// The EQ as heard (plan §7.5): every band in the sound — drawn gain under the gain scale, bypass
/// and placement as the audio has them — at the rate it runs.
pub struct HeardEq {
    pub bands: Vec<(Cascade, Placement)>,
    pub sample_rate: f64,
}

/// `(a + jb)(c + jd)`.
fn times((a, b): (f64, f64), (c, d): (f64, f64)) -> (f64, f64) {
    (a * c - b * d, a * d + b * c)
}

/// Each stage's diagonal at `freq` — the channel stage's left and right, then the mid/side stage's
/// mid and side (plan §7.3) — for `bands`.
pub fn stages<'a>(
    bands: impl IntoIterator<Item = (&'a Cascade, Placement)>,
    freq: f32,
    sample_rate: f64,
) -> [(f64, f64); 4] {
    let mut diagonal = [(1.0, 0.0); 4];
    for (cascade, placement) in bands {
        let h = cascade.response(f64::from(freq), sample_rate);
        let apply: &[usize] = match placement {
            Placement::Stereo => &[0, 1],
            Placement::Left => &[0],
            Placement::Right => &[1],
            Placement::Mid => &[2],
            Placement::Side => &[3],
        };
        for &index in apply {
            diagonal[index] = times(diagonal[index], h);
        }
    }
    diagonal
}

/// What a spectrum hears of the stages, in dB: each channel's response to itself (plan §7.3, as the
/// graph draws them), and for the power average the mean of the left's and the right's power.
pub fn domain_db(domain: Domain, [dl, dr, dm, ds]: [(f64, f64); 4]) -> f32 {
    let half = |(a, b): (f64, f64), (c, d): (f64, f64)| (0.5 * (a + c), 0.5 * (b + d));
    let power = |(re, im): (f64, f64)| re * re + im * im;
    let left = times(half(dm, ds), dl);
    let right = times(half(dm, ds), dr);
    let value = match domain {
        Domain::Left => power(left),
        Domain::Right => power(right),
        Domain::Mid => power(times(dm, half(dl, dr))),
        Domain::Side => power(times(ds, half(dl, dr))),
        Domain::Average => 0.5 * (power(left) + power(right)),
    };
    (10.0 * value.max(1.0e-30).log10()) as f32
}

/// One band of a proposal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProposedBand {
    pub domain: Domain,
    pub freq: f32,
    pub q: f32,
    /// The depth as heard, after the gain scale: what Apply all gives it.
    pub gain_db: f32,
    /// What it aimed at, `−(Amount × P − what the EQ already takes)`.
    pub target_db: f32,
}

impl ProposedBand {
    pub fn cascade(&self, sample_rate: f64) -> Cascade {
        bell(self.freq, self.q, self.gain_db, sample_rate)
    }
}

/// A resonance's proposal: one band, or a Left and a Right band at one centre.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub freq: f32,
    pub bands: Vec<ProposedBand>,
}

fn bell(freq: f32, q: f32, gain_db: f32, sample_rate: f64) -> Cascade {
    design(
        Design {
            shape: Shape::Bell,
            slope: Slope::Db12,
            freq: f64::from(freq),
            gain_db: f64::from(gain_db),
            q: f64::from(q),
        },
        sample_rate,
    )
}

/// The proposals (plan §7.5) for `candidates`, at `amount` (0 to 1), with `free_slots` bands' room,
/// on the EQ as heard; `shown` is what was proposed last, so a proposal already shown goes only
/// under [`RELEASE_DB`].
pub fn propose(
    evidence: &Evidence,
    candidates: &[Candidate],
    amount: f32,
    free_slots: usize,
    heard: &HeardEq,
    shown: &[Proposal],
) -> Vec<Proposal> {
    let amount = amount.clamp(0.0, 1.0);
    let mut proposals: Vec<Proposal> = Vec::new();
    let mut room = free_slots;
    let mut db = Vec::new();
    for candidate in candidates {
        let mut bands = Vec::new();
        for measured in &candidate.bands {
            let removed = removed_by(evidence, measured, heard, &mut db);
            let remainder = amount * measured.prominence - removed;
            let was_shown = shown.iter().any(|proposal| {
                proposal.bands.iter().any(|band| {
                    band.domain == measured.domain
                        && octaves(band.freq, measured.freq).abs() <= HALF_SEMITONE
                })
            });
            let threshold = if was_shown { RELEASE_DB } else { TREATED_DB };
            if remainder < threshold {
                continue;
            }
            bands.push(ProposedBand {
                domain: measured.domain,
                freq: measured.freq,
                q: measured.q.clamp(Q_RANGE.0, Q_RANGE.1),
                gain_db: (-remainder).max(-MAX_CUT_DB),
                target_db: -remainder,
            });
        }
        // In rank order, a candidate whose bands do not fit the room left is passed over.
        if bands.is_empty() || bands.len() > room {
            continue;
        }
        room -= bands.len();
        proposals.push(Proposal {
            freq: candidate.freq,
            bands,
        });
    }
    solve(&mut proposals, heard);
    for proposal in &mut proposals {
        proposal.bands.retain(|band| band.gain_db <= -NO_CUT_DB);
    }
    proposals.retain(|proposal| !proposal.bands.is_empty());
    proposals
}

/// How much of `measured`'s peak the EQ as heard already takes off (plan §7.5): the bell fitted to
/// its long-term spectrum, less the bell fitted, from the same guess, to that spectrum with the EQ
/// added — each band's self-response on that spectrum (§7.3). Two fits from one start, so with no
/// EQ there it is exactly nothing. Where the second fit finds no bump left, the EQ's own depth
/// there stands for it.
fn removed_by(evidence: &Evidence, measured: &Measured, heard: &HeardEq, db: &mut Vec<f32>) -> f32 {
    if heard.bands.is_empty() {
        return 0.0;
    }
    let Some((bin_hz, search)) = evidence.spectrum_of(measured, db) else {
        return 0.0;
    };
    let before = fit_bell(db, bin_hz, search, measured).map_or(measured.prominence, |fit| fit.1);
    // The fit's window with the EQ added in, read on a 1/96-octave grid and interpolated, so a
    // candidate costs a few dozen responses rather than one a bin.
    let reach = 2f32.powf(KEY_OCTAVES + HALF_SEMITONE);
    let window = bin_range(
        measured.freq / reach,
        measured.freq * reach,
        bin_hz,
        db.len(),
    );
    let window = (window.0.max(search.0), window.1.min(search.1));
    let eq_db = |freq: f32| {
        domain_db(
            measured.domain,
            stages(
                heard.bands.iter().map(|(c, p)| (c, *p)),
                freq,
                heard.sample_rate,
            ),
        )
    };
    if window.1 <= window.0 + 2 {
        return -eq_db(measured.freq);
    }
    let (low, high) = (window.0 as f32 * bin_hz, window.1 as f32 * bin_hz);
    let steps = ((high / low).log2() * 96.0).ceil().max(1.0) as usize;
    let grid: Vec<(f32, f32)> = (0..=steps)
        .map(|step| {
            let freq = low * (high / low).powf(step as f32 / steps as f32);
            (freq.log2(), eq_db(freq))
        })
        .collect();
    let eq_at = |freq: f32| -> f32 {
        let x = freq.max(f32::MIN_POSITIVE).log2();
        let at = grid
            .partition_point(|(gx, _)| *gx < x)
            .clamp(1, grid.len() - 1);
        let ((x0, y0), (x1, y1)) = (grid[at - 1], grid[at]);
        let t = ((x - x0) / (x1 - x0).max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
        y0 + (y1 - y0) * t
    };
    for (bin, level) in db.iter_mut().enumerate().take(window.1).skip(window.0) {
        *level += eq_at(bin as f32 * bin_hz);
    }
    match fit_bell(db, bin_hz, search, measured) {
        Some((_, after, _)) => before - after,
        None => -eq_at(measured.freq),
    }
}

/// The depths solved together (plan §7.5): at each band's centre, its spectrum's response of the
/// EQ as heard plus every proposal should stand at the EQ's own plus its target. Projected
/// Gauss–Seidel, [`SWEEPS`] sweeps in rank order on the actual responses, each depth held inside
/// −12…0 dB; it converges wherever each bell is deepest at its own centre, as a cut bell is, and
/// it is bounded and deterministic everywhere.
pub fn solve(proposals: &mut [Proposal], heard: &HeardEq) {
    let rate = heard.sample_rate;
    let mut bands: Vec<&mut ProposedBand> = proposals
        .iter_mut()
        .flat_map(|proposal| proposal.bands.iter_mut())
        .collect();
    if bands.is_empty() {
        return;
    }
    let eq: Vec<([(f64, f64); 4], f32)> = bands
        .iter()
        .map(|band| {
            let stage = stages(heard.bands.iter().map(|(c, p)| (c, *p)), band.freq, rate);
            (stage, domain_db(band.domain, stage))
        })
        .collect();
    let mut cascades: Vec<Cascade> = bands.iter().map(|band| band.cascade(rate)).collect();
    for _ in 0..SWEEPS {
        let mut worst = 0.0f32;
        for index in 0..bands.len() {
            let (stage, own) = eq[index];
            let freq = bands[index].freq;
            let mut diagonal = stage;
            for (other, cascade) in bands.iter().zip(&cascades) {
                let h = cascade.response(f64::from(freq), rate);
                let apply: &[usize] = match other.domain.placement() {
                    Placement::Stereo => &[0, 1],
                    Placement::Left => &[0],
                    Placement::Right => &[1],
                    Placement::Mid => &[2],
                    Placement::Side => &[3],
                };
                for &at in apply {
                    diagonal[at] = times(diagonal[at], h);
                }
            }
            let reached = domain_db(bands[index].domain, diagonal) - own;
            let error = bands[index].target_db - reached;
            worst = worst.max(error.abs());
            let band = &mut bands[index];
            let gain = (band.gain_db + error).clamp(-MAX_CUT_DB, 0.0);
            if gain != band.gain_db {
                band.gain_db = gain;
                cascades[index] = band.cascade(rate);
            }
        }
        if worst < 0.01 {
            break;
        }
    }
}

#[cfg(test)]
mod tests;

// ---- the search, away from the editor's frame ------------------------------------------------

/// One search for Find (plan §7.5), on `evidence`, a copy of what the finder has heard: its
/// candidates found again, or `candidates` kept where the finder has heard nothing new, then the
/// proposals for them on the EQ as heard.
pub struct Search {
    pub evidence: Evidence,
    pub candidates: Option<Vec<Candidate>>,
    pub heard: HeardEq,
    pub amount: f32,
    pub free_slots: usize,
    pub shown: Vec<Proposal>,
}

/// What a search found.
pub struct Found {
    pub candidates: Vec<Candidate>,
    pub proposals: Vec<Proposal>,
}

impl Search {
    /// Runs it, where it is.
    pub fn run(self) -> Found {
        let candidates = self
            .candidates
            .unwrap_or_else(|| self.evidence.candidates());
        let proposals = propose(
            &self.evidence,
            &candidates,
            self.amount,
            self.free_slots,
            &self.heard,
            &self.shown,
        );
        Found {
            candidates,
            proposals,
        }
    }
}

/// A thread the editor hands its searches to, so a search's fits never cost the editor a frame
/// (the analyser's 2 ms budget, plan §4); one search at a time. Dropping it ends the thread.
pub struct Worker {
    jobs: std::sync::mpsc::Sender<Search>,
    found: std::sync::mpsc::Receiver<Found>,
    busy: bool,
}

impl Worker {
    pub fn new() -> Option<Worker> {
        let (jobs, take) = std::sync::mpsc::channel::<Search>();
        let (give, found) = std::sync::mpsc::channel::<Found>();
        std::thread::Builder::new()
            .name("mxm-fx-eq-n finder".to_owned())
            .spawn(move || {
                for search in take {
                    if give.send(search.run()).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Worker {
            jobs,
            found,
            busy: false,
        })
    }

    /// Whether a search is running.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Starts `search`; refused, and handed back, while another runs or if the thread has gone.
    pub fn start(&mut self, search: Search) -> Result<(), Box<Search>> {
        if self.busy {
            return Err(Box::new(search));
        }
        match self.jobs.send(search) {
            Ok(()) => {
                self.busy = true;
                Ok(())
            }
            Err(std::sync::mpsc::SendError(search)) => Err(Box::new(search)),
        }
    }

    /// What the running search found, once it has.
    pub fn take(&mut self) -> Option<Found> {
        let found = self.found.try_recv().ok()?;
        self.busy = false;
        Some(found)
    }
}
