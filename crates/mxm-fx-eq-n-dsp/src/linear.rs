//! F6's linear phase (plan §7.7): the EQ's magnitude as a zero-phase FIR kernel, run by uniformly
//! partitioned FFT convolution, at a latency of the kernel's centre and one partition.
//!
//! - **The kernel** is designed on N points: per bin the EQ's static magnitude, each stage's
//!   (§7.3) taken as zero phase — one kernel for an EQ of Stereo bands, one per channel with Left
//!   or Right bands, and with Mid or Side bands the real 2×2 matrix of the two stages, four kernels
//!   — inverse-transformed, shifted circularly by N/2, shaped by the periodic Hann window
//!   `sin²(π·m/N)` and its tap pairs mirrored, so it is symmetric to the bit about tap N/2 and its
//!   group delay is N/2 at every frequency. With the window, each bin hears its neighbours by
//!   ¼, ½, ¼: the window's transform ([`heard_lines`] reads it so).
//! - **The convolver** runs in `f64`, as the cascade does: valid settings reach +1,440 dB (24
//!   coincident +60 dB bells), past anything `f32` holds, and the output's own bound clips after.
//!   It keeps one frequency-domain delay line of the input's partitions of B; a kernel bank is only
//!   coefficients, so a new bank's first output is its convolution of
//!   everything heard (overlap-save, `research:effects/convolution-reverb.md` §7's family).
//! - **Banks are designed off the audio thread**, on the engine's own design thread, into one of
//!   three preallocated banks handed over through atomics; the audio thread takes the newest
//!   finished one at an update boundary (every four partitions) and crossfades to it over the
//!   next interval, never waiting — except in offline rendering, where it waits for the bank it
//!   asked for, so a bounce is the same every time.
//! - **An empty EQ is a delay line**, bit-exact, of the same latency.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::thread::JoinHandle;

use mxm_fft::{Complex, Planner, RealFft};

use crate::SLOTS;
use crate::design::{Cascade, tan_half};
use crate::engine::Placement;

/// The phase an engine runs (plan §7.7, decision 35).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Phase {
    /// The matched minimum-phase cascade, at no latency.
    #[default]
    ZeroLatency,
    Linear(Resolution),
}

/// Linear phase's resolution (decision 36): the kernel's length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Resolution {
    Low,
    #[default]
    Medium,
    High,
}

impl Resolution {
    pub const ALL: [Resolution; 3] = [Resolution::Low, Resolution::Medium, Resolution::High];

    /// The kernel's length at 44.1–48 kHz: 85, 171 or 341 ms at 48 kHz.
    fn base_points(self) -> usize {
        match self {
            Resolution::Low => 4_096,
            Resolution::Medium => 8_192,
            Resolution::High => 16_384,
        }
    }
}

/// The partition at 44.1–48 kHz, and the smallest at any rate.
const BASE_BLOCK: usize = 256;
const MIN_BLOCK: usize = 32;
/// Partitions in an update interval: a new kernel is asked for, taken and faded in on these
/// boundaries.
pub const BLOCKS_PER_UPDATE: usize = 4;
/// How long the design thread rests between looks for a request.
const DESIGN_REST: std::time::Duration = std::time::Duration::from_millis(1);
/// The most octaves a resolution's lengths are scaled up, at 768 kHz and above, so no rate the
/// plugin is given can ask for a kernel past what memory holds.
const MAX_OCTAVES: i32 = 4;
/// How long a wait for a bank (offline, or at activation) lasts at most, so a design thread that
/// has gone can never hang the host.
const LONGEST_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// A Linear-phase engine's sizes at one rate: the kernel's N and the partition's B, powers of
/// two, both scaled from their 44.1–48 kHz lengths by the power of two nearest `rate / 48 kHz`, so a
/// resolution keeps its time and N/B is the same at every rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizes {
    pub kernel: usize,
    pub block: usize,
}

impl Sizes {
    pub fn new(sample_rate: f64, resolution: Resolution) -> Sizes {
        // `usable_sample_rate` holds the rate at or above 1 kHz, at most six octaves down.
        let octaves = ((crate::usable_sample_rate(sample_rate) / 48_000.0)
            .log2()
            .round() as i32)
            .min(MAX_OCTAVES);
        let scaled = |base: usize| {
            if octaves >= 0 {
                base << octaves.unsigned_abs()
            } else {
                base >> octaves.unsigned_abs()
            }
        };
        let block = scaled(BASE_BLOCK).max(MIN_BLOCK);
        let kernel = scaled(resolution.base_points()).max(block * 8);
        Sizes { kernel, block }
    }

    /// The kernel's partitions.
    pub fn parts(self) -> usize {
        self.kernel / self.block
    }

    /// The latency in samples: the kernel's centre and one partition.
    pub fn latency(self) -> usize {
        self.kernel / 2 + self.block
    }

    /// An update interval in samples.
    pub fn update(self) -> usize {
        self.block * BLOCKS_PER_UPDATE
    }

    /// From the input's last nonzero sample, how long the output can still sound: the last
    /// partition holding sound leaves the delay line within this.
    pub fn tail(self) -> usize {
        self.kernel + 2 * self.block
    }
}

/// What the kernel hears (plan §7.7): every band in the sound — its static design, gain scale
/// applied, and its placement — and a sequence number. Plain data, copied to the design thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    pub bands: [Option<(Cascade, Placement)>; SLOTS],
    pub seq: u64,
}

impl Heard {
    pub const EMPTY: Heard = Heard {
        bands: [None; SLOTS],
        seq: 0,
    };
}

/// How many kernels the EQ heard takes: none (a delay), one for both channels, one each, or the
/// 2×2 matrix of the two stages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Identity,
    One,
    Two,
    Four,
}

impl Layout {
    pub fn kernels(self) -> usize {
        match self {
            Layout::Identity => 0,
            Layout::One => 1,
            Layout::Two => 2,
            Layout::Four => 4,
        }
    }

    pub fn of(heard: &Heard) -> Layout {
        let (mut any, mut channels, mut mid_side) = (false, false, false);
        for (_, placement) in heard.bands.iter().flatten() {
            any = true;
            match placement {
                Placement::Left | Placement::Right => channels = true,
                Placement::Mid | Placement::Side => mid_side = true,
                Placement::Stereo => {}
            }
        }
        match (any, channels, mid_side) {
            (false, _, _) => Layout::Identity,
            (_, _, true) => Layout::Four,
            (_, true, false) => Layout::Two,
            _ => Layout::One,
        }
    }
}

/// Each stage's magnitude at `freq`, zero phase (plan §7.3): the channel stage's left and right,
/// then the mid/side stage's mid and side.
pub fn stage_magnitudes(heard: &Heard, freq: f64, sample_rate: f64) -> [f64; 4] {
    let t = tan_half(freq, sample_rate);
    let mut power = [1.0f64; 4];
    for (cascade, placement) in heard.bands.iter().flatten() {
        let p: f64 = cascade
            .sections()
            .iter()
            .map(|section| section.power(t))
            .product();
        let apply: &[usize] = match placement {
            Placement::Stereo => &[0, 1],
            Placement::Left => &[0],
            Placement::Right => &[1],
            Placement::Mid => &[2],
            Placement::Side => &[3],
        };
        for &index in apply {
            power[index] *= p;
        }
    }
    power.map(f64::sqrt)
}

/// The kernels' real gains from the stages' magnitudes: one; left and right; or the matrix
/// `L′ = a·l·L + b·r·R`, `R′ = b·l·L + a·r·R`, `a, b = (m ± s)/2`, as `[LL, LR, RL, RR]`.
pub fn kernel_gains(layout: Layout, [l, r, m, s]: [f64; 4]) -> [f64; 4] {
    match layout {
        Layout::Identity => [1.0, 0.0, 0.0, 1.0],
        Layout::One => [l, 0.0, 0.0, 0.0],
        Layout::Two => [l, r, 0.0, 0.0],
        Layout::Four => {
            let (a, b) = (0.5 * (m + s), 0.5 * (m - s));
            [a * l, b * r, b * l, a * r]
        }
    }
}

/// What each line hears from the kernels' gains, `[L→L, R→R, M→M, S→S]`, as the graph draws a
/// channel's response to itself (plan §7.3): linear in the gains, so it commutes with the window.
fn lines(layout: Layout, g: [f64; 4]) -> [f64; 4] {
    match layout {
        Layout::Identity => [1.0; 4],
        Layout::One => [g[0]; 4],
        Layout::Two => {
            let both = 0.5 * (g[0] + g[1]);
            [g[0], g[1], both, both]
        }
        Layout::Four => [
            g[0],
            g[3],
            0.5 * (g[0] + g[1] + g[2] + g[3]),
            0.5 * (g[0] - g[1] - g[2] + g[3]),
        ],
    }
}

/// What Linear phase's kernel does at `freq` (plan §7.7), as each line hears it, `[L→L, R→R, M→M,
/// S→S]` in magnitude: at the bins either side the kernels' gains smoothed by the window's
/// transform (¼, ½, ¼, exactly what the periodic Hann does to a sampled response), read between
/// the two bins in dB. What the editor draws.
pub fn heard_lines(heard: &Heard, freq: f64, sample_rate: f64, sizes: Sizes) -> [f64; 4] {
    let sample_rate = crate::usable_sample_rate(sample_rate);
    let n = sizes.kernel as f64;
    let half = sizes.kernel / 2;
    let layout = Layout::of(heard);
    let gains_at = |bin: isize| {
        // A real response's bins mirror about DC and Nyquist.
        let bin = bin.unsigned_abs();
        let bin = if bin > half {
            2 * half - bin.min(2 * half)
        } else {
            bin
        };
        kernel_gains(
            layout,
            stage_magnitudes(heard, bin as f64 * sample_rate / n, sample_rate),
        )
    };
    let smoothed = |bin: isize| {
        let (a, b, c) = (gains_at(bin - 1), gains_at(bin), gains_at(bin + 1));
        lines(
            layout,
            std::array::from_fn(|k| 0.25 * a[k] + 0.5 * b[k] + 0.25 * c[k]),
        )
    };
    let at = (freq.max(0.0) * n / sample_rate).min(half as f64);
    let below = at.floor();
    let t = at - below;
    let (low, high) = (smoothed(below as isize), smoothed(below as isize + 1));
    let db = |value: f64| 20.0 * value.abs().max(1.0e-12).log10();
    std::array::from_fn(|k| {
        let mixed = db(low[k]) + (db(high[k]) - db(low[k])) * t;
        10f64.powf(mixed / 20.0)
    })
}

// ---- banks and their design ----------------------------------------------------------------

/// One set of kernels: its layout, the request it was designed from, and each kernel's partitions'
/// spectra, `parts × (B + 1)` bins each.
struct Bank {
    layout: Layout,
    seq: u64,
    kernels: [Vec<Complex<f64>>; 4],
}

impl Bank {
    fn new(sizes: Sizes) -> Bank {
        let len = sizes.parts() * (sizes.block + 1);
        Bank {
            layout: Layout::Identity,
            seq: 0,
            kernels: std::array::from_fn(|_| vec![Complex::new(0.0, 0.0); len]),
        }
    }
}

/// The design: every buffer a kernel needs, made ahead of use.
struct Designer {
    sizes: Sizes,
    sample_rate: f64,
    inverse: RealFft<f64>,
    inverse_scratch: Vec<Complex<f64>>,
    spectrum: Vec<Complex<f64>>,
    zero_phase: Vec<f64>,
    taps: Vec<f64>,
    window: Vec<f64>,
    gains: Vec<[f64; 4]>,
    forward: RealFft<f64>,
    forward_scratch: Vec<Complex<f64>>,
    segment: Vec<f64>,
}

impl Designer {
    fn new(sizes: Sizes, sample_rate: f64) -> Designer {
        let n = sizes.kernel;
        let inverse = Planner::<f64>::new().real(n);
        let forward = Planner::<f64>::new().real(2 * sizes.block);
        Designer {
            sizes,
            sample_rate,
            inverse_scratch: inverse.make_scratch(),
            spectrum: vec![Complex::new(0.0, 0.0); inverse.spectrum_len()],
            inverse,
            zero_phase: vec![0.0; n],
            taps: vec![0.0; n],
            window: (0..n)
                .map(|m| (std::f64::consts::PI * m as f64 / n as f64).sin().powi(2))
                .collect(),
            gains: vec![[0.0; 4]; n / 2 + 1],
            forward_scratch: forward.make_scratch(),
            segment: vec![0.0; 2 * sizes.block],
            forward,
        }
    }

    /// Designs `heard` into `bank`.
    fn design(&mut self, heard: &Heard, bank: &mut Bank) {
        let Sizes {
            kernel: n,
            block: b,
        } = self.sizes;
        let half = n / 2;
        let layout = Layout::of(heard);
        bank.layout = layout;
        bank.seq = heard.seq;
        if layout == Layout::Identity {
            return;
        }
        for (bin, gains) in self.gains.iter_mut().enumerate() {
            let freq = bin as f64 * self.sample_rate / n as f64;
            // Every valid gain is finite in `f64`; a non-finite one is no sound.
            *gains = kernel_gains(layout, stage_magnitudes(heard, freq, self.sample_rate))
                .map(|gain| if gain.is_finite() { gain } else { 0.0 });
        }
        for kernel in 0..layout.kernels() {
            for (value, gains) in self.spectrum.iter_mut().zip(&self.gains) {
                *value = Complex::new(gains[kernel], 0.0);
            }
            // The lengths are the plan's own, so the transform cannot refuse them.
            let _ = self.inverse.inverse(
                &mut self.spectrum,
                &mut self.zero_phase,
                &mut self.inverse_scratch,
            );
            // Centred on tap N/2, windowed, its pairs mirrored: symmetric to the bit.
            for (m, tap) in self.taps.iter_mut().enumerate() {
                *tap = self.zero_phase[(m + half) % n] * self.window[m];
            }
            self.taps[0] = 0.0;
            for j in 1..half {
                let mean = 0.5 * (self.taps[half - j] + self.taps[half + j]);
                self.taps[half - j] = mean;
                self.taps[half + j] = mean;
            }
            let spectra = &mut bank.kernels[kernel];
            for part in 0..self.sizes.parts() {
                for (out, tap) in self.segment[..b]
                    .iter_mut()
                    .zip(&self.taps[part * b..(part + 1) * b])
                {
                    *out = *tap;
                }
                self.segment[b..].fill(0.0);
                let _ = self.forward.forward(
                    &mut self.segment,
                    &mut spectra[part * (b + 1)..(part + 1) * (b + 1)],
                    &mut self.forward_scratch,
                );
            }
        }
    }
}

// ---- the handoff ---------------------------------------------------------------------------

const FREE: u8 = 0;
const WRITING: u8 = 1;
const READY: u8 = 2;
const USED: u8 = 3;

/// What the audio thread and the design thread share: the request, triple-buffered, and three
/// banks, each owned by one side at a time through its state.
struct Shared {
    /// Three request slots: the audio thread writes its own, then swaps it into the middle (the
    /// index and a fresh bit in `middle`); the design thread swaps the middle out when fresh.
    requests: [UnsafeCell<Heard>; 3],
    middle: AtomicU8,
    banks: [UnsafeCell<Bank>; 3],
    states: [AtomicU8; 3],
    seqs: [AtomicU64; 3],
    exit: AtomicBool,
}

// SAFETY: every cell is reached only by the side that owns it at that moment: a request slot by
// the side holding its index (`middle` hands indices over atomically), a bank by the design thread
// while WRITING and by the audio thread while USED; the states move with acquire/release ordering.
unsafe impl Sync for Shared {}

const FRESH: u8 = 0b100;
const INDEX: u8 = 0b011;

/// The design thread: requests in, banks out, until the engine goes.
fn design_thread(shared: Arc<Shared>, mut designer: Designer) {
    let mut front = 2u8;
    let mut pending: Option<Heard> = None;
    while !shared.exit.load(Ordering::Acquire) {
        if shared.middle.load(Ordering::Acquire) & FRESH != 0 {
            front = shared.middle.swap(front, Ordering::AcqRel) & INDEX;
            // SAFETY: the slot at `front` is this thread's until it swaps it back.
            pending = Some(unsafe { *shared.requests[usize::from(front)].get() });
        }
        if let Some(heard) = pending {
            let claimed = (0..3).find(|&index| {
                shared.states[index]
                    .compare_exchange(FREE, WRITING, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
            });
            if let Some(index) = claimed {
                // SAFETY: WRITING makes the bank this thread's alone.
                designer.design(&heard, unsafe { &mut *shared.banks[index].get() });
                shared.seqs[index].store(heard.seq, Ordering::Relaxed);
                shared.states[index].store(READY, Ordering::Release);
                // A newer bank makes an older one not yet taken worthless.
                for other in (0..3).filter(|&other| other != index) {
                    if shared.seqs[other].load(Ordering::Relaxed) < heard.seq {
                        let _ = shared.states[other].compare_exchange(
                            READY,
                            FREE,
                            Ordering::AcqRel,
                            Ordering::Relaxed,
                        );
                    }
                }
                pending = None;
                continue;
            }
        }
        std::thread::sleep(DESIGN_REST);
    }
}

// ---- the convolver -------------------------------------------------------------------------

/// The engine's Linear phase: its sizes, the convolver, the banks in use and the design thread.
pub struct Linear {
    sizes: Sizes,
    resolution: Resolution,
    /// An update boundary passed in the last frame, for the engine to ask for a kernel.
    boundary_passed: bool,
    channels: usize,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    /// The request slot the audio thread writes, and the last sequence asked for.
    back: u8,
    asked: u64,
    /// The bank heard, and the one fading out with how far the fade has gone.
    current: Option<usize>,
    old: Option<usize>,
    faded: usize,
    offline: bool,
    /// After a restart in realtime: the bank asked for is taken the first partition it is ready.
    fresh: bool,
    /// Whether a partition with sound in it has been convolved since the last clear.
    sounded: bool,
    blocks: usize,
    forward: RealFft<f64>,
    inverse: RealFft<f64>,
    scratch: Vec<Complex<f64>>,
    /// Per channel the transform's window, the partition before and the one filling.
    window: [Vec<f64>; 2],
    filled: usize,
    /// The frequency-domain delay line: per channel `parts × (B + 1)`, `head` the newest.
    line: [Vec<Complex<f64>>; 2],
    head: usize,
    work_in: Vec<f64>,
    work_out: Vec<f64>,
    accumulator: Vec<Complex<f64>>,
    /// The partition's output, per bank in use (current, old) and channel.
    outputs: [[Vec<f64>; 2]; 2],
    /// The delay line of the latency: an empty EQ's output, and solo's input.
    delay: [Vec<f32>; 2],
    delay_at: usize,
    since_input: usize,
}

impl Linear {
    /// Linear phase at `sample_rate` and `resolution` for `channels` (1 or 2): every buffer
    /// allocated and the design thread started here, never on the audio thread. `None` if the
    /// thread could not be started.
    pub fn new(sample_rate: f64, resolution: Resolution, channels: usize) -> Option<Linear> {
        let sample_rate = crate::usable_sample_rate(sample_rate);
        let sizes = Sizes::new(sample_rate, resolution);
        let shared = Arc::new(Shared {
            requests: std::array::from_fn(|_| UnsafeCell::new(Heard::EMPTY)),
            middle: AtomicU8::new(1),
            banks: std::array::from_fn(|_| UnsafeCell::new(Bank::new(sizes))),
            states: std::array::from_fn(|_| AtomicU8::new(FREE)),
            seqs: std::array::from_fn(|_| AtomicU64::new(0)),
            exit: AtomicBool::new(false),
        });
        let designer = Designer::new(sizes, sample_rate);
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("mxm-fx-eq-n linear phase".to_owned())
                .spawn(move || design_thread(shared, designer))
                .ok()?
        };
        let b = sizes.block;
        let mut planner = Planner::<f64>::new();
        let forward = planner.real(2 * b);
        let inverse = forward.clone();
        let line_len = sizes.parts() * (b + 1);
        Some(Linear {
            sizes,
            resolution,
            boundary_passed: false,
            channels: channels.clamp(1, 2),
            shared,
            thread: Some(thread),
            back: 0,
            asked: 0,
            current: None,
            old: None,
            faded: 0,
            offline: false,
            fresh: false,
            sounded: false,
            blocks: 0,
            scratch: forward.make_scratch(),
            forward,
            inverse,
            window: std::array::from_fn(|_| vec![0.0; 2 * b]),
            filled: 0,
            line: std::array::from_fn(|_| vec![Complex::new(0.0, 0.0); line_len]),
            head: 0,
            work_in: vec![0.0; 2 * b],
            work_out: vec![0.0; 2 * b],
            accumulator: vec![Complex::new(0.0, 0.0); b + 1],
            outputs: std::array::from_fn(|_| std::array::from_fn(|_| vec![0.0; b])),
            delay: std::array::from_fn(|_| vec![0.0; sizes.latency()]),
            delay_at: 0,
            since_input: usize::MAX,
        })
    }

    pub fn sizes(&self) -> Sizes {
        self.sizes
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Whether an update boundary passed in the last frame; read once.
    pub fn take_boundary(&mut self) -> bool {
        std::mem::take(&mut self.boundary_passed)
    }

    /// No fade in progress and the newest kernel asked for is the one heard.
    pub fn is_settled(&self) -> bool {
        self.old.is_none() && self.heard_seq() >= self.asked
    }

    pub fn latency(&self) -> usize {
        self.sizes.latency()
    }

    /// Offline rendering: a bank asked for is waited for at the boundary after, so a bounce is the
    /// same every time.
    pub fn set_offline(&mut self, offline: bool) {
        self.offline = offline;
    }

    /// The sequence of the bank heard, 0 before any.
    pub fn heard_seq(&self) -> u64 {
        self.current
            .map_or(0, |index| self.shared.seqs[index].load(Ordering::Relaxed))
    }

    /// Asks for a kernel for `heard`, whose sequence must rise with every request.
    pub fn ask(&mut self, heard: &Heard) {
        // SAFETY: the slot at `back` is the audio thread's until it is swapped into the middle.
        unsafe {
            *self.shared.requests[usize::from(self.back)].get() = *heard;
        }
        self.back = self.shared.middle.swap(self.back | FRESH, Ordering::AcqRel) & INDEX;
        self.asked = heard.seq;
    }

    /// The newest finished bank newer than the one heard, taken for use.
    fn take_ready(&mut self) -> Option<usize> {
        let heard = self.heard_seq();
        let newest = (0..3)
            .filter(|&index| self.shared.states[index].load(Ordering::Acquire) == READY)
            .max_by_key(|&index| self.shared.seqs[index].load(Ordering::Relaxed))?;
        if self.current.is_some() && self.shared.seqs[newest].load(Ordering::Relaxed) <= heard {
            return None;
        }
        self.shared.states[newest]
            .compare_exchange(READY, USED, Ordering::AcqRel, Ordering::Relaxed)
            .ok()
            .map(|_| newest)
    }

    /// Whether a finished bank holds the kernel asked for last, or a newer one.
    fn asked_ready(&self) -> bool {
        (0..3).any(|index| {
            self.shared.states[index].load(Ordering::Acquire) == READY
                && self.shared.seqs[index].load(Ordering::Relaxed) >= self.asked
        })
    }

    /// Waits, at most [`LONGEST_WAIT`], for the bank asked for last: only offline and at
    /// activation, never on a realtime audio thread.
    fn wait_for_asked(&mut self) -> Option<usize> {
        let until = std::time::Instant::now() + LONGEST_WAIT;
        loop {
            if self.heard_seq() >= self.asked {
                return None;
            }
            if self.asked_ready() {
                return self.take_ready();
            }
            if std::time::Instant::now() > until {
                return None;
            }
            std::thread::yield_now();
        }
    }

    /// The kernel for `heard`, designed now and heard from the next sample, no fade: for
    /// activation, off the audio thread.
    pub fn prime(&mut self, heard: &Heard) {
        self.ask(heard);
        if let Some(index) = self.wait_for_asked() {
            self.hear_at_once(index);
        }
    }

    /// After [`Linear::clear`], the kernel for `heard` (a reset, plan §7.7): offline designed now
    /// and heard from the next sample, as at activation; in realtime asked for, never waited for,
    /// and taken the first partition it is ready — unfaded while nothing sounding has been
    /// convolved since the clear, so heard as if primed; else at the next update boundary, faded
    /// in as any change is.
    pub fn restart(&mut self, heard: &Heard) {
        if self.offline {
            self.prime(heard);
        } else {
            self.ask(heard);
            self.fresh = true;
        }
    }

    /// `index` heard from the next partition, no fade: the banks in use released.
    fn hear_at_once(&mut self, index: usize) {
        let (current, old) = (self.current.take(), self.old.take());
        self.release(current);
        self.release(old);
        self.current = Some(index);
    }

    /// A restart's bank, at a partition: taken unfaded while every partition convolved since the
    /// clear was silent, since every kernel's output on silence is the same nothing.
    fn take_fresh(&mut self) {
        if self.sounded || self.heard_seq() >= self.asked {
            self.fresh = false;
        } else if self.asked_ready() {
            if let Some(index) = self.take_ready() {
                self.hear_at_once(index);
            }
            self.fresh = false;
        }
    }

    fn release(&self, bank: Option<usize>) {
        if let Some(index) = bank {
            self.shared.states[index].store(FREE, Ordering::Release);
        }
    }

    /// Clears every signal: the delay lines, the partitions and the outputs; a fade in progress
    /// finishes. The banks stay.
    pub fn clear(&mut self) {
        for channel in 0..2 {
            self.window[channel].fill(0.0);
            self.line[channel].fill(Complex::new(0.0, 0.0));
            self.delay[channel].fill(0.0);
            for outputs in &mut self.outputs {
                outputs[channel].fill(0.0);
            }
        }
        self.filled = 0;
        self.head = 0;
        self.delay_at = 0;
        self.blocks = 0;
        let old = self.old.take();
        self.release(old);
        self.faded = 0;
        self.since_input = usize::MAX;
        self.fresh = false;
        self.sounded = false;
    }

    /// Whether the output may still sound: the input's last nonzero sample within the tail.
    pub fn is_ringing(&self) -> bool {
        self.since_input < self.sizes.tail()
    }

    /// An update boundary (plan §7.7): a fade that has run its interval ends; offline, the bank
    /// asked for is waited for; the newest finished bank is taken and faded in.
    fn boundary(&mut self) {
        self.boundary_passed = true;
        if self.old.is_some() && self.faded >= self.sizes.update() {
            let old = self.old.take();
            self.release(old);
        }
        if self.old.is_some() {
            return;
        }
        let taken = if self.offline && self.asked > self.heard_seq() {
            self.wait_for_asked()
        } else {
            self.take_ready()
        };
        if let Some(index) = taken {
            self.old = self.current.replace(index);
            self.faded = 0;
            if self.old.is_none() {
                // Nothing to fade from: heard at once.
                self.faded = self.sizes.update();
            }
        }
    }

    /// The partition just filled: into the delay line, and out through each bank in use.
    fn block(&mut self) {
        self.blocks += 1;
        if self.fresh {
            self.take_fresh();
        }
        if self.blocks.is_multiple_of(BLOCKS_PER_UPDATE) {
            self.boundary();
        }
        let b = self.sizes.block;
        let bins = b + 1;
        let parts = self.sizes.parts();
        let head = self.head;
        for channel in 0..self.channels {
            self.work_in.copy_from_slice(&self.window[channel]);
            let _ = self.forward.forward(
                &mut self.work_in,
                &mut self.line[channel][head * bins..(head + 1) * bins],
                &mut self.scratch,
            );
        }
        for (slot, bank) in [self.current, self.old].into_iter().enumerate() {
            let Some(index) = bank else {
                continue;
            };
            // SAFETY: USED makes the bank the audio thread's; the design thread never writes it.
            let bank = unsafe { &*self.shared.banks[index].get() };
            if bank.layout == Layout::Identity {
                continue;
            }
            for out in 0..self.channels {
                self.accumulator.fill(Complex::new(0.0, 0.0));
                for part in 0..parts {
                    let at = (head + parts - part) % parts;
                    let range = part * bins..(part + 1) * bins;
                    let input = |channel: usize| &self.line[channel][at * bins..(at + 1) * bins];
                    let mut add = |kernel: &[Complex<f64>], input: &[Complex<f64>]| {
                        for ((sum, h), x) in self.accumulator.iter_mut().zip(kernel).zip(input) {
                            *sum += h * x;
                        }
                    };
                    match bank.layout {
                        Layout::One => add(&bank.kernels[0][range], input(out)),
                        Layout::Two => add(&bank.kernels[out][range], input(out)),
                        Layout::Four => {
                            add(&bank.kernels[2 * out][range.clone()], input(0));
                            add(&bank.kernels[2 * out + 1][range], input(1));
                        }
                        Layout::Identity => {}
                    }
                }
                let _ = self.inverse.inverse(
                    &mut self.accumulator,
                    &mut self.work_out,
                    &mut self.scratch,
                );
                self.outputs[slot][out].copy_from_slice(&self.work_out[b..]);
            }
        }
        for channel in 0..self.channels {
            self.window[channel].copy_within(b.., 0);
        }
        self.head = (head + 1) % parts;
        self.filled = 0;
        // `tick` counted this partition's samples before calling here.
        self.sounded = self.since_input != usize::MAX;
    }

    /// One frame through the kernel: the output, `latency()` behind, and the input delayed by the
    /// same, for solo.
    pub fn tick<const C: usize>(&mut self, input: [f64; C]) -> ([f64; C], [f64; C]) {
        let b = self.sizes.block;
        let at = self.filled;
        let mut out = [0.0f64; C];
        let mut delayed = [0.0f64; C];
        let heard = |bank: Option<usize>| {
            // SAFETY: as in `block`.
            bank.map(|index| unsafe { &*self.shared.banks[index].get() }.layout)
        };
        let (current, old) = (heard(self.current), heard(self.old));
        let fading = old.is_some() && self.faded < self.sizes.update();
        let share = if fading {
            self.faded as f64 / self.sizes.update() as f64
        } else {
            1.0
        };
        let mut sounding = false;
        for c in 0..C.min(self.channels) {
            // The delay line keeps the input as it came, `f32`, so an empty EQ is bit-exact.
            let x = input[c] as f32;
            sounding |= x != 0.0;
            let late = self.delay[c][self.delay_at];
            self.delay[c][self.delay_at] = x;
            delayed[c] = f64::from(late);
            self.window[c][b + at] = input[c];
            let through = |layout: Option<Layout>, slot: usize| match layout {
                Some(Layout::Identity) | None => f64::from(late),
                Some(_) => self.outputs[slot][c][at],
            };
            let now = through(current, 0);
            out[c] = if fading {
                let before = through(old, 1);
                before + (now - before) * share
            } else {
                now
            };
        }
        self.delay_at = (self.delay_at + 1) % self.delay[0].len();
        if fading {
            self.faded += 1;
        }
        self.since_input = if sounding {
            0
        } else {
            self.since_input.saturating_add(1)
        };
        self.filled += 1;
        if self.filled == b {
            self.block();
        }
        (out, delayed)
    }
}

impl Drop for Linear {
    fn drop(&mut self) {
        self.shared.exit.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
