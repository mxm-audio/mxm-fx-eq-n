//! The match against plan §7.6: what it proposes for a known difference, what it leaves alone,
//! how it counts the EQ, keeps the level and orders its bands.

use std::sync::Arc;

use super::*;
use mxm_fx_eq_n_dsp::Placement;
use mxm_fx_eq_n_dsp::filter::Filter;

const RATE: f32 = 48_000.0;

/// White noise in [−1, 1] (xorshift).
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// The EQ's own bands in series on one channel, matched to their analogue curves.
struct Chain(Vec<Filter>);

impl Chain {
    fn new(cascades: &[Cascade]) -> Chain {
        Chain(cascades.iter().map(Filter::fresh).collect())
    }

    fn tick(&mut self, x: f32) -> f32 {
        let mut y = f64::from(x);
        for filter in &mut self.0 {
            [y] = filter.process([y]);
        }
        y as f32
    }
}

fn cascades(bands: &[MatchBand]) -> Vec<Cascade> {
    bands
        .iter()
        .map(|band| band.cascade(f64::from(RATE)))
        .collect()
}

/// `block` learned: its input before the EQ the track's, its side chain as the output, the
/// reference.
fn hear(matcher: &mut Matcher, block: &Block) {
    matcher.ingest_streams(block, &block.pre, &block.key);
}

/// A matcher that heard `seconds` of what `next` gives, sample by sample: the input's left and
/// right, then the side chain's.
fn heard_for(seconds: f32, mut next: impl FnMut() -> ([f32; 2], [f32; 2])) -> Matcher {
    let mut matcher = Matcher::new(RATE);
    let blocks = (RATE * seconds) as usize / BLOCK_FRAMES;
    for seq in 0..blocks as u64 {
        let mut block = Block {
            seq,
            generation: 0,
            sample_rate: RATE,
            channels: 2,
            pre: [[0.0; BLOCK_FRAMES]; 2],
            post: [[0.0; BLOCK_FRAMES]; 2],
            key: [[0.0; BLOCK_FRAMES]; 2],
        };
        for index in 0..BLOCK_FRAMES {
            let (pre, key) = next();
            for channel in 0..2 {
                block.pre[channel][index] = pre[channel];
                block.key[channel][index] = key[channel];
            }
        }
        hear(&mut matcher, &block);
    }
    matcher
}

/// The plan's known EQ: a +4 dB low shelf at 120 Hz, a −3 dB bell at 800 Hz Q 1.4, a +3 dB bell at
/// 3 kHz Q 1, a −2 dB high shelf at 9 kHz.
fn known_eq() -> Vec<MatchBand> {
    let band = |shape, freq, gain_db, q| MatchBand {
        shape,
        freq,
        gain_db,
        q,
    };
    vec![
        band(Shape::LowShelf, 120.0, 4.0, SHELF_Q),
        band(Shape::Bell, 800.0, -3.0, 1.4),
        band(Shape::Bell, 3_000.0, 3.0, 1.0),
        band(Shape::HighShelf, 9_000.0, -2.0, SHELF_Q),
    ]
}

/// The fixture (plan §7.6): the reference the noise through the known EQ, `gain` louder, the
/// source the same noise.
fn fixture(seconds: f32, gain: f32) -> Matcher {
    let mut noise = Noise(0x2545_f491);
    let mut chain = Chain::new(&cascades(&known_eq()));
    heard_for(seconds, move || {
        let x = 0.3 * noise.next();
        let y = gain * chain.tick(x);
        ([x, x], [y, y])
    })
}

fn flat() -> HeardEq {
    HeardEq {
        bands: Vec::new(),
        sample_rate: f64::from(RATE),
        deepest_db: 30.0,
    }
}

/// The EQ as heard holding `bands`, every one Stereo.
fn holding(bands: &[MatchBand]) -> HeardEq {
    HeardEq {
        bands: cascades(bands)
            .into_iter()
            .map(|cascade| (cascade, Placement::Stereo))
            .collect(),
        ..flat()
    }
}

struct Fitted {
    bands: Vec<MatchBand>,
    target: Vec<f32>,
    points: Vec<f32>,
    cells: Vec<Option<CellCurve>>,
}

fn fitted(matcher: &Matcher, heard: &HeardEq, amount: f32, max_bands: usize) -> Fitted {
    let cells = curves(&matcher.learned(), &matcher.reference(), heard);
    let level = level_offset(&cells, amount);
    let points = points(heard.sample_rate);
    let target = target(&cells, &points, amount, level);
    let bands = fit(
        &target,
        &weights(&points, &cells),
        &points,
        heard.sample_rate,
        max_bands,
        heard.deepest_db,
    );
    Fitted {
        bands,
        target,
        points,
        cells,
    }
}

/// What is left of the target once `bands` are in, weighted as the fit reads it.
fn left_after(fitted: &Fitted) -> Vec<f32> {
    let responses: Vec<Vec<f64>> = fitted
        .bands
        .iter()
        .map(|band| band.responses(&fitted.points, f64::from(RATE)))
        .collect();
    left_weighted(
        &fitted.target,
        &weights(&fitted.points, &fitted.cells),
        &responses,
    )
}

fn octaves(a: f32, b: f32) -> f32 {
    (a / b).log2().abs()
}

/// **A known difference is matched** (plan §7.6): at 100 % and at 40 % nothing left is just
/// audible, and at 100 % each of the four is a band of its sign, the shelves shelves, at most eight
/// bands in all.
#[test]
fn a_known_difference_is_matched() {
    // Long enough for the lows to settle, the low shelf's among them.
    let matcher = fixture(30.0, 1.0);
    assert!(matcher.learned().seconds > 0.0);
    for amount in [1.0, 0.4] {
        let fit = fitted(&matcher, &flat(), amount, 24);
        let left = left_after(&fit);
        let audible: Vec<Region> = regions(&left, &fit.points, MAX_GAIN_DB)
            .into_iter()
            .filter(|region| region.counts)
            .collect();
        assert!(
            audible.is_empty(),
            "{amount}: {audible:#?}\n{:#?}",
            fit.bands
        );
        if amount == 1.0 {
            assert!(fit.bands.len() <= 8, "{:#?}", fit.bands);
            for want in known_eq() {
                assert!(
                    fit.bands.iter().any(|band| band.shape == want.shape
                        && band.gain_db.signum() == want.gain_db.signum()
                        && octaves(band.freq, want.freq) <= 1.0 / 6.0),
                    "{want:?} in {:#?}",
                    fit.bands
                );
            }
        }
    }
}

/// **The level comes out** (plan §7.6): a reference 10 dB louder proposes the same, and the
/// source's K-weighted power through the proposals is what it was, at 40 % and at 100 %.
#[test]
fn the_level_comes_out_and_the_loudness_is_kept() {
    let matcher = fixture(12.0, 1.0);
    let louder = fixture(12.0, 10f32.powf(10.0 / 20.0));
    for amount in [1.0, 0.4] {
        let fit = fitted(&matcher, &flat(), amount, 24);
        let loud = fitted(&louder, &flat(), amount, 24);
        assert_eq!(fit.bands.len(), loud.bands.len(), "{amount}");
        for (a, b) in fit.bands.iter().zip(&loud.bands) {
            assert!(
                (a.gain_db - b.gain_db).abs() <= 0.5 && octaves(a.freq, b.freq) <= 1.0 / 48.0,
                "{a:?} {b:?}"
            );
        }
        let (mut before, mut after) = (0.0f64, 0.0f64);
        for (cell, curve) in fit.cells.iter().enumerate() {
            let Some(curve) = curve else {
                continue;
            };
            let freq = cell_freq(cell);
            if ease(freq) <= 0.0 {
                continue;
            }
            let power = k_weight(f64::from(freq)) * curve.power;
            let through: f64 = fit
                .bands
                .iter()
                .map(|band| {
                    band.cascade(f64::from(RATE))
                        .response_db(f64::from(freq), f64::from(RATE))
                })
                .sum();
            before += power;
            after += power * 10f64.powf(through / 10.0);
        }
        let change = 10.0 * (after / before).log10();
        assert!(change.abs() <= 0.5, "{amount}: {change} dB");
    }
}

/// **The EQ is counted as heard** (plan §7.6): with a Mid band and a Left band in it, on a stereo
/// source whose channels are alike in part, the source's level through the EQ is the level of
/// the output rendered through those bands, within 0.5 dB.
#[test]
fn a_mid_band_and_a_left_band_are_counted_as_heard() {
    let rate = f64::from(RATE);
    let bell = |freq: f64, gain_db: f64| {
        design(
            Design {
                shape: Shape::Bell,
                slope: Slope::Db12,
                freq,
                gain_db,
                q: 1.0,
            },
            rate,
        )
    };
    let (mid, left) = (bell(1_000.0, 6.0), bell(4_000.0, -6.0));
    let heard_eq = HeardEq {
        bands: vec![(left, Placement::Left), (mid, Placement::Mid)],
        ..flat()
    };
    let input = |seed: u32| {
        let (mut a, mut b) = (Noise(seed), Noise(seed.rotate_left(7) ^ 0x9e37_79b9));
        move || {
            let (x, y) = (0.3 * a.next(), 0.3 * b.next());
            [x, 0.6 * x + 0.8 * y]
        }
    };
    let mut source = input(0x1234_5678);
    let through_eq = heard_for(12.0, move || {
        let pre = source();
        (pre, pre)
    });
    let (mut left_chain, mut mid_chain) = (Chain::new(&[left]), Chain::new(&[mid]));
    let mut source = input(0x1234_5678);
    let rendered = heard_for(12.0, move || {
        let [l, r] = source();
        // The channel stage, then the mid/side stage (plan §7.3).
        let l = left_chain.tick(l);
        let (m, s) = (mid_chain.tick(0.5 * (l + r)), 0.5 * (l - r));
        let out = [m + s, m - s];
        (out, out)
    });
    let expected = levels(&through_eq.learned(), &through_eq.reference(), &heard_eq);
    let heard = levels(&rendered.learned(), &rendered.reference(), &flat());
    for cell in 0..CELLS {
        let freq = cell_freq(cell);
        if !(RANGE.0..=RANGE.1).contains(&freq) {
            continue;
        }
        let (Some(want), Some(got)) = (expected.through[cell], heard.source[cell]) else {
            continue;
        };
        assert!((want - got).abs() <= 0.5, "{freq} Hz: {want} against {got}");
    }
}

/// **A key's notes are not matched** (plan §7.6): a difference that is a comb of lines a semitone
/// apart, 12 dB over a flat floor at any offset, is smoothed flat within 0.25 dB from 30 Hz up.
#[test]
fn a_comb_of_notes_is_smoothed_flat() {
    let first = (CELLS_PER_OCTAVE * (RANGE.0 / LOWEST_HZ).log2()).ceil() as usize;
    let last = (CELLS_PER_OCTAVE * (RANGE.1 / LOWEST_HZ).log2()).floor() as usize;
    for offset in 0..4 {
        let comb: Vec<Option<f32>> = (0..CELLS)
            .map(|cell| Some(if (cell + offset) % 4 == 0 { 12.0 } else { 0.0 }))
            .collect();
        let smoothed = smooth(&comb);
        for (cell, value) in smoothed.iter().enumerate().take(last + 1).skip(first) {
            let value = value.expect("every cell heard");
            assert!(
                (value - 3.0).abs() <= 0.25,
                "offset {offset}, {} Hz: {value}",
                cell_freq(cell)
            );
        }
    }
}

/// **What the EQ already does is not proposed again** (plan §7.6): with the known EQ in it the
/// match proposes nothing, and after its own bands are applied nothing is left.
#[test]
fn what_the_eq_already_does_is_not_proposed_again() {
    let matcher = fixture(12.0, 1.0);
    let carried = fitted(&matcher, &holding(&known_eq()), 1.0, 24);
    assert!(carried.bands.is_empty(), "{:#?}", carried.bands);
    let first = fitted(&matcher, &flat(), 1.0, 24);
    assert!(!first.bands.is_empty());
    let again = fitted(&matcher, &holding(&first.bands), 1.0, 24);
    assert!(again.bands.is_empty(), "{:#?}", again.bands);
}

/// **Fewer slots, or a lower Detail, keep the first bands** (plan §7.6): with three, exactly the
/// unbounded fit's state after its third band.
#[test]
fn three_bands_are_the_unbounded_fits_first_three() {
    let matcher = fixture(12.0, 1.0);
    let cells = curves(&matcher.learned(), &matcher.reference(), &flat());
    let level = level_offset(&cells, 1.0);
    let points = points(f64::from(RATE));
    let target = target(&cells, &points, 1.0, level);
    let weights = weights(&points, &cells);
    let mut third = None;
    let rate = f64::from(RATE);
    let all = fit_each(&target, &weights, &points, rate, 24, 30.0, |bands| {
        if bands.len() == 3 {
            third = Some(bands.to_vec());
        }
    });
    assert!(all.len() > 3, "{all:#?}");
    let three = fit(&target, &weights, &points, rate, 3, 30.0);
    assert_eq!(Some(three), third);
}

/// **A short learn matches only what has settled, and silence counts for nothing** (plan §7.6):
/// three seconds of the known difference settle the highs and not the lows, so the 3 kHz bell is
/// proposed and nothing under 300 Hz; a silent stretch in the side chain changes nothing.
#[test]
fn a_short_learn_matches_only_what_settled_and_silence_counts_for_nothing() {
    let early = fixture(3.0, 1.0);
    let fit = Fit {
        track: Arc::new(early.learned()),
        reference: early.reference(),
        heard: flat(),
        amount: 1.0,
        max_bands: 24,
        listen: 0,
        basis: Vec::new(),
    };
    let bands = fit.run().bands;
    assert!(
        bands.iter().all(|band| band.freq > 300.0),
        "nothing where it has not settled: {bands:#?}"
    );
    assert!(
        bands
            .iter()
            .any(|band| octaves(band.freq, 3_000.0) <= 1.0 / 6.0 && band.gain_db > 0.0),
        "what has: {bands:#?}"
    );

    let plain = fixture(16.0, 1.0);
    let mut noise = Noise(0x2545_f491);
    let mut chain = Chain::new(&cascades(&known_eq()));
    let mut sample = 0usize;
    let gapped = heard_for(16.0, move || {
        let x = 0.3 * noise.next();
        let y = chain.tick(x);
        sample += 1;
        let silent = (5 * RATE as usize..8 * RATE as usize).contains(&sample);
        ([x, x], if silent { [0.0; 2] } else { [y, y] })
    });
    // The same match: the two fits' responses within 0.5 dB over the range. A frame straddling
    // the silence mixes it in a little differently on rungs of different lengths, which a longer
    // listen dilutes; the bands' own numbers may differ inside that.
    let response = |fit: &Fitted| -> Vec<f64> {
        let responses: Vec<Vec<f64>> = fit
            .bands
            .iter()
            .map(|band| band.responses(&fit.points, f64::from(RATE)))
            .collect();
        (0..fit.points.len())
            .map(|at| responses.iter().map(|r| r[at]).sum())
            .collect()
    };
    let (a, b) = (
        fitted(&plain, &flat(), 1.0, 24),
        fitted(&gapped, &flat(), 1.0, 24),
    );
    for (at, (x, y)) in response(&a).iter().zip(response(&b)).enumerate() {
        let freq = a.points[at];
        if (RANGE.0..=RANGE.1).contains(&freq) {
            assert!(
                (x - y).abs() <= 0.5,
                "{freq} Hz: {x} against {y}\n{:#?}\n{:#?}",
                a.bands,
                b.bands
            );
        }
    }
}

/// **Quiet frames still count** (plan §7.6): after the ten seconds, frames below −60 dBFS change
/// the long-term means and are something new, so the match is fitted again; they add no seconds
/// heard.
#[test]
fn quiet_frames_change_the_means_and_ask_a_new_fit() {
    let mut matcher = fixture(12.0, 1.0);
    let (revision, heard) = (matcher.revision(), matcher.heard_seconds());
    let before = curves(&matcher.learned(), &matcher.reference(), &flat());
    let mut noise = Noise(99);
    let mut chain = Chain::new(&cascades(&[MatchBand {
        shape: Shape::Bell,
        freq: 1_000.0,
        gain_db: 12.0,
        q: 1.0,
    }]));
    let quiet = 1.0e-4;
    for seq in 0..(RATE * 4.0) as u64 / BLOCK_FRAMES as u64 {
        let mut block = Block {
            seq: seq + 1_000_000,
            generation: 0,
            sample_rate: RATE,
            channels: 2,
            pre: [[0.0; BLOCK_FRAMES]; 2],
            post: [[0.0; BLOCK_FRAMES]; 2],
            key: [[0.0; BLOCK_FRAMES]; 2],
        };
        for index in 0..BLOCK_FRAMES {
            let x = quiet * noise.next();
            let y = chain.tick(x);
            for channel in 0..2 {
                block.pre[channel][index] = x;
                block.key[channel][index] = y;
            }
        }
        hear(&mut matcher, &block);
    }
    assert_ne!(matcher.revision(), revision, "something new");
    assert_eq!(matcher.heard_seconds(), heard, "no seconds heard");
    let after = curves(&matcher.learned(), &matcher.reference(), &flat());
    assert_ne!(before, after, "the means moved");
}

/// **A reset keeps what was learned, and Learn on again starts anew** (plan §7.6): a new
/// generation, as Resolve's reset from clip to clip, restarts the frames and keeps the sums and
/// the seconds; a restart forgets the input and the output alike.
#[test]
fn a_reset_keeps_what_was_learned_and_a_restart_forgets_it() {
    let mut noise = Noise(7);
    let mut matcher = Matcher::new(RATE);
    for seq in 0..(RATE * 12.0) as u64 / BLOCK_FRAMES as u64 {
        let mut block = Block {
            seq,
            // A reset every second.
            generation: (seq * BLOCK_FRAMES as u64 / RATE as u64) as u32,
            sample_rate: RATE,
            channels: 2,
            pre: [[0.0; BLOCK_FRAMES]; 2],
            post: [[0.0; BLOCK_FRAMES]; 2],
            key: [[0.0; BLOCK_FRAMES]; 2],
        };
        for index in 0..BLOCK_FRAMES {
            let x = 0.3 * noise.next();
            block.pre[0][index] = x;
            block.pre[1][index] = x;
            block.key[0][index] = x;
            block.key[1][index] = x;
        }
        hear(&mut matcher, &block);
    }
    let heard = matcher.heard_seconds();
    assert!(heard > 10.0, "{heard} s");
    assert!(matcher.learned().seconds > 10.0 && matcher.reference().seconds > 10.0);
    matcher.restart();
    assert_eq!(matcher.heard_seconds(), 0.0);
    let learned = matcher.learned();
    assert!(
        learned.input.iter().all(Option::is_none) && learned.output.iter().all(Option::is_none),
        "anew"
    );
}

/// **A reference does not depend on the rate it was learned at** (plan §7.6): its cells are power
/// per hertz, so a sine's power summed over the cells around it reads the same learned at 44.1 kHz
/// and at 96 kHz, on every rung: within 0.2 dB at 1 and 8 kHz, and within 0.5 dB at 100 Hz, where a
/// cell is four or five of the lowest rung's bins and its width is read to a bin (the match smooths
/// over a third of an octave or more).
#[test]
fn a_reference_learned_at_another_rate_reads_the_same() {
    let power_near = |rate: f32, freq: f32| -> f32 {
        let mut matcher = Matcher::new(rate);
        let step = std::f64::consts::TAU * f64::from(freq) / f64::from(rate);
        let mut n = 0u64;
        for seq in 0..(rate * 8.0) as u64 / BLOCK_FRAMES as u64 {
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
                let x = (0.3 * (step * n as f64).sin()) as f32;
                n += 1;
                block.key[0][index] = x;
                block.key[1][index] = x;
            }
            hear(&mut matcher, &block);
        }
        let reference = matcher.reference();
        let centre = (CELLS_PER_OCTAVE * (freq / LOWEST_HZ).log2()).round() as usize;
        let edge = 2f64.powf(1.0 / (2.0 * f64::from(CELLS_PER_OCTAVE)));
        let power: f64 = (centre - 3..=centre + 3)
            .filter_map(|cell| {
                let db = reference.cells[cell]?;
                let width = f64::from(cell_freq(cell)) * (edge - edge.recip());
                Some(10f64.powf(f64::from(db) / 10.0) * width)
            })
            .sum();
        (10.0 * power.log10()) as f32
    };
    for (freq, within) in [(100.0, 0.5), (1_000.0, 0.2), (8_000.0, 0.2)] {
        let (low, high) = (power_near(44_100.0, freq), power_near(96_000.0, freq));
        assert!(
            (low - high).abs() <= within,
            "{freq} Hz: {low} dB at 44.1 kHz, {high} dB at 96 kHz"
        );
    }
}

/// **Where the reference was not read, nothing is asked** (plan §7.6): its cells above what its
/// rate read weigh nothing in the fit.
#[test]
fn an_unread_reference_cell_weighs_nothing() {
    let matcher = fixture(12.0, 1.0);
    let mut reference = matcher.reference();
    let cut = (0..CELLS)
        .find(|&cell| cell_freq(cell) > 8_000.0)
        .expect("a cell over 8 kHz");
    for cell in &mut reference.cells[cut..] {
        *cell = None;
    }
    let cells = curves(&matcher.learned(), &reference, &flat());
    let points = points(f64::from(RATE));
    let weights = weights(&points, &cells);
    for (freq, weight) in points.iter().zip(&weights) {
        if *freq > 8_100.0 {
            assert_eq!(*weight, 0.0, "{freq} Hz");
        } else if (1_000.0..=8_000.0).contains(freq) {
            assert_eq!(*weight, 1.0, "{freq} Hz, settled there");
        }
    }
}

/// **A learn reads the level of what played, its pauses aside** (plan §7.6): noise heard two
/// seconds on and two off reads, smoothed, within 0.5 dB of the same noise heard throughout, the
/// track's and the reference's alike, from the lows' long frames to the highs' short ones.
#[test]
fn a_learn_reads_the_level_of_what_played_its_pauses_aside() {
    let learned = |paused: bool| {
        let mut noise = Noise(11);
        let mut sample = 0u64;
        let seconds = if paused { 22.0 } else { 11.0 };
        heard_for(seconds, move || {
            let silent = paused && (sample / (2 * RATE as u64)) % 2 == 1;
            sample += 1;
            let x = if silent { 0.0 } else { 0.3 * noise.next() };
            ([x, x], [x, x])
        })
    };
    let (steady, paused) = (learned(false), learned(true));
    let lines = |matcher: &Matcher| {
        [
            smooth(&matcher.reference().cells),
            smooth(&matcher.learned().input_levels()),
        ]
    };
    let (steady, paused) = (lines(&steady), lines(&paused));
    for freq in [60.0, 1_000.0, 8_000.0] {
        for learn in 0..2 {
            let at = |lines: &[Vec<Option<f32>>; 2]| cell_at(&lines[learn], freq).expect("read");
            let (steady, paused) = (at(&steady), at(&paused));
            assert!(
                (steady - paused).abs() <= 0.5,
                "{freq} Hz, learn {learn}: {paused} dB against {steady} dB"
            );
        }
    }
}

/// **A fit is deterministic** (plan §7.6): the same spectra give the same bands, bit for bit.
#[test]
fn a_fit_is_deterministic() {
    let matcher = fixture(12.0, 1.0);
    let run = || {
        Fit {
            track: Arc::new(matcher.learned()),
            reference: matcher.reference(),
            heard: flat(),
            amount: 0.4,
            max_bands: 8,
            listen: 3,
            basis: vec![1, 2],
        }
        .run()
    };
    let (a, b) = (run(), run());
    assert_eq!(a.bands, b.bands);
    assert_eq!((a.listen, a.basis), (3, vec![1, 2]));
}

/// **The regions' edges** (plan §7.6): a zero point splits a run; a region reaching the low end
/// seeds a low shelf, the high end a high shelf, both ends the shelf on its extreme's nearer side;
/// a region in the middle a bell at its extreme; of two equal areas, the lower is taken.
#[test]
fn the_regions_edges() {
    let points = points(f64::from(RATE));
    let at = |freq: f32| {
        points
            .iter()
            .position(|&point| point >= freq)
            .expect("inside")
    };
    let bump = |centre: f32, gain: f32, width: f32| -> Vec<f32> {
        points
            .iter()
            .map(|&freq| {
                let x = (freq / centre).log2() / width;
                gain * (-x * x).exp()
            })
            .collect()
    };

    // A zero point splits a run.
    let mut split = bump(1_000.0, 4.0, 1.0);
    split[at(1_000.0)] = 0.0;
    let found = regions(&split, &points, 12.0);
    assert!(found.len() >= 2, "{found:#?}");

    // The ends.
    let low = bump(25.0, 4.0, 1.0);
    let seeded = regions(&low, &points, 12.0)[0].seed;
    assert_eq!(seeded.shape, Shape::LowShelf, "{seeded:?}");
    let high = bump(20_000.0, -4.0, 1.0);
    let seeded = regions(&high, &points, 12.0)[0].seed;
    assert_eq!(seeded.shape, Shape::HighShelf, "{seeded:?}");
    let rising: Vec<f32> = points
        .iter()
        .map(|&freq| 0.5 + 0.3 * (freq / 20.0).log2())
        .collect();
    let seeded = regions(&rising, &points, 12.0)[0].seed;
    assert_eq!(
        seeded.shape,
        Shape::HighShelf,
        "both ends, its extreme nearer the top: {seeded:?}"
    );

    // A bell in the middle, at its extreme, its Q from its width at half height.
    let middle = bump(1_000.0, -5.0, 0.5);
    let region = regions(&middle, &points, 12.0)
        .into_iter()
        .find(|region| region.counts)
        .expect("a region");
    assert_eq!(region.seed.shape, Shape::Bell);
    assert!(
        octaves(region.seed.freq, 1_000.0) <= 1.0 / 24.0,
        "{region:?}"
    );
    assert!((region.seed.gain_db + 5.0).abs() < 0.05, "{region:?}");

    // Of two equal areas, the lower.
    let pair: Vec<f32> = bump(300.0, 3.0, 0.3)
        .iter()
        .zip(bump(3_000.0, 3.0, 0.3))
        .map(|(a, b)| a + b)
        .collect();
    let eased: Vec<f32> = points.iter().map(|&freq| ease(freq)).collect();
    let bands = fit(&pair, &eased, &points, f64::from(RATE), 1, 12.0);
    assert!(octaves(bands[0].freq, 300.0) <= 1.0 / 12.0, "{bands:?}");

    // A seed's gain is held to the bound at once.
    let tall = bump(1_000.0, 20.0, 0.5);
    assert_eq!(regions(&tall, &points, 12.0)[0].seed.gain_db, 12.0);
}

/// **The match's listening stays inside the analyser's budget** (plan §7.6): at 192 kHz in
/// release, per 1/60 s of audio on the GUI thread, and a fit's time on the worker.
#[test]
#[ignore = "a measurement; run in release"]
fn match_cost() {
    let rate = 192_000.0f32;
    let mut matcher = Matcher::new(rate);
    let mut noise = Noise(7);
    // The side chain the noise through the known EQ, so the fit has something to fit.
    let mut chain = Chain::new(
        &known_eq()
            .iter()
            .map(|band| band.cascade(f64::from(rate)))
            .collect::<Vec<_>>(),
    );
    let blocks_per_frame = (rate / 60.0 / BLOCK_FRAMES as f32).ceil() as usize;
    let mut seq = 0u64;
    let (mut total, mut worst, mut frames) = (0.0f64, 0.0f64, 0usize);
    for _ in 0..(rate as usize * 20 / BLOCK_FRAMES / blocks_per_frame) {
        let mut blocks = Vec::new();
        for _ in 0..blocks_per_frame {
            let mut block = Block {
                seq,
                generation: 0,
                sample_rate: rate,
                channels: 2,
                pre: [[0.0; BLOCK_FRAMES]; 2],
                post: [[0.0; BLOCK_FRAMES]; 2],
                key: [[0.0; BLOCK_FRAMES]; 2],
            };
            seq += 1;
            for index in 0..BLOCK_FRAMES {
                let x = 0.3 * noise.next();
                let y = chain.tick(x);
                for channel in 0..2 {
                    block.pre[channel][index] = x;
                    block.key[channel][index] = y;
                }
            }
            blocks.push(block);
        }
        let start = std::time::Instant::now();
        for block in &blocks {
            hear(&mut matcher, block);
        }
        let copy = matcher.learned();
        let took = start.elapsed().as_secs_f64() * 1_000.0;
        drop(copy);
        total += took;
        worst = worst.max(took);
        frames += 1;
    }
    let heard_eq = HeardEq {
        bands: Vec::new(),
        sample_rate: f64::from(rate),
        deepest_db: 30.0,
    };
    let start = std::time::Instant::now();
    let fitted = Fit {
        track: Arc::new(matcher.learned()),
        reference: matcher.reference(),
        heard: heard_eq,
        amount: 1.0,
        max_bands: 24,
        listen: 0,
        basis: Vec::new(),
    }
    .run();
    let fit_ms = start.elapsed().as_secs_f64() * 1_000.0;
    println!(
        "match at 192 kHz: on the GUI thread {:.3} ms a frame on average, {:.3} ms at worst; a fit \
         of {} bands {:.1} ms",
        total / frames as f64,
        worst,
        fitted.bands.len(),
        fit_ms
    );
}
