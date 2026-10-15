//! The analyser's editor half (plan §4): a contiguous history per stream, real FFTs through the
//! shared `crates/mxm-fft`, and the curves the graph draws behind the bands.
//!
//! - **The display reads band power**, a real-time analyser's way: the power in a twelfth of an
//!   octave around each drawn frequency, summed over the bins it covers (fractions at its edges),
//!   averaged over the channels so anti-phase or side-only energy still shows (plan §4). A
//!   full-scale sine reads 0 dB, and noise reads the same whatever the window's length, so the long
//!   window takes over the lows without a step.
//! - **Two windows, two rates.** A short window of about 85 ms at the full rate is analysed when a
//!   quarter of it is new. **The lows' long window runs on a decimated signal** (plan §4): an
//!   eighth-order low pass, then every `factor`-th sample, about 3 kHz whatever the rate, so a
//!   1.4 s window is 4,096 points rather than 270,000. It is recomputed when an eighth of it is new.
//!   Below [`CROSSOVER_HZ`] the long window is read once it holds enough audio.
//! - **Coherent windows.** A dropped block or a new generation restarts both windows, never splicing
//!   across the gap; a block recorded at another rate is ignored. **A silence is a gap too**: with
//!   no audio for [`STALE_SECONDS`] the windows' readings go, the display falls to the floor, and
//!   the next audio restarts them.
//! - **Ballistics**: a rise shows at once, a fall at the chosen speed. **Tilt** about 1 kHz.

use mxm_fft::{Complex, Planner, RealFft};

use crate::analyser::{BLOCK_FRAMES, Block};
use crate::view::{Resolution, Show, ViewSettings};

/// The quietest level drawn.
pub const FLOOR_DB: f32 = -120.0;
/// The share of an octave each drawn frequency reads.
pub const BAND_OCTAVES: f32 = 1.0 / 12.0;
/// Below this the long window, once ready, is read instead of the short one.
pub const CROSSOVER_HZ: f32 = 300.0;
/// With no audio for this long, what the windows hold is the sound from before: the host stopped
/// calling the plugin (it sleeps on silence, or its transport stopped). Their readings go, the
/// display falls to the floor at the chosen speed, and the next audio restarts them as a gap does.
/// Longer than any host's block.
const STALE_SECONDS: f64 = 0.3;

/// The decimated rate the long window aims at.
const LONG_RATE: f32 = 3_000.0;
/// The anti-alias low pass, as a share of the decimated rate.
const LONG_CUTOFF: f64 = 0.4;
/// An eighth-order Butterworth as four sections: their Q values.
const BUTTERWORTH_8: [f64; 4] = [0.509_795_6, 0.601_344_9, 0.899_976_2, 2.562_915_4];

/// Pre left, pre right, post left, post right, key left, key right.
const STREAMS: usize = 6;
/// Pre, post and the side chain's key (plan §7.4).
const OUTPUTS: usize = 3;

/// One window: a ring of each stream, its plan and its working buffers, and the power per bin of
/// pre and post.
struct Window {
    len: usize,
    history: [Vec<f32>; STREAMS],
    write: usize,
    contiguous: usize,
    /// Samples since the last analysis.
    since: usize,
    hop: usize,
    fft: RealFft<f32>,
    window: Vec<f32>,
    /// Band power of a bin: `4 / (N · Σw²)` makes a full-scale sine's lobe sum to 1.
    scale: f32,
    input: Vec<f32>,
    output: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    /// Prefix sums of the power per bin, pre, post and key: `sums[s][k]` is bins `0..k`.
    sums: [Vec<f64>; OUTPUTS],
    /// Per output: whether its sums are current.
    analysed: [bool; OUTPUTS],
    /// One transform's power per bin, kept so a frame allocates nothing.
    power: Vec<f64>,
    bin_hz: f32,
}

impl Window {
    fn new(planner: &mut Planner<f32>, len: usize, rate: f32, hop: usize) -> Self {
        let fft = planner.real(len);
        let bins = fft.spectrum_len();
        // Hann.
        let window: Vec<f32> = (0..len)
            .map(|n| 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / len as f32).cos())
            .collect();
        let energy: f32 = window.iter().map(|w| w * w).sum();
        Self {
            len,
            history: std::array::from_fn(|_| vec![0.0; len]),
            write: 0,
            contiguous: 0,
            since: 0,
            hop: hop.max(1),
            scratch: fft.make_scratch(),
            output: vec![Complex::new(0.0, 0.0); fft.spectrum_len()],
            input: vec![0.0; len],
            sums: std::array::from_fn(|_| vec![0.0; fft.spectrum_len() + 1]),
            fft,
            window,
            scale: 4.0 / (len as f32 * energy),
            analysed: [false; OUTPUTS],
            power: vec![0.0; bins],
            bin_hz: rate / len as f32,
        }
    }

    fn restart(&mut self) {
        self.contiguous = 0;
        self.since = 0;
        self.analysed = [false; OUTPUTS];
    }

    fn push(&mut self, frame: [f32; STREAMS]) {
        for (stream, value) in frame.into_iter().enumerate() {
            self.history[stream][self.write] = value;
        }
        self.write = (self.write + 1) % self.len;
        self.contiguous = (self.contiguous + 1).min(self.len);
        self.since += 1;
    }

    fn ready(&self) -> bool {
        self.contiguous >= self.len
    }

    /// Recomputes the power per bin of the shown streams, when enough is new, or at once for a
    /// stream Show has just revealed. A hidden stream's sums fall behind, so it is not current.
    fn analyse(&mut self, channels: usize, show: Show, key: bool) {
        let shown = [
            matches!(show, Show::Pre | Show::Both),
            matches!(show, Show::Post | Show::Both),
            key,
        ];
        for (analysed, shown) in self.analysed.iter_mut().zip(shown) {
            if !shown {
                *analysed = false;
            }
        }
        let revealed = (0..OUTPUTS).any(|out| shown[out] && !self.analysed[out]);
        if !self.ready() || (!revealed && self.since < self.hop) {
            return;
        }
        self.since = 0;
        for (out, first) in [(0, 0), (1, 2), (2, 4)] {
            if !shown[out] {
                self.analysed[out] = false;
                continue;
            }
            self.power.fill(0.0);
            for channel in 0..channels {
                let history = &self.history[first + channel];
                for n in 0..self.len {
                    self.input[n] = history[(self.write + n) % self.len] * self.window[n];
                }
                if self
                    .fft
                    .forward(&mut self.input, &mut self.output, &mut self.scratch)
                    .is_err()
                {
                    return;
                }
                for (bin, value) in self.power.iter_mut().zip(&self.output) {
                    *bin += f64::from(value.norm_sqr() * self.scale);
                }
            }
            let sums = &mut self.sums[out];
            sums[0] = 0.0;
            for (k, bin) in self.power.iter().enumerate() {
                sums[k + 1] = sums[k] + bin / channels as f64;
            }
            self.analysed[out] = true;
        }
    }

    /// The band power between `low` and `high` hertz from stream `out`'s bins: each bin covers
    /// `[k − ½, k + ½]` bins, and a bin at an edge counts for the share inside.
    fn band(&self, out: usize, low: f32, high: f32) -> f64 {
        let sums = &self.sums[out];
        let last = (sums.len() - 1) as f64;
        let edge = |hz: f32| (f64::from(hz / self.bin_hz) + 0.5).clamp(0.0, last);
        let cumulative = |at: f64| {
            let whole = at.floor() as usize;
            let whole = whole.min(sums.len() - 1);
            let part = at - whole as f64;
            let next = (whole + 1).min(sums.len() - 1);
            sums[whole] + part * (sums[next] - sums[whole])
        };
        (cumulative(edge(high)) - cumulative(edge(low))).max(0.0)
    }
}

/// A biquad low pass in `f64`: the decimator's anti-alias sections.
#[derive(Clone, Copy, Default)]
struct LowPass {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl LowPass {
    fn new(cutoff: f64, rate: f64, q: f64) -> Self {
        let w = std::f64::consts::TAU * cutoff / rate;
        let alpha = w.sin() / (2.0 * q);
        let cos = w.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - cos) / 2.0 / a0,
            b1: (1.0 - cos) / a0,
            b2: (1.0 - cos) / 2.0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn tick(&mut self, x: f64) -> f64 {
        // A non-finite sample is silence here, and a state gone non-finite starts again: one bad
        // sample costs the long window a window, not the low bands for good.
        let x = if x.is_finite() { x } else { 0.0 };
        if !(self.z1.is_finite() && self.z2.is_finite()) {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if self.z1.abs() < 1.0e-30 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1.0e-30 {
            self.z2 = 0.0;
        }
        y
    }
}

/// The lows' long window and the decimator feeding it.
struct Long {
    resolution: Resolution,
    factor: usize,
    phase: usize,
    filters: [[LowPass; 4]; STREAMS],
    window: Window,
}

impl Long {
    fn new(planner: &mut Planner<f32>, resolution: Resolution, rate: f32) -> Option<Self> {
        let len = match resolution {
            Resolution::Low => return None,
            Resolution::Medium => 1_024,
            Resolution::High => 4_096,
        };
        let factor = ((rate / LONG_RATE).max(1.0) as usize).next_power_of_two();
        let decimated = rate / factor as f32;
        let section = |q| LowPass::new(LONG_CUTOFF * f64::from(decimated), f64::from(rate), q);
        Some(Self {
            resolution,
            factor,
            phase: 0,
            filters: std::array::from_fn(|_| BUTTERWORTH_8.map(section)),
            window: Window::new(planner, len, decimated, len / 8),
        })
    }

    fn restart(&mut self) {
        self.window.restart();
        self.phase = 0;
        for stream in &mut self.filters {
            for section in stream {
                section.z1 = 0.0;
                section.z2 = 0.0;
            }
        }
    }

    fn push(&mut self, frame: [f32; STREAMS]) {
        let mut filtered = [0.0f32; STREAMS];
        for (stream, value) in frame.into_iter().enumerate() {
            let mut x = f64::from(value);
            for section in &mut self.filters[stream] {
                x = section.tick(x);
            }
            filtered[stream] = x as f32;
        }
        self.phase += 1;
        if self.phase == self.factor {
            self.phase = 0;
            self.window.push(filtered);
        }
    }
}

/// The editor's analyser.
pub struct Spectrum {
    sample_rate: f32,
    planner: Planner<f32>,
    short: Window,
    long: Option<Long>,
    channels: usize,
    expected_seq: Option<u64>,
    generation: Option<u32>,
    gaps: u64,
    freqs: Vec<f32>,
    pub pre_db: Vec<f32>,
    pub post_db: Vec<f32>,
    /// The side chain's spectrum, while the Side chain view is on (plan §7.4).
    pub key_db: Vec<f32>,
    last_time: Option<f64>,
    /// Audio came since the last analysis, when audio last came, and whether the windows hold
    /// only the sound from before a silence ([`STALE_SECONDS`]).
    fresh: bool,
    last_audio: Option<f64>,
    stale: bool,
}

impl Spectrum {
    pub fn new(sample_rate: f32) -> Spectrum {
        let sample_rate = if sample_rate.is_finite() && sample_rate >= 8_000.0 {
            sample_rate
        } else {
            48_000.0
        };
        // About 85 ms at any rate: 4,096 points at 44.1 and 48 kHz, 16,384 at 192 kHz.
        let len = ((sample_rate * 0.085) as usize)
            .next_power_of_two()
            .clamp(1_024, 32_768);
        let mut planner = Planner::new();
        let short = Window::new(&mut planner, len, sample_rate, len / 4);
        Spectrum {
            sample_rate,
            planner,
            short,
            long: None,
            channels: 2,
            expected_seq: None,
            generation: None,
            gaps: 0,
            freqs: Vec::new(),
            pre_db: Vec::new(),
            post_db: Vec::new(),
            key_db: Vec::new(),
            last_time: None,
            fresh: false,
            last_audio: None,
            stale: false,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// How many times the windows restarted on a gap.
    pub fn gaps(&self) -> u64 {
        self.gaps
    }

    /// Forgets the history, as after the editor reopens or the analyser comes back on.
    pub fn restart(&mut self) {
        self.short.restart();
        if let Some(long) = &mut self.long {
            long.restart();
        }
        self.expected_seq = None;
        self.generation = None;
        self.fresh = false;
        self.last_audio = None;
        self.stale = false;
        self.pre_db.iter_mut().for_each(|db| *db = FLOOR_DB);
        self.post_db.iter_mut().for_each(|db| *db = FLOOR_DB);
    }

    /// Plans the long window a resolution needs. Allocates, on the GUI thread, only on a change.
    pub fn set_resolution(&mut self, resolution: Resolution) {
        if self.long.as_ref().map(|long| long.resolution) == Some(resolution)
            || (self.long.is_none() && resolution == Resolution::Low)
        {
            return;
        }
        self.long = Long::new(&mut self.planner, resolution, self.sample_rate);
    }

    /// The frame's time, before its blocks are ingested: a silence longer than
    /// [`STALE_SECONDS`] since the last audio makes the windows stale here, so the audio that ends
    /// it restarts them rather than being spliced on.
    pub fn observe(&mut self, now: f64) {
        let silent = self
            .last_audio
            .is_some_and(|last| now - last > STALE_SECONDS);
        if silent && !self.fresh && !self.stale {
            self.stale = true;
            self.short.analysed = [false; OUTPUTS];
            if let Some(long) = &mut self.long {
                long.window.analysed = [false; OUTPUTS];
            }
        }
    }

    pub fn ingest(&mut self, block: &Block) {
        if block.sample_rate != self.sample_rate {
            return;
        }
        let broken = self
            .generation
            .is_some_and(|generation| generation != block.generation);
        let dropped = self
            .expected_seq
            .is_some_and(|expected| block.seq != expected);
        if broken || dropped || std::mem::take(&mut self.stale) {
            // A gap in the audio, a dropped block or audio after a silence: restart, never splice
            // across.
            self.short.restart();
            if let Some(long) = &mut self.long {
                long.restart();
            }
            self.gaps += 1;
        }
        self.fresh = true;
        self.generation = Some(block.generation);
        self.expected_seq = Some(block.seq.wrapping_add(1));
        self.channels = usize::from(block.channels.clamp(1, 2));
        for index in 0..BLOCK_FRAMES {
            let frame = [
                block.pre[0][index],
                block.pre[1][index],
                block.post[0][index],
                block.post[1][index],
                block.key[0][index],
                block.key[1][index],
            ];
            self.short.push(frame);
            if let Some(long) = &mut self.long {
                long.push(frame);
            }
        }
    }

    /// Whether the short window holds a whole window of contiguous audio.
    pub fn ready(&self) -> bool {
        self.short.ready()
    }

    /// Whether the lows are read from the long window now.
    pub fn long_ready(&self) -> bool {
        self.long.as_ref().is_some_and(|long| long.window.ready())
    }

    /// Recomputes the displayed curves at `freqs`, at time `now` in seconds.
    pub fn analyse(&mut self, freqs: &[f32], now: f64, settings: &ViewSettings) {
        if self.freqs.as_slice() != freqs {
            self.freqs = freqs.to_vec();
            self.pre_db = vec![FLOOR_DB; freqs.len()];
            self.post_db = vec![FLOOR_DB; freqs.len()];
            self.key_db = vec![FLOOR_DB; freqs.len()];
        }
        let elapsed = self
            .last_time
            .map_or(0.0, |last| (now - last).clamp(0.0, 0.25)) as f32;
        self.last_time = Some(now);
        let fall = settings.speed.release_db_per_second() * elapsed;
        if std::mem::take(&mut self.fresh) {
            self.last_audio = Some(now);
        } else {
            self.observe(now);
        }
        if !self.stale {
            self.short
                .analyse(self.channels, settings.show, settings.side_chain);
            if let Some(long) = &mut self.long {
                long.window
                    .analyse(self.channels, settings.show, settings.side_chain);
            }
        }
        let long = self.long.as_ref().map(|long| &long.window);
        let short = &self.short;
        let half = 2f32.powf(BAND_OCTAVES / 2.0);
        let tilt = settings.tilt.db_per_octave();
        for (index, &freq) in self.freqs.iter().enumerate() {
            for (out, shown) in [
                (0, &mut self.pre_db),
                (1, &mut self.post_db),
                (2, &mut self.key_db),
            ] {
                let window = match long {
                    Some(long) if freq < CROSSOVER_HZ && long.analysed[out] => Some(long),
                    _ => Some(short).filter(|short| short.analysed[out]),
                };
                let current = &mut shown[index];
                let reading = window.map(|window| {
                    let power = window.band(out, freq / half, freq * half);
                    let db = 10.0 * power.max(1.0e-14).log10() as f32;
                    (db + tilt * (freq / 1_000.0).log2()).max(FLOOR_DB)
                });
                *current = match reading {
                    Some(db) if db >= *current => db,
                    Some(db) => (*current - fall).max(db),
                    None => (*current - fall).max(FLOOR_DB),
                };
            }
        }
    }
}

/// The analyser's budget on the GUI thread (plan §4): a 60 Hz frame's ingest and analysis, in the
/// worst case of every resolution, view and rate, in milliseconds.
pub const BUDGET_MS: f64 = 2.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{Speed, Tilt};

    /// **The analyser's cost, every resolution × pre/post/both × rate** (plan §4 and §8), on the GUI
    /// thread: one 60 Hz frame of blocks ingested and the curves analysed at 450 points, a graph
    /// about 900 points wide. Release only; the figures are in the plugin's `AGENTS.md`.
    ///
    /// `cargo test -p mxm-fx-eq-n --release --lib analyser_cost -- --ignored --nocapture`
    #[test]
    #[ignore = "a release measurement"]
    fn analyser_cost() {
        let freqs: Vec<f32> = (0..450)
            .map(|i| 10.0 * 2_000f32.powf(i as f32 / 449.0))
            .collect();
        let mut worst = 0.0f64;
        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            for resolution in Resolution::ALL {
                for show in Show::ALL {
                    let settings = ViewSettings {
                        resolution,
                        show,
                        ..ViewSettings::default()
                    };
                    let mut spectrum = Spectrum::new(rate);
                    spectrum.set_resolution(resolution);
                    let mut source = noise();
                    let mut seq = 0u64;
                    let per_frame = (rate / 60.0) as usize / BLOCK_FRAMES + 1;
                    let mut frame = |spectrum: &mut Spectrum, time: f64| {
                        for _ in 0..per_frame {
                            let mut block = Block {
                                seq,
                                generation: 0,
                                sample_rate: rate,
                                channels: 2,
                                pre: [[0.0; BLOCK_FRAMES]; 2],
                                post: [[0.0; BLOCK_FRAMES]; 2],
                                key: [[0.0; BLOCK_FRAMES]; 2],
                            };
                            for index in 0..BLOCK_FRAMES {
                                let value = source(0);
                                block.pre[0][index] = value;
                                block.pre[1][index] = -value;
                                block.post[0][index] = value * 0.5;
                                block.post[1][index] = value * 0.25;
                            }
                            spectrum.ingest(&block);
                            seq += 1;
                        }
                        spectrum.analyse(&freqs, time, &settings);
                    };
                    // Warm: two seconds, so the long window is full.
                    for n in 0..120 {
                        frame(&mut spectrum, f64::from(n) / 60.0);
                    }
                    let frames = 300;
                    let start = std::time::Instant::now();
                    for n in 0..frames {
                        frame(&mut spectrum, 2.0 + f64::from(n) / 60.0);
                    }
                    let ms = start.elapsed().as_secs_f64() * 1_000.0 / f64::from(frames);
                    worst = worst.max(ms);
                    println!("{rate:>7} Hz  {resolution:<6?} {show:<4?}  {ms:.3} ms a frame");
                }
            }
        }
        println!("worst {worst:.3} ms against {BUDGET_MS} ms");
        assert!(worst < BUDGET_MS, "{worst} ms a frame");
    }

    fn flat() -> ViewSettings {
        ViewSettings {
            tilt: Tilt::Flat,
            ..ViewSettings::default()
        }
    }

    /// Feeds `seconds` of `signal(n)` as consecutive blocks on both channels, pre and post.
    fn feed(
        spectrum: &mut Spectrum,
        seq: &mut u64,
        seconds: f32,
        signal: &mut dyn FnMut(usize) -> f32,
    ) {
        let blocks = (spectrum.sample_rate() * seconds) as usize / BLOCK_FRAMES;
        for _ in 0..blocks {
            let mut block = Block {
                seq: *seq,
                generation: 0,
                sample_rate: spectrum.sample_rate(),
                channels: 2,
                pre: [[0.0; BLOCK_FRAMES]; 2],
                post: [[0.0; BLOCK_FRAMES]; 2],
                key: [[0.0; BLOCK_FRAMES]; 2],
            };
            for frame in 0..BLOCK_FRAMES {
                let value = signal(*seq as usize * BLOCK_FRAMES + frame);
                block.pre[0][frame] = value;
                block.pre[1][frame] = value;
                block.post[0][frame] = value * 0.5;
                block.post[1][frame] = value * 0.5;
            }
            spectrum.ingest(&block);
            *seq += 1;
        }
    }

    fn reading(spectrum: &mut Spectrum, freq: f32, settings: &ViewSettings) -> (f32, f32) {
        spectrum.analyse(&[freq], 0.0, settings);
        (spectrum.pre_db[0], spectrum.post_db[0])
    }

    #[test]
    fn a_full_scale_sine_reads_0_db_in_both_windows_and_post_half_minus_6() {
        for (resolution, freq) in [
            (Resolution::Low, 1_000.0f32),
            (Resolution::Medium, 120.0),
            (Resolution::High, 60.0),
        ] {
            let mut spectrum = Spectrum::new(48_000.0);
            spectrum.set_resolution(resolution);
            let mut seq = 0;
            let rate = spectrum.sample_rate();
            feed(&mut spectrum, &mut seq, 2.0, &mut |n| {
                (std::f32::consts::TAU * freq * n as f32 / rate).sin()
            });
            assert_eq!(spectrum.long_ready(), resolution != Resolution::Low);
            let (pre, post) = reading(&mut spectrum, freq, &flat());
            assert!(pre.abs() < 0.5, "{resolution:?} at {freq} Hz: {pre}");
            assert!((post - pre + 6.02).abs() < 0.1, "{post}");
        }
    }

    fn noise() -> impl FnMut(usize) -> f32 {
        let mut state = 0x2545_f491u32;
        move |_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    #[test]
    fn noise_reads_alike_on_both_sides_of_the_crossover() {
        // White noise has equal power per hertz, so a twelfth of an octave just below the crossover
        // holds 6 % less than one just above: 0.26 dB. The long window must not read it otherwise.
        // Averaged in power over many analyses, with the fastest fall so the display follows each:
        // a band of few bins wanders more, and an average in dB would read it low.
        let settings = ViewSettings {
            speed: Speed::Fast,
            ..flat()
        };
        for resolution in [Resolution::Medium, Resolution::High] {
            let mut spectrum = Spectrum::new(48_000.0);
            spectrum.set_resolution(resolution);
            let mut seq = 0;
            let mut source = noise();
            feed(&mut spectrum, &mut seq, 3.0, &mut source);
            let (mut below, mut above) = (0.0f64, 0.0f64);
            let frames = 120;
            for frame in 0..frames {
                feed(&mut spectrum, &mut seq, 0.2, &mut source);
                spectrum.analyse(
                    &[CROSSOVER_HZ * 0.97, CROSSOVER_HZ * 1.03],
                    frame as f64,
                    &settings,
                );
                below += 10f64.powf(f64::from(spectrum.pre_db[0]) / 10.0);
                above += 10f64.powf(f64::from(spectrum.pre_db[1]) / 10.0);
            }
            let difference = 10.0 * (below / above).log10();
            assert!(
                (difference + 0.26).abs() < 1.0,
                "{resolution:?}: the long window reads {difference:.2} dB against the short"
            );
        }
    }

    #[test]
    fn the_long_window_resolves_what_the_short_cannot() {
        let tone = |spectrum: &mut Spectrum| {
            let mut seq = 0;
            let rate = spectrum.sample_rate();
            feed(spectrum, &mut seq, 3.0, &mut |n| {
                (std::f32::consts::TAU * 60.0 * n as f32 / rate).sin()
            });
            // A semitone and a half above the tone.
            spectrum.analyse(&[60.0, 60.0 * 2f32.powf(1.5 / 12.0)], 0.0, &flat());
            spectrum.pre_db[0] - spectrum.pre_db[1]
        };
        let mut high = Spectrum::new(48_000.0);
        high.set_resolution(Resolution::High);
        let mut low = Spectrum::new(48_000.0);
        low.set_resolution(Resolution::Low);
        let (high, low) = (tone(&mut high), tone(&mut low));
        assert!(
            high > 30.0,
            "High separates the tone from its neighbour: {high} dB"
        );
        assert!(low < 15.0, "Low smears it across: {low} dB");
    }

    #[test]
    fn a_gap_restarts_both_windows_and_another_rate_is_ignored() {
        let mut spectrum = Spectrum::new(48_000.0);
        spectrum.set_resolution(Resolution::Medium);
        let mut seq = 0;
        feed(&mut spectrum, &mut seq, 1.0, &mut noise());
        assert!(spectrum.ready() && spectrum.long_ready());
        seq += 1;
        feed(&mut spectrum, &mut seq, 0.02, &mut noise());
        assert!(
            !spectrum.ready() && !spectrum.long_ready(),
            "a hole is never spliced over"
        );
        assert_eq!(spectrum.gaps(), 1);

        let before = spectrum.gaps();
        let mut other = Block {
            seq,
            generation: 7,
            sample_rate: 44_100.0,
            channels: 2,
            pre: [[0.5; BLOCK_FRAMES]; 2],
            post: [[0.5; BLOCK_FRAMES]; 2],
            key: [[0.0; BLOCK_FRAMES]; 2],
        };
        spectrum.ingest(&other);
        assert_eq!(
            spectrum.gaps(),
            before,
            "a block at another rate is not this spectrum's"
        );
        other.sample_rate = 48_000.0;
        spectrum.ingest(&other);
        assert_eq!(spectrum.gaps(), before + 1, "a new generation is a break");
    }

    #[test]
    fn a_fall_runs_at_the_chosen_speed_and_a_rise_shows_at_once() {
        let mut spectrum = Spectrum::new(48_000.0);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        let settings = ViewSettings {
            speed: Speed::Slow,
            ..flat()
        };
        feed(&mut spectrum, &mut seq, 0.5, &mut |n| {
            (std::f32::consts::TAU * 1_000.0 * n as f32 / rate).sin()
        });
        spectrum.analyse(&[1_000.0], 0.0, &settings);
        let loud = spectrum.pre_db[0];
        feed(&mut spectrum, &mut seq, 0.5, &mut |_| 0.0);
        spectrum.analyse(&[1_000.0], 0.1, &settings);
        assert!(
            (loud - spectrum.pre_db[0] - 1.2).abs() < 0.01,
            "12 dB/s for 0.1 s"
        );
    }

    /// **With no audio the whole display falls**, the long window's lows included (the owner,
    /// 2026-10-14, in MXM Player: the part under 250 Hz "did not sink down and dissapear"), and the
    /// next audio starts the windows again rather than splicing onto what came before the silence.
    #[test]
    fn with_no_audio_the_whole_display_falls_and_the_next_audio_restarts_the_windows() {
        let mut spectrum = Spectrum::new(48_000.0);
        spectrum.set_resolution(Resolution::High);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        let settings = flat();
        feed(&mut spectrum, &mut seq, 2.0, &mut |n| {
            (std::f32::consts::TAU * 60.0 * n as f32 / rate).sin()
        });
        let freqs = [60.0, 2_000.0];
        spectrum.analyse(&freqs, 0.0, &settings);
        assert!(spectrum.long_ready());
        assert!(spectrum.pre_db[0] > -6.0, "the tone, from the long window");
        // The host stops calling the plugin: the editor's frames go on, and no audio comes.
        for frame in 1..=600 {
            spectrum.analyse(&freqs, f64::from(frame) / 60.0, &settings);
        }
        assert_eq!(spectrum.pre_db, [FLOOR_DB; 2], "pre fell to the floor");
        assert_eq!(spectrum.post_db, [FLOOR_DB; 2], "post fell to the floor");
        feed(&mut spectrum, &mut seq, 0.05, &mut |_| 0.0);
        assert!(
            !spectrum.ready() && !spectrum.long_ready(),
            "restarted, not spliced"
        );
    }

    /// **Audio that ends a silence restarts the windows**, even when it arrives before any
    /// analysis has seen the silence: the frame observes its time before it ingests.
    #[test]
    fn audio_ending_a_silence_restarts_the_windows_before_any_analysis_sees_it() {
        let mut spectrum = Spectrum::new(48_000.0);
        spectrum.set_resolution(Resolution::High);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        feed(&mut spectrum, &mut seq, 2.0, &mut |n| {
            (std::f32::consts::TAU * 60.0 * n as f32 / rate).sin()
        });
        spectrum.analyse(&[60.0], 0.0, &flat());
        assert!(spectrum.long_ready());
        // The next frame is 0.31 s on, and its first block came with it.
        spectrum.observe(0.31);
        feed(&mut spectrum, &mut seq, 0.01, &mut |_| 0.0);
        assert!(
            !spectrum.ready() && !spectrum.long_ready(),
            "restarted, not spliced"
        );
        // Within the threshold nothing restarts.
        feed(&mut spectrum, &mut seq, 2.0, &mut |_| 0.0);
        spectrum.analyse(&[60.0], 1.0, &flat());
        spectrum.observe(1.2);
        feed(&mut spectrum, &mut seq, 0.01, &mut |_| 0.0);
        assert!(spectrum.long_ready(), "0.2 s is no silence");
    }

    #[test]
    fn only_the_shown_streams_are_transformed() {
        let mut spectrum = Spectrum::new(48_000.0);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        feed(&mut spectrum, &mut seq, 0.5, &mut |n| {
            (std::f32::consts::TAU * 1_000.0 * n as f32 / rate).sin()
        });
        let post_only = ViewSettings {
            show: Show::Post,
            ..flat()
        };
        spectrum.analyse(&[1_000.0], 0.0, &post_only);
        assert_eq!(spectrum.pre_db[0], FLOOR_DB, "pre is never computed");
        assert!(spectrum.post_db[0] > -7.0);
    }

    #[test]
    fn a_settled_frame_allocates_nothing() {
        let mut spectrum = Spectrum::new(48_000.0);
        let both = ViewSettings {
            show: Show::Both,
            resolution: Resolution::High,
            ..flat()
        };
        spectrum.set_resolution(both.resolution);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        feed(&mut spectrum, &mut seq, 2.0, &mut |n| {
            (std::f32::consts::TAU * 100.0 * n as f32 / rate).sin()
        });
        let freqs: Vec<f32> = (0..200).map(|k| 20.0 * 1.035f32.powi(k)).collect();
        spectrum.analyse(&freqs, 0.0, &both);
        // Enough new audio for both windows to transform again, then a frame.
        feed(&mut spectrum, &mut seq, 1.5, &mut |n| {
            (std::f32::consts::TAU * 100.0 * n as f32 / rate).sin()
        });
        nice_assert_no_alloc::assert_no_alloc(|| spectrum.analyse(&freqs, 1.0 / 60.0, &both));
    }

    #[test]
    fn a_stream_hidden_and_shown_inside_one_hop_is_analysed_again() {
        let mut spectrum = Spectrum::new(48_000.0);
        let both = ViewSettings {
            show: Show::Both,
            resolution: Resolution::High,
            ..flat()
        };
        spectrum.set_resolution(both.resolution);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        let mut tone = |n: usize| (std::f32::consts::TAU * 60.0 * n as f32 / rate).sin();
        feed(&mut spectrum, &mut seq, 2.0, &mut tone);
        spectrum.analyse(&[60.0], 0.0, &both);
        let post = ViewSettings {
            show: Show::Post,
            ..both
        };
        feed(&mut spectrum, &mut seq, 0.02, &mut tone);
        spectrum.analyse(&[60.0], 1.0 / 60.0, &post);
        let long = |spectrum: &Spectrum| spectrum.long.as_ref().unwrap().window.analysed;
        assert!(!long(&spectrum)[0], "hidden: its sums fall behind");
        spectrum.analyse(&[60.0], 2.0 / 60.0, &both);
        assert!(long(&spectrum)[0], "shown again: analysed at once");
    }

    #[test]
    fn one_bad_sample_costs_the_low_bands_a_window_not_for_good() {
        let mut spectrum = Spectrum::new(48_000.0);
        let high = ViewSettings {
            resolution: Resolution::High,
            ..flat()
        };
        spectrum.set_resolution(high.resolution);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        feed(&mut spectrum, &mut seq, 3.0, &mut |n| {
            if n == 10 {
                f32::NAN
            } else {
                (std::f32::consts::TAU * 60.0 * n as f32 / rate).sin()
            }
        });
        spectrum.analyse(&[60.0], 0.0, &high);
        assert!(spectrum.pre_db[0] > -10.0, "{}", spectrum.pre_db[0]);
    }

    #[test]
    fn a_stream_show_reveals_is_drawn_without_new_audio() {
        let mut spectrum = Spectrum::new(48_000.0);
        let mut seq = 0;
        let rate = spectrum.sample_rate();
        feed(&mut spectrum, &mut seq, 0.5, &mut |n| {
            (std::f32::consts::TAU * 1_000.0 * n as f32 / rate).sin()
        });
        let post_only = ViewSettings {
            show: Show::Post,
            ..flat()
        };
        spectrum.analyse(&[1_000.0], 0.0, &post_only);
        let pre_only = ViewSettings {
            show: Show::Pre,
            ..flat()
        };
        spectrum.analyse(&[1_000.0], 1.0 / 60.0, &pre_only);
        assert!(spectrum.pre_db[0] > -7.0, "{}", spectrum.pre_db[0]);
    }
    /// **The side chain is a third reading** (plan §7.4): a key's tone reads where it is while the
    /// Side chain view is on, and the key stays at the floor while it is off.
    #[test]
    fn the_key_is_read_only_while_the_side_chain_view_is_on() {
        let mut spectrum = Spectrum::new(48_000.0);
        let rate = spectrum.sample_rate();
        let blocks = (rate * 2.0) as usize / BLOCK_FRAMES;
        let freq = 500.0f32;
        let feed_key = |spectrum: &mut Spectrum, from: usize| {
            for seq in from..from + blocks {
                let mut block = Block {
                    seq: seq as u64,
                    generation: 0,
                    sample_rate: rate,
                    channels: 2,
                    pre: [[0.0; BLOCK_FRAMES]; 2],
                    post: [[0.0; BLOCK_FRAMES]; 2],
                    key: [[0.0; BLOCK_FRAMES]; 2],
                };
                for frame in 0..BLOCK_FRAMES {
                    let n = seq * BLOCK_FRAMES + frame;
                    let value = (std::f32::consts::TAU * freq * n as f32 / rate).sin();
                    block.key[0][frame] = value;
                    block.key[1][frame] = value;
                }
                spectrum.ingest(&block);
            }
        };
        feed_key(&mut spectrum, 0);
        spectrum.analyse(&[freq], 0.0, &flat());
        assert_eq!(spectrum.key_db[0], FLOOR_DB, "the view off reads no key");
        let on = ViewSettings {
            side_chain: true,
            ..flat()
        };
        feed_key(&mut spectrum, blocks);
        spectrum.analyse(&[freq], 0.1, &on);
        assert!(
            spectrum.key_db[0].abs() < 0.5,
            "the key's tone: {}",
            spectrum.key_db[0]
        );
        assert_eq!(
            spectrum.pre_db[0], FLOOR_DB,
            "and the silent input reads none"
        );
    }
}
