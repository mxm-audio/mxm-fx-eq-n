//! The finder against plan §7.5: what it finds and where it places it, what it leaves alone, and
//! what its proposals do to the peaks they cut.

use super::*;

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

/// The cookbook's peaking biquad (Bristow-Johnson), its Q in the half-gain-in-dB convention the
/// EQ's bell keeps: a resonance of `gain_db` at `freq`, its width at half its height `freq / q`.
#[derive(Clone, Copy)]
struct Resonance {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Resonance {
    fn new(freq: f32, gain_db: f32, q: f32) -> Resonance {
        let a = 10f64.powf(f64::from(gain_db) / 40.0);
        let w = std::f64::consts::TAU * f64::from(freq) / f64::from(RATE);
        let alpha = w.sin() / (2.0 * f64::from(q));
        let a0 = 1.0 + alpha / a;
        Resonance {
            b: [
                (1.0 + alpha * a) / a0,
                -2.0 * w.cos() / a0,
                (1.0 - alpha * a) / a0,
            ],
            a: [-2.0 * w.cos() / a0, (1.0 - alpha / a) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn tick(&mut self, input: f32) -> f32 {
        let x = f64::from(input);
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y as f32
    }

    /// Its magnitude at `freq`, in dB.
    fn db(&self, freq: f32) -> f32 {
        let w = std::f64::consts::TAU * f64::from(freq) / f64::from(RATE);
        let at = |c: [f64; 3]| {
            let re = c[0] + c[1] * w.cos() + c[2] * (2.0 * w).cos();
            let im = -c[1] * w.sin() - c[2] * (2.0 * w).sin();
            re * re + im * im
        };
        (10.0 * (at(self.b) / at([1.0, self.a[0], self.a[1]])).log10()) as f32
    }
}

/// A melody of quarter-second notes, each a saw-like tone of its harmonics at 1/h up to 16 kHz,
/// faded in and out over 20 ms on a raised cosine, as a played note is rather than a switched one.
struct Melody {
    notes: Vec<f32>,
    n: usize,
}

impl Melody {
    fn new(notes: &[f32]) -> Melody {
        Melody {
            notes: notes.to_vec(),
            n: 0,
        }
    }

    /// Twelve pitches over an octave and a half, spaced by the golden ratio rather than tempered,
    /// so their harmonics move with them: a consonant scale shares harmonics between half its
    /// notes, a harmonic series in disguise that no persistence test can tell from a resonance
    /// (research §8.2).
    fn scale() -> Melody {
        let notes: Vec<f32> = (1..=12)
            .map(|k| {
                let place = (k as f32 * 0.618_034).fract();
                196.0 * 2f32.powf(1.4 * place)
            })
            .collect();
        Melody::new(&notes)
    }

    fn next(&mut self) -> f32 {
        let t = self.n as f64 / f64::from(RATE);
        self.n += 1;
        let note = (t / 0.25) as usize % self.notes.len();
        let local = t % 0.25;
        let edge = (local / 0.02).min((0.25 - local) / 0.02).clamp(0.0, 1.0);
        let fade = 0.5 - 0.5 * (std::f64::consts::PI * edge).cos();
        let freq = f64::from(self.notes[note]);
        let mut value = 0.0;
        let mut h = 1.0;
        while freq * h < 16_000.0 {
            value += (std::f64::consts::TAU * freq * h * t).sin() / h;
            h += 1.0;
        }
        (0.1 * fade * value) as f32
    }
}

/// `seconds` of `source` into `finder`, as the analyser's blocks: pre only, `channels` of them.
fn feed(
    finder: &mut Finder,
    seq: &mut u64,
    seconds: f32,
    channels: u8,
    mut source: impl FnMut() -> [f32; 2],
) {
    let blocks = (RATE * seconds) as usize / BLOCK_FRAMES;
    for _ in 0..blocks {
        let mut block = Block {
            seq: *seq,
            generation: 0,
            sample_rate: RATE,
            channels,
            pre: [[0.0; BLOCK_FRAMES]; 2],
            post: [[0.0; BLOCK_FRAMES]; 2],
            key: [[0.0; BLOCK_FRAMES]; 2],
        };
        for index in 0..BLOCK_FRAMES {
            let [left, right] = source();
            block.pre[0][index] = left;
            block.pre[1][index] = right;
        }
        finder.ingest(&block);
        *seq += 1;
    }
}

/// A melody with noise `noise_db` under it, through `resonances`, the same on both channels.
fn melody_through(resonances: &[(f32, f32, f32)], noise_db: f32, seconds: f32) -> Finder {
    let mut finder = Finder::new(RATE);
    let mut melody = Melody::scale();
    let mut noise = Noise(0x2545_f491);
    let gain = 10f32.powf(noise_db / 20.0);
    let mut filters: Vec<Resonance> = resonances
        .iter()
        .map(|&(freq, gain_db, q)| Resonance::new(freq, gain_db, q))
        .collect();
    let mut seq = 0;
    feed(&mut finder, &mut seq, seconds, 2, || {
        let mut x = melody.next() + gain * noise.next();
        for filter in &mut filters {
            x = filter.tick(x);
        }
        [x, x]
    });
    finder
}

fn near(a: f32, b: f32, octaves_apart: f32) -> bool {
    octaves(a, b).abs() <= octaves_apart
}

/// **A fixed resonance under a melody is found, and the melody's harmonics are not** (plan §7.5):
/// a Q-10 resonance at 1 kHz under quarter-second notes, within a forty-eighth of an octave and
/// its Q within 20 %; nothing else is proposed, though every note's harmonics stand in the
/// long-term spectrum.
#[test]
fn a_fixed_resonance_under_a_melody_is_found_and_the_harmonics_are_not() {
    let finder = melody_through(&[(1_000.0, 12.0, 10.0)], -10.0, 12.0);
    assert!(finder.listened());
    let long_term = finder.measured();
    assert!(
        long_term.len() > 8,
        "the harmonics stand in the long-term spectrum: {}",
        long_term.len()
    );
    let candidates = finder.candidates();
    let found: Vec<&Candidate> = candidates
        .iter()
        .filter(|candidate| near(candidate.freq, 1_000.0, 1.0 / 48.0))
        .collect();
    assert_eq!(found.len(), 1, "{candidates:#?}");
    let band = found[0].bands[0];
    assert!(
        (band.q / 10.0 - 1.0).abs() <= 0.2,
        "built Q 10, measured {}",
        band.q
    );
    assert_eq!(
        band.domain,
        Domain::Average,
        "a centred resonance is Stereo"
    );
    assert_eq!(candidates.len(), 1, "only the resonance: {candidates:#?}");
}

/// **A region only one note reaches gives no verdict** (plan §7.5): a lone note's harmonics are
/// not proposed, persistent as they are.
#[test]
fn a_lone_note_s_harmonics_are_not_proposed() {
    let mut finder = Finder::new(RATE);
    let mut note = Melody::new(&[330.0]);
    let mut seq = 0;
    feed(&mut finder, &mut seq, 10.0, 2, || {
        let x = note.next();
        [x, x]
    });
    assert!(finder.listened());
    assert!(finder.candidates().is_empty(), "{:#?}", finder.candidates());
}

/// **Resonances low and high are found** (plan §7.5): 60 Hz at Q 15, on the lowest rung, and 8 kHz
/// at Q 20, on the highest.
#[test]
fn resonances_at_60_hz_and_8_khz_are_found() {
    let finder = melody_through(&[(60.0, 12.0, 15.0), (8_000.0, 12.0, 20.0)], -20.0, 30.0);
    let candidates = finder.candidates();
    for (freq, q) in [(60.0, 15.0), (8_000.0, 20.0)] {
        let found = candidates
            .iter()
            .find(|candidate| near(candidate.freq, freq, 1.0 / 48.0));
        let band = found
            .unwrap_or_else(|| panic!("{freq} Hz: {candidates:#?}"))
            .bands[0];
        assert!(
            (band.q / q - 1.0).abs() <= 0.25,
            "{freq} Hz: built Q {q}, measured {}",
            band.q
        );
    }
}

/// Stereo `left` and `right`, each a melody with noise under it through its own resonances.
fn stereo(
    left: &[(f32, f32, f32)],
    right: &[(f32, f32, f32)],
    side: Option<(f32, f32, f32)>,
) -> Finder {
    let mut finder = Finder::new(RATE);
    let mut melody = Melody::scale();
    let (mut mid_noise, mut side_noise) = (Noise(0x2545_f491), Noise(0x1234_5677));
    let make = |set: &[(f32, f32, f32)]| -> Vec<Resonance> {
        set.iter()
            .map(|&(freq, gain_db, q)| Resonance::new(freq, gain_db, q))
            .collect()
    };
    let (mut lefts, mut rights) = (make(left), make(right));
    let mut sides: Vec<Resonance> = side
        .map(|(freq, gain_db, q)| vec![Resonance::new(freq, gain_db, q)])
        .unwrap_or_default();
    let mut seq = 0;
    feed(&mut finder, &mut seq, 12.0, 2, || {
        let centre = melody.next() + 0.1 * mid_noise.next();
        let mut side = 0.1 * side_noise.next();
        for filter in &mut sides {
            side = filter.tick(side);
        }
        let (mut l, mut r) = (centre + side, centre - side);
        for filter in &mut lefts {
            l = filter.tick(l);
        }
        for filter in &mut rights {
            r = filter.tick(r);
        }
        [l, r]
    });
    finder
}

fn domains_at(finder: &Finder, freq: f32) -> Vec<Domain> {
    let candidates = finder.candidates();
    candidates
        .iter()
        .find(|candidate| near(candidate.freq, freq, 1.0 / 24.0))
        .unwrap_or_else(|| panic!("{freq} Hz: {candidates:#?}"))
        .bands
        .iter()
        .map(|band| band.domain)
        .collect()
}

/// **Placed where the excess is** (plan §7.5): a left-only resonance a Left band, a side-only one a
/// Side band, unequal channels a Left and a Right band, a centred one Stereo.
#[test]
fn each_resonance_is_placed_where_its_excess_is() {
    let left_only = stereo(&[(1_200.0, 12.0, 10.0)], &[], None);
    assert_eq!(domains_at(&left_only, 1_200.0), [Domain::Left]);
    let side_only = stereo(&[], &[], Some((1_200.0, 15.0, 10.0)));
    assert_eq!(domains_at(&side_only, 1_200.0), [Domain::Side]);
    let unequal = stereo(&[(1_200.0, 15.0, 10.0)], &[(1_200.0, 8.0, 10.0)], None);
    assert_eq!(domains_at(&unequal, 1_200.0), [Domain::Left, Domain::Right]);
    // A pair's two bands stand at one centre, the candidate's.
    let pair = unequal
        .candidates()
        .into_iter()
        .find(|candidate| near(candidate.freq, 1_200.0, 1.0 / 24.0))
        .unwrap();
    assert!(
        pair.bands.iter().all(|band| band.freq == pair.freq),
        "{pair:?}"
    );
    let centred = stereo(&[(1_200.0, 12.0, 10.0)], &[(1_200.0, 12.0, 10.0)], None);
    assert_eq!(domains_at(&centred, 1_200.0), [Domain::Average]);
}

/// **Only heard frames count, and a reset starts the listen again** (plan §7.5).
#[test]
fn quiet_frames_count_for_nothing_and_a_reset_restarts_the_listen() {
    let mut finder = Finder::new(RATE);
    let mut noise = Noise(7);
    let mut resonance = Resonance::new(1_000.0, 12.0, 10.0);
    let mut seq = 0;
    // −70 dBFS.
    feed(&mut finder, &mut seq, 10.0, 2, || {
        let x = resonance.tick(0.000_3 * noise.next());
        [x, x]
    });
    assert!(!finder.listened(), "{} s heard", finder.heard_seconds());
    assert!(finder.measured().is_empty(), "quiet frames enter nothing");
    feed(&mut finder, &mut seq, 6.0, 2, || {
        let x = resonance.tick(0.3 * noise.next());
        [x, x]
    });
    assert!(finder.listened());
    let before = finder.revision();
    let block = Block {
        seq,
        generation: 1,
        sample_rate: RATE,
        channels: 2,
        pre: [[0.0; BLOCK_FRAMES]; 2],
        post: [[0.0; BLOCK_FRAMES]; 2],
        key: [[0.0; BLOCK_FRAMES]; 2],
    };
    finder.ingest(&block);
    assert!(!finder.listened(), "a new generation is a new listen");
    assert_ne!(finder.revision(), before);
    assert!(finder.measured().is_empty());
}

/// No EQ in the sound.
fn flat() -> HeardEq {
    HeardEq {
        bands: Vec::new(),
        sample_rate: f64::from(RATE),
        deepest_db: 30.0,
    }
}

/// **At 100 % the applied bell is the peak's inverse** (plan §7.5): a cookbook-shaped peak cancelled
/// within 1 dB across its width.
#[test]
fn at_full_amount_the_proposal_cancels_the_peak_across_its_width() {
    let finder = melody_through(&[(2_000.0, 9.0, 8.0)], -10.0, 10.0);
    let candidates = finder.candidates();
    let proposals = propose(finder.evidence(), &candidates, 1.0, 24, &flat(), &[]);
    let proposal = proposals
        .iter()
        .find(|proposal| near(proposal.freq, 2_000.0, 1.0 / 48.0))
        .unwrap_or_else(|| panic!("{proposals:#?}"));
    let band = proposal.bands[0];
    let peak = Resonance::new(2_000.0, 9.0, 8.0);
    let bell = band.cascade(f64::from(RATE));
    // Across the peak's width at half its height, and a little beyond.
    let edge = graph_edge(8.0);
    for step in 0..=40 {
        let freq = 2_000.0 / edge * edge.powf(2.0 * step as f32 / 40.0);
        let left = peak.db(freq) + bell.response_db(f64::from(freq), f64::from(RATE)) as f32;
        assert!(left.abs() <= 1.0, "{freq:.0} Hz: {left:+.2} dB left");
    }
}

/// `f0 · r(Q)`, a second-order band's upper edge ratio (the graph's `edge_ratio`).
fn graph_edge(q: f32) -> f32 {
    (1.0 + 1.0 / (4.0 * q * q)).sqrt() + 1.0 / (2.0 * q)
}

fn measured(domain: Domain, freq: f32, prominence: f32, q: f32) -> Measured {
    Measured {
        domain,
        rung: 0,
        freq,
        db: -20.0,
        prominence,
        q,
    }
}

fn candidate(bands: Vec<Measured>) -> Candidate {
    Candidate {
        freq: bands[0].freq,
        audibility: bands
            .iter()
            .map(Measured::audibility)
            .reduce(f32::max)
            .unwrap(),
        bands,
    }
}

/// Each proposed band's response, on its own spectrum, at its centre: the EQ's and every
/// proposal's together.
fn reached(proposals: &[Proposal], heard: &HeardEq) -> Vec<(ProposedBand, f32)> {
    let rate = heard.sample_rate;
    let bands: Vec<(Cascade, Placement)> = heard
        .bands
        .iter()
        .cloned()
        .chain(proposals.iter().flat_map(|proposal| {
            proposal
                .bands
                .iter()
                .map(move |band| (band.cascade(rate), band.domain.placement()))
        }))
        .collect();
    proposals
        .iter()
        .flat_map(|proposal| proposal.bands.iter().copied())
        .map(|band| {
            let with = domain_db(
                band.domain,
                stages(bands.iter().map(|(c, p)| (c, *p)), band.freq, rate),
            );
            let without = domain_db(
                band.domain,
                stages(heard.bands.iter().map(|(c, p)| (c, *p)), band.freq, rate),
            );
            (band, with - without)
        })
        .collect()
}

/// **Depths are solved together** (plan §7.5): two candidates a sixth of an octave apart, applied
/// together, each within 0.5 dB of its target; two overlapping in different placements within
/// 1 dB; a target past the cap held at −12 dB; Q held in its bounds.
#[test]
fn depths_are_solved_together_capped_and_their_q_bounded() {
    let finder = Finder::new(RATE);
    let sixth = 2f32.powf(1.0 / 6.0);
    let pair = [
        candidate(vec![measured(Domain::Average, 1_000.0, 10.0, 4.0)]),
        candidate(vec![measured(Domain::Average, 1_000.0 * sixth, 8.0, 4.0)]),
    ];
    let proposals = propose(finder.evidence(), &pair, 0.5, 24, &flat(), &[]);
    assert_eq!(proposals.len(), 2);
    for (band, at) in reached(&proposals, &flat()) {
        assert!(
            (at - band.target_db).abs() <= 0.5,
            "{} Hz reaches {at:.2} for {:.2}",
            band.freq,
            band.target_db
        );
        assert!(
            band.gain_db > band.target_db,
            "a neighbour's skirt does part of the cut"
        );
    }
    let mixed = [
        candidate(vec![measured(Domain::Left, 1_000.0, 10.0, 3.0)]),
        candidate(vec![measured(Domain::Mid, 1_000.0 * sixth, 10.0, 3.0)]),
    ];
    let proposals = propose(finder.evidence(), &mixed, 0.6, 24, &flat(), &[]);
    for (band, at) in reached(&proposals, &flat()) {
        assert!(
            (at - band.target_db).abs() <= 1.0,
            "{:?} at {} Hz reaches {at:.2} for {:.2}",
            band.domain,
            band.freq,
            band.target_db
        );
    }
    let deep = [candidate(vec![measured(
        Domain::Average,
        1_000.0,
        30.0,
        40.0,
    )])];
    let proposals = propose(finder.evidence(), &deep, 1.0, 24, &flat(), &[]);
    let band = proposals[0].bands[0];
    assert_eq!(band.gain_db, -MAX_CUT_DB, "held at the cap");
    assert_eq!(band.q, Q_RANGE.1, "Q held at its top");
    let wide = [candidate(vec![measured(
        Domain::Average,
        1_000.0,
        10.0,
        0.2,
    )])];
    let proposals = propose(finder.evidence(), &wide, 0.5, 24, &flat(), &[]);
    assert_eq!(proposals[0].bands[0].q, Q_RANGE.0, "Q held at its bottom");
}

/// **What is left decides** (plan §7.5): a candidate the EQ as heard already cuts by Amount × P is
/// treated and not proposed; bypassed, or with a larger Amount, it is proposed again; and a
/// proposal shown stays until under the release, so updates do not flicker.
#[test]
fn a_treated_candidate_is_not_proposed_and_comes_back_when_it_is_not() {
    let finder = melody_through(&[(2_000.0, 10.0, 8.0)], -10.0, 10.0);
    let candidates = finder.candidates();
    let candidate = candidates
        .iter()
        .find(|candidate| near(candidate.freq, 2_000.0, 1.0 / 48.0))
        .unwrap_or_else(|| panic!("{candidates:#?}"))
        .clone();
    let one = std::slice::from_ref(&candidate);
    let prominence = candidate.bands[0].prominence;
    let q = candidate.bands[0].q;
    let eq_cutting = |depth: f32| HeardEq {
        bands: vec![(
            ProposedBand {
                domain: Domain::Average,
                freq: candidate.freq,
                q,
                gain_db: depth,
                target_db: depth,
            }
            .cascade(f64::from(RATE)),
            Placement::Stereo,
        )],
        sample_rate: f64::from(RATE),
        deepest_db: 30.0,
    };
    // The EQ already takes a fifth of the prominence off.
    let treated = eq_cutting(-0.2 * prominence);
    assert!(propose(finder.evidence(), one, 0.2, 24, &treated, &[]).is_empty());
    assert_eq!(
        propose(finder.evidence(), one, 0.2, 24, &flat(), &[]).len(),
        1,
        "its band bypassed"
    );
    assert_eq!(
        propose(finder.evidence(), one, 0.5, 24, &treated, &[]).len(),
        1,
        "a larger Amount"
    );
    // A little less than the treated threshold is left: not proposed afresh, kept once shown.
    let nearly = eq_cutting(-(0.2 * prominence - 0.4));
    assert!(propose(finder.evidence(), one, 0.2, 24, &nearly, &[]).is_empty());
    let shown = propose(finder.evidence(), one, 0.2, 24, &flat(), &[]);
    assert_eq!(
        propose(finder.evidence(), one, 0.2, 24, &nearly, &shown).len(),
        1
    );
}

/// **The count, spacing and free slots** (plan §7.5): at most eight candidates, a sixth of an
/// octave apart; a pair needs two slots and is passed over with one left, for the next that fits.
#[test]
fn candidates_are_counted_spaced_and_fitted_to_the_free_slots() {
    let many: Vec<Candidate> = (0..12)
        .map(|n| {
            candidate(vec![measured(
                Domain::Average,
                100.0 * 1.4f32.powi(n),
                6.0 + n as f32,
                4.0,
            )])
        })
        .collect();
    let ranked = rank(many);
    assert_eq!(ranked.len(), MAX_CANDIDATES);
    for pair in ranked.windows(2) {
        assert!(pair[0].audibility >= pair[1].audibility);
    }
    let close = rank(vec![
        candidate(vec![measured(Domain::Average, 1_000.0, 12.0, 4.0)]),
        candidate(vec![measured(Domain::Average, 1_080.0, 9.0, 4.0)]),
    ]);
    assert_eq!(close.len(), 1, "a sixth of an octave apart at least");
    assert_eq!(close[0].freq, 1_000.0, "the more audible kept");

    let finder = Finder::new(RATE);
    let ordered = [
        candidate(vec![
            measured(Domain::Left, 500.0, 14.0, 4.0),
            measured(Domain::Right, 500.0, 8.0, 4.0),
        ]),
        candidate(vec![measured(Domain::Average, 2_000.0, 10.0, 4.0)]),
    ];
    let proposals = propose(finder.evidence(), &ordered, 0.2, 1, &flat(), &[]);
    assert_eq!(proposals.len(), 1, "{proposals:#?}");
    assert_eq!(proposals[0].freq, 2_000.0, "the pair passed over");
    let both = propose(finder.evidence(), &ordered, 0.2, 3, &flat(), &[]);
    assert_eq!(both.len(), 2);
    assert_eq!(both[0].bands.len(), 2, "a pair is one candidate, two bands");
}

/// **The finder's cost**, at 192 kHz (plan §7.5): on the GUI thread, a 60 Hz frame's blocks into
/// the finder and, each half second, the copy of its evidence a search takes, against the
/// analyser's 2 ms budget; on the worker, the search itself, which must finish well inside the
/// half second between searches. The audio is rendered beforehand, a second of it cycled, so only
/// the finder is timed. Release only; the figures are in the plugin's `AGENTS.md`.
///
/// `cargo test -p mxm-fx-eq-n --release --lib finder_cost -- --ignored --nocapture`
#[test]
#[ignore = "a release measurement"]
fn finder_cost() {
    let rate = 192_000.0f32;
    let mut melody = Melody::scale();
    let mut noise = Noise(3);
    let mut resonance = Resonance::new(1_000.0, 12.0, 10.0);
    let blocks: Vec<[[f32; BLOCK_FRAMES]; 2]> = (0..(rate as usize / BLOCK_FRAMES))
        .map(|_| {
            let mut pre = [[0.0f32; BLOCK_FRAMES]; 2];
            let [left, right] = &mut pre;
            for (left, right) in left.iter_mut().zip(right.iter_mut()) {
                let x = resonance.tick(melody.next() + 0.3 * noise.next());
                *left = x;
                *right = -x;
            }
            pre
        })
        .collect();
    let mut finder = Finder::new(rate);
    let per_frame = (rate / 60.0) as usize / BLOCK_FRAMES + 1;
    let mut seq = 0u64;
    let mut frame = |finder: &mut Finder| {
        for _ in 0..per_frame {
            let block = Block {
                seq,
                generation: 0,
                sample_rate: rate,
                channels: 2,
                pre: blocks[seq as usize % blocks.len()],
                post: [[0.0; BLOCK_FRAMES]; 2],
                key: [[0.0; BLOCK_FRAMES]; 2],
            };
            finder.ingest(&block);
            seq += 1;
        }
    };
    // Warm: eight seconds, every rung's width frames full and the listen past its four seconds.
    for _ in 0..480 {
        frame(&mut finder);
    }
    let frames = 600;
    let (mut worst_gui, mut worst_copy, mut worst_search, mut total) =
        (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for n in 0..frames {
        let start = std::time::Instant::now();
        frame(&mut finder);
        let mut gui = start.elapsed().as_secs_f64() * 1_000.0;
        total += gui;
        if n % 30 == 0 {
            // The GUI thread copies the evidence; the worker searches it.
            let start = std::time::Instant::now();
            let evidence = finder.evidence().clone();
            let copy = start.elapsed().as_secs_f64() * 1_000.0;
            worst_copy = worst_copy.max(copy);
            gui += copy;
            let start = std::time::Instant::now();
            let candidates = evidence.candidates();
            let _ = propose(&evidence, &candidates, 0.2, 24, &flat(), &[]);
            worst_search = worst_search.max(start.elapsed().as_secs_f64() * 1_000.0);
        }
        worst_gui = worst_gui.max(gui);
    }
    println!(
        "finder at 192 kHz: on the GUI thread {:.3} ms a frame on average, {worst_gui:.3} ms at \
         worst (the evidence copied in {worst_copy:.3} ms); a search {worst_search:.3} ms",
        total / f64::from(frames)
    );
    assert!(
        worst_gui < crate::editor::spectrum::BUDGET_MS,
        "{worst_gui} ms of a frame"
    );
    assert!(
        worst_search < 1_000.0 * UPDATE_SECONDS / 4.0,
        "a search {worst_search} ms"
    );
}

/// **The fit finds a cookbook bell exactly** (plan §7.5): on a noiseless sloping floor, its
/// centre, gain and Q, from a guess off in all three.
#[test]
fn the_fit_finds_a_cookbook_bell_exactly() {
    let bin_hz = 1.464_843_8f32;
    for (centre, gain, q) in [
        (1_000.0f32, 12.0f32, 10.0f32),
        (700.0, 6.0, 3.0),
        (1_300.0, 20.0, 25.0),
    ] {
        let db: Vec<f32> = (0..4_097)
            .map(|bin| {
                let freq = (bin as f32 * bin_hz).max(1.0);
                -60.0 - 2.0 * (freq / 1_000.0).log2()
                    + bell_db(f64::from(freq / centre), f64::from(gain), f64::from(q)) as f32
            })
            .collect();
        let guess = Measured {
            domain: Domain::Average,
            rung: 1,
            freq: centre * 1.02,
            db: -60.0 + gain,
            prominence: gain * 0.8,
            q: q * 1.3,
        };
        let (freq, fitted, fitted_q) = fit_bell(&db, bin_hz, (1, 4_096), &guess).expect("a fit");
        assert!(near(freq, centre, 0.001), "{centre}: {freq}");
        assert!((fitted - gain).abs() < 0.05, "{centre}: gain {fitted}");
        assert!((fitted_q / q - 1.0).abs() < 0.01, "{centre}: Q {fitted_q}");
    }
}

/// **The excitation lies outside the cell's own third** (plan §7.5), whatever the loudest cell is
/// near: a cell between the loudest and the next loudest takes neither, if both are within its
/// third, but the loudest beyond it.
#[test]
fn what_plays_for_a_cell_lies_outside_its_own_third() {
    let sixth = 8;
    // Loudest first: cell 100, then 109, then 140.
    let loudest_first = [100, 109, 140, 60];
    assert_eq!(
        playing(&loudest_first, 104, sixth),
        Some(140),
        "100 and 109 are both near"
    );
    assert_eq!(playing(&loudest_first, 120, sixth), Some(100));
    assert_eq!(playing(&loudest_first, 100, sixth), Some(109));
    assert_eq!(playing(&[100, 101], 100, sixth), None);
}

/// **A low gain scale bounds the depth** (plan §7.5): a proposal never cuts deeper than a band can
/// be heard to, so Apply gives the depth shown.
#[test]
fn the_gain_scale_bounds_how_deep_a_proposal_cuts() {
    let finder = Finder::new(RATE);
    let deep = [Candidate {
        freq: 1_000.0,
        audibility: 20.0,
        bands: vec![Measured {
            domain: Domain::Average,
            rung: 0,
            freq: 1_000.0,
            db: -20.0,
            prominence: 20.0,
            q: 8.0,
        }],
    }];
    let quiet = HeardEq {
        deepest_db: 3.0,
        ..flat()
    };
    let proposals = propose(finder.evidence(), &deep, 1.0, 24, &quiet, &[]);
    assert_eq!(
        proposals[0].bands[0].gain_db, -3.0,
        "held where a band can reach"
    );
}

/// **Dropping the worker waits for its thread** (plan §7.5): a search under way finishes, and no
/// plugin code is left running.
#[test]
fn dropping_the_worker_ends_its_thread() {
    let mut worker = Worker::new().expect("a thread");
    let search = Search {
        evidence: Finder::new(RATE).evidence().clone(),
        candidates: None,
        heard: flat(),
        amount: 0.2,
        free_slots: 24,
        shown: Vec::new(),
        listen: 0,
    };
    assert!(worker.start(search).is_ok());
    assert!(worker.busy());
    drop(worker);
}

/// **Early in a listen nothing false is proposed** (plan §7.5): at the first update, and as the
/// lowest rung's first candidates come, neither the melody's lines nor the noise under it are.
#[test]
fn early_in_a_listen_nothing_false_is_proposed() {
    for seconds in [4.5, 6.0, 8.0] {
        let finder = melody_through(&[(1_000.0, 12.0, 10.0)], -10.0, seconds);
        assert!(finder.listened());
        for candidate in finder.candidates() {
            assert!(
                near(candidate.freq, 1_000.0, 1.0 / 24.0),
                "{seconds} s, only the resonance: {candidate:?}"
            );
        }
    }
}

/// **Every rate the plugin runs at is read without a gap, to as high as a band runs** (plan
/// §7.5): the rungs own their frequencies continuously from the lowest to the DSP's
/// `max_frequency`, at the plan's edges wherever every rung reaches them, on both sides of the
/// decimation thresholds; where rung 0 is decimated, a rung at the full rate reads the top; and
/// every rung but the fastest reads a third of an octave past what it owns.
#[test]
fn the_rungs_own_every_frequency_at_every_rate() {
    for rate in [
        4_000.0f32, 12_000.0, 32_000.0, 44_100.0, 48_000.0, 64_000.0, 65_000.0, 88_200.0, 96_000.0,
        128_000.0, 130_000.0, 176_400.0, 192_000.0,
    ] {
        let finder = Finder::new(rate);
        assert_eq!(finder.sample_rate(), rate);
        let rungs = &finder.evidence().rungs;
        let lowest = rungs.len() - 1;
        assert_eq!(rungs[lowest].low, LOWEST_HZ, "{rate}");
        for pair in rungs.windows(2) {
            assert_eq!(
                pair[1].high, pair[0].low,
                "{rate}: a rung meets the one above"
            );
        }
        assert_eq!(
            rungs[0].high,
            mxm_fx_eq_n_dsp::max_frequency(f64::from(rate)) as f32,
            "{rate}: to as high as a band runs"
        );
        for rung in rungs {
            assert!(
                rung.high <= rung.top + 1.0e-3,
                "{rate}: a rung owns only what it reads"
            );
        }
        for rung in &rungs[1..] {
            assert!(
                rung.high * 2f32.powf(KEY_OCTAVES) <= rung.top * (1.0 + 1.0e-5),
                "{rate}: a third of an octave read past what it owns"
            );
        }
        if rate >= 44_100.0 {
            let edges: Vec<f32> = rungs[lowest - 2..].iter().map(|rung| rung.low).collect();
            assert_eq!(edges, [RUNG_EDGES[1], RUNG_EDGES[0], LOWEST_HZ], "{rate}");
            assert_eq!(
                rungs.len(),
                if rate > FIRST_RATE_MAX { 4 } else { 3 },
                "{rate}"
            );
        }
    }
}

/// **A low rate is listened to as it is**: a 4 kHz finder takes 4 kHz blocks.
#[test]
fn a_low_rate_is_listened_to() {
    let rate = 4_000.0f32;
    let mut finder = Finder::new(rate);
    let mut noise = Noise(5);
    for seq in 0..(rate as usize * 6 / BLOCK_FRAMES) as u64 {
        let mut block = Block {
            seq,
            generation: 0,
            sample_rate: rate,
            channels: 1,
            pre: [[0.0; BLOCK_FRAMES]; 2],
            post: [[0.0; BLOCK_FRAMES]; 2],
            key: [[0.0; BLOCK_FRAMES]; 2],
        };
        for index in 0..BLOCK_FRAMES {
            block.pre[0][index] = 0.3 * noise.next();
        }
        finder.ingest(&block);
    }
    assert!(finder.listened(), "{} s heard", finder.heard_seconds());
}

/// **Two excitations are two clusters a semitone apart, of the frames with one recorded** (plan
/// §7.5): none recorded is no diversity; one, or one odd frame beside it, is one; one that drifts
/// a quarter or three quarters of a semitone is one; two a semitone apart are two.
#[test]
fn excitations_are_counted_as_clusters_of_those_recorded() {
    let cell = |excitations: &[(u16, u32)], present: u32| Cell {
        eligible: present,
        present,
        excitations: excitations.to_vec(),
    };
    assert!(!cell(&[], 16).varied(), "none recorded");
    assert!(!cell(&[(100, 16)], 16).varied(), "one");
    assert!(
        !cell(&[(100, 15), (120, 1)], 16).varied(),
        "one, and one odd frame"
    );
    assert!(
        !cell(&[(100, 1)], 16).varied(),
        "one rare frame among those standing out"
    );
    assert!(
        !cell(&[(100, 10), (101, 10)], 20).varied(),
        "a quarter of a semitone"
    );
    assert!(
        !cell(&[(100, 10), (103, 10)], 20).varied(),
        "three quarters of a semitone"
    );
    assert!(
        cell(&[(100, 10), (104, 10)], 20).varied(),
        "a semitone apart"
    );
    let mut drifting = Cell::default();
    for frame in 0..20 {
        drifting.present += 1;
        drifting.excited_by(if frame % 2 == 0 { 100 } else { 101 });
    }
    assert!(!drifting.varied(), "recorded as it comes");
}

/// White noise through `resonances`, `seconds` of it at `rate`, into a finder. Each resonance is
/// the EQ's own bell, matched to its analogue curve up to the top of the range, so it keeps its Q
/// near Nyquist where the cookbook's narrows.
fn noise_through_at(rate: f32, resonances: &[(f32, f32, f32)], seconds: f32) -> Finder {
    let mut finder = Finder::new(rate);
    let mut noise = Noise(0x2545_f491);
    let mut filters: Vec<mxm_fx_eq_n_dsp::filter::Filter> = resonances
        .iter()
        .map(|&(freq, gain_db, q)| {
            mxm_fx_eq_n_dsp::filter::Filter::fresh(&mxm_fx_eq_n_dsp::design(
                mxm_fx_eq_n_dsp::Design {
                    shape: mxm_fx_eq_n_dsp::Shape::Bell,
                    slope: mxm_fx_eq_n_dsp::Slope::Db12,
                    freq: f64::from(freq),
                    gain_db: f64::from(gain_db),
                    q: f64::from(q),
                },
                f64::from(rate),
            ))
        })
        .collect();
    let blocks = (rate * seconds) as usize / BLOCK_FRAMES;
    for seq in 0..blocks as u64 {
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
            let mut x = f64::from(0.3 * noise.next());
            for filter in &mut filters {
                [x] = filter.process([x]);
            }
            block.pre[0][index] = x as f32;
            block.pre[1][index] = x as f32;
        }
        finder.ingest(&block);
    }
    finder
}

/// **Resonances near the top and either side of a rung's boundary are found** (plan §7.5): within
/// a semitone of `max_frequency` at 48 kHz; at 65 kHz just below and above where the decimated
/// rung 0 hands over to the full-rate one (10.3 kHz) and just below its 13 kHz anti-alias corner;
/// either side of the 1.6 kHz edge at 48 kHz; and in the full-rate rung at 96 kHz.
#[test]
fn resonances_at_the_edges_of_what_is_read_are_found() {
    for (rate, freq) in [
        (48_000.0f32, 21_000.0f32),
        (48_000.0, 23_000.0),
        (48_000.0, 1_550.0),
        (48_000.0, 1_650.0),
        (65_000.0, 10_000.0),
        (65_000.0, 10_600.0),
        (65_000.0, 12_800.0),
        (65_000.0, 14_500.0),
        (96_000.0, 22_000.0),
    ] {
        let finder = noise_through_at(rate, &[(freq, 12.0, 20.0)], 8.0);
        let candidates = finder.evidence().candidates();
        assert!(
            candidates
                .iter()
                .any(|candidate| near(candidate.freq, freq, 1.0 / 48.0)),
            "{freq} Hz at {rate}: {candidates:#?}"
        );
    }
}
