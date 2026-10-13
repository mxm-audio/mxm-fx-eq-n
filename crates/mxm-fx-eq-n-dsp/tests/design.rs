//! The band designs against their meanings (brief A5) and their analogue prototypes.

use mxm_fx_eq_n_dsp::{
    Design, MIN_SAMPLE_RATE, Method, Section, Shape, Slope, analogue_db, design, design_running,
    design_solo, design_with, max_frequency, usable_sample_rate,
};
use std::f64::consts::FRAC_1_SQRT_2;

fn band(shape: Shape, freq: f64, gain_db: f64, q: f64, slope: Slope) -> Design {
    Design {
        shape,
        slope,
        freq,
        gain_db,
        q,
    }
}

fn db(design_: Design, freq: f64, rate: f64) -> f64 {
    design(design_, rate).response_db(freq, rate)
}

/// Every section finite and stable: `g > 0` and `k > 0` are the SVF's whole stability condition.
fn stable(section: &Section) -> bool {
    match *section {
        Section::Second { g, k, m0, m1, m2 } => {
            g.is_finite()
                && g > 0.0
                && k.is_finite()
                && k > 0.0
                && [m0, m1, m2].iter().all(|m| m.is_finite())
        }
        Section::First { g, m0, m1 } => {
            g.is_finite() && g > 0.0 && m0.is_finite() && m1.is_finite()
        }
    }
}

#[test]
fn every_design_in_range_is_finite_and_stable_at_every_rate() {
    for rate in [1_000.0, 22_050.0, 44_100.0, 96_000.0, 192_000.0, 768_000.0] {
        let top = max_frequency(rate);
        for shape in Shape::ALL {
            for slope in Slope::ALL {
                if !shape.has_slope() && slope != Slope::Db12 {
                    continue;
                }
                for fi in 0..=16 {
                    let freq = 10.0 * (top / 10.0f64).powf(f64::from(fi) / 16.0);
                    for qi in 0..=8 {
                        let q = 0.025 * 1_600f64.powf(f64::from(qi) / 8.0);
                        for gain_db in [-60.0, -31.0, -6.0, 0.5, 12.0, 47.0, 60.0] {
                            let candidate = band(shape, freq, gain_db, q, slope);
                            let cascade = design(candidate, rate);
                            assert!(
                                cascade.sections().iter().all(stable),
                                "{candidate:?} at {rate}: {:?}",
                                cascade.sections()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_gain_shape_at_0_db_is_the_identity_with_no_sections() {
    for shape in Shape::ALL.into_iter().filter(|shape| shape.has_gain()) {
        let cascade = design(band(shape, 1_234.0, 0.0, 3.0, Slope::Db12), 48_000.0);
        assert!(cascade.sections().is_empty(), "{shape:?}");
    }
    // A gainless shape ignores its stored gain.
    let notch = |gain_db| {
        design(
            band(Shape::Notch, 1_234.0, gain_db, 3.0, Slope::Db12),
            48_000.0,
        )
    };
    assert_eq!(notch(0.0), notch(17.0));
}

#[test]
fn a_cut_exactly_cancels_the_equal_boost() {
    for rate in [44_100.0, 192_000.0] {
        for shape in [
            Shape::Bell,
            Shape::LowShelf,
            Shape::HighShelf,
            Shape::TiltShelf,
        ] {
            for (freq, q, gain) in [
                (25.0, 0.4, 9.0),
                (1_000.0, 1.0, 18.0),
                (15_000.0, 6.0, 30.0),
                (400.0, 40.0, 3.0),
            ] {
                let boost = design(band(shape, freq, gain, q, Slope::Db12), rate);
                let cut = design(band(shape, freq, -gain, q, Slope::Db12), rate);
                for probe in [12.0, 200.0, freq, 3_000.0, 19_000.0] {
                    let sum = boost.response_db(probe, rate) + cut.response_db(probe, rate);
                    assert!(
                        sum.abs() < 1.0e-8,
                        "{shape:?} {freq} Hz at {rate}: {sum} dB at {probe} Hz"
                    );
                }
            }
        }
    }
}

#[test]
fn a_bell_reaches_its_gain_at_its_centre_and_half_of_it_at_its_band_edges() {
    // Miller's probe (RaneNote 167, via research §3.2): the bell's level at its edges names the Q
    // convention. Half-gain-in-dB: the edges of an N-octave band sit at half the dB, whatever the
    // gain. 1/Q = 2 sinh(N ln2 / 2).
    let rate = 96_000.0;
    for q in [0.5f64, 1.0, 4.0] {
        let octaves = 2.0 / std::f64::consts::LN_2 * (0.5 / q).asinh();
        for gain in [-12.0, -3.0, 3.0, 12.0] {
            let bell = band(Shape::Bell, 1_000.0, gain, q, Slope::Db12);
            assert!(
                (db(bell, 1_000.0, rate) - gain).abs() < 1.0e-9,
                "centre, Q {q}, {gain} dB"
            );
            for edge in [
                1_000.0 * 2f64.powf(-octaves / 2.0),
                1_000.0 * 2f64.powf(octaves / 2.0),
            ] {
                let level = db(bell, edge, rate);
                assert!(
                    (level - gain / 2.0).abs() < 0.01,
                    "Q {q}, {gain} dB: {level} at {edge}"
                );
            }
        }
    }
}

#[test]
fn a_bells_peak_is_at_its_frequency_even_near_nyquist() {
    // The matched bell keeps Vicanek's extremum at the centre.
    let rate = 44_100.0;
    for freq in [5_000.0, 15_000.0, 19_000.0] {
        let bell = band(Shape::Bell, freq, 12.0, 1.0, Slope::Db12);
        for cents in [-20.0, 20.0] {
            let beside = db(bell, freq * 2f64.powf(cents / 1_200.0), rate);
            assert!(beside < 12.0, "{freq}: {beside} dB at {cents} cents");
        }
    }
}

#[test]
fn a_shelf_is_half_its_gain_at_its_frequency_and_its_gain_at_the_end() {
    for rate in [44_100.0, 48_000.0, 192_000.0] {
        for (freq, gain, q) in [
            (80.0, 12.0, FRAC_1_SQRT_2),
            (2_000.0, -9.0, 1.4),
            (12_000.0, 18.0, FRAC_1_SQRT_2),
        ] {
            let low = band(Shape::LowShelf, freq, gain, q, Slope::Db12);
            let high = band(Shape::HighShelf, freq, gain, q, Slope::Db12);
            assert!(
                (db(low, freq, rate) - gain / 2.0).abs() < 1.0e-6,
                "low {freq} at {rate}"
            );
            assert!(
                (db(high, freq, rate) - gain / 2.0).abs() < 1.0e-6,
                "high {freq} at {rate}"
            );
            assert!(
                (db(low, 1.0e-3, rate) - gain).abs() < 1.0e-6,
                "low shelf's DC"
            );
            assert!(db(high, 1.0e-3, rate).abs() < 1.0e-6, "high shelf's DC");
        }
    }
}

#[test]
fn a_cuts_corner_is_its_q_and_its_stopband_falls_at_its_slope() {
    let rate = 96_000.0;
    for slope in Slope::ALL {
        for (shape, below, above) in [
            (Shape::LowCut, 125.0, 8_000.0),
            (Shape::HighCut, 8_000.0, 125.0),
        ] {
            let cut = band(shape, 1_000.0, 0.0, FRAC_1_SQRT_2, slope);
            let corner = db(cut, 1_000.0, rate);
            assert!(
                (corner + 3.0103).abs() < 1.0e-6,
                "{shape:?} {slope:?}: {corner} at the corner"
            );
            // Three octaves into the stopband, against the Butterworth prototype.
            let stop = db(cut, below, rate);
            let order = slope.order() as f64;
            let expected = -10.0 * (1.0 + 8f64.powf(2.0 * order)).log10();
            assert!(
                (stop - expected).abs() < 0.2,
                "{shape:?} {slope:?}: {stop}, not {expected}"
            );
            let pass = db(cut, above, rate) - analogue_db(cut, above, rate);
            assert!(pass.abs() < 0.01, "{shape:?} {slope:?} passband: {pass}");
            // Q sets the corner's gain to Q itself, except for the first-order cut, which has none.
            let resonant = db(band(shape, 1_000.0, 0.0, 4.0, slope), 1_000.0, rate);
            if slope == Slope::Db6 {
                assert!(
                    (resonant - corner).abs() < 1.0e-9,
                    "a 6 dB/octave cut has no Q"
                );
                assert!(!shape.has_q(slope));
            } else {
                assert!(
                    (resonant - 20.0 * 4f64.log10()).abs() < 1.0e-6,
                    "{slope:?}: {resonant}"
                );
                assert!(shape.has_q(slope));
            }
        }
    }
}

#[test]
fn the_tilt_turns_about_its_pivot() {
    for rate in [44_100.0, 192_000.0] {
        for (pivot, gain) in [(200.0, 6.0), (1_000.0, -9.0), (8_000.0, 12.0)] {
            let tilt = band(Shape::TiltShelf, pivot, gain, 1.0, Slope::Db12);
            assert!(db(tilt, pivot, rate).abs() < 1.0e-9, "{pivot} at {rate}");
            assert!(
                (db(tilt, 1.0e-3, rate) + gain).abs() < 1.0e-6,
                "−gain at DC"
            );
        }
    }
}

#[test]
fn the_notch_is_silent_at_its_centre_and_the_band_pass_unity() {
    for rate in [44_100.0, 192_000.0] {
        for (freq, q) in [(50.0, 2.0), (3_000.0, 0.5), (16_000.0, 10.0)] {
            assert!(db(band(Shape::Notch, freq, 0.0, q, Slope::Db12), freq, rate) < -120.0);
            let pass = band(Shape::BandPass, freq, 0.0, q, Slope::Db12);
            assert!(db(pass, freq, rate).abs() < 1.0e-9, "{freq} at {rate}");
            assert!(db(band(Shape::Notch, freq, 0.0, q, Slope::Db12), 1.0e-3, rate).abs() < 1.0e-6);
        }
    }
}

#[test]
fn a_20_hz_q_10_bell_at_192_khz_is_exact() {
    // The research's low-frequency precision case (§4.5): direct-form coefficients in f32 make
    // this a +13.6 dB bell 97.5 cents sharp.
    let rate = 192_000.0;
    let bell = band(Shape::Bell, 20.0, 12.0, 10.0, Slope::Db12);
    assert!((db(bell, 20.0, rate) - 12.0).abs() < 1.0e-9);
    for cents in [-2.0, 2.0] {
        assert!(db(bell, 20.0 * 2f64.powf(cents / 1_200.0), rate) < 12.0);
    }
    let analogue = analogue_db(bell, 21.0, rate);
    assert!((db(bell, 21.0, rate) - analogue).abs() < 1.0e-4);
}

/// The largest error against the analogue prototype up to 20 kHz, stopbands below −60 dB left out.
fn worst_error(method: Method, candidate: Design, rate: f64) -> f64 {
    let cascade = design_with(method, candidate, rate);
    (0..1_000)
        .map(|i| 10.0 * 2_000f64.powf(f64::from(i) / 999.0))
        .filter(|&freq| analogue_db(candidate, freq, rate) >= -60.0)
        .map(|freq| cascade.response_db(freq, rate) - analogue_db(candidate, freq, rate))
        .fold(0.0f64, |worst, error| {
            if error.abs() > worst.abs() {
                error
            } else {
                worst
            }
        })
}

#[test]
fn the_matched_design_stays_on_its_analogue_curve_to_20_khz() {
    // The figures `eq_n_cramping` measured, held: (case, matched limit, bilinear at least).
    let cases = [
        (
            band(Shape::Bell, 10_000.0, 12.0, 1.0, Slope::Db12),
            0.9,
            3.0,
        ),
        (
            band(Shape::Bell, 15_000.0, 12.0, 1.0, Slope::Db12),
            0.8,
            6.0,
        ),
        (
            band(Shape::Bell, 12_000.0, -12.0, 1.0, Slope::Db12),
            1.0,
            4.0,
        ),
        (
            band(Shape::LowShelf, 10_000.0, 12.0, FRAC_1_SQRT_2, Slope::Db12),
            0.2,
            1.3,
        ),
        (
            band(
                Shape::HighShelf,
                15_000.0,
                -12.0,
                FRAC_1_SQRT_2,
                Slope::Db12,
            ),
            0.25,
            2.5,
        ),
        (
            band(Shape::LowCut, 12_000.0, 0.0, FRAC_1_SQRT_2, Slope::Db18),
            0.2,
            6.0,
        ),
        (
            band(Shape::HighCut, 12_000.0, 0.0, FRAC_1_SQRT_2, Slope::Db18),
            0.6,
            20.0,
        ),
        (
            band(Shape::HighCut, 15_000.0, 0.0, FRAC_1_SQRT_2, Slope::Db48),
            0.9,
            40.0,
        ),
        (
            band(Shape::Notch, 15_000.0, 0.0, 8.0, Slope::Db12),
            0.1,
            6.0,
        ),
        (
            band(Shape::BandPass, 10_000.0, 0.0, 2.0, Slope::Db12),
            1.4,
            9.0,
        ),
        (
            band(Shape::TiltShelf, 10_000.0, 12.0, 1.0, Slope::Db12),
            0.35,
            4.0,
        ),
    ];
    for rate in [44_100.0, 48_000.0] {
        for (candidate, limit, bilinear_at_least) in cases {
            let matched = worst_error(Method::Matched, candidate, rate);
            let bilinear = worst_error(Method::Bilinear, candidate, rate);
            assert!(
                matched.abs() <= limit,
                "{candidate:?} at {rate}: matched {matched}"
            );
            assert!(
                bilinear.abs() >= bilinear_at_least * 0.9,
                "{candidate:?} at {rate}: bilinear {bilinear}"
            );
        }
    }
}

#[test]
fn solo_plays_what_the_brief_says() {
    let rate = 48_000.0;
    let solo = |shape, slope| design_solo(band(shape, 1_000.0, 6.0, 4.0, slope), rate);
    // A bell, notch or band pass: a band pass at its frequency and Q.
    for shape in [Shape::Bell, Shape::Notch, Shape::BandPass] {
        let pass = solo(shape, Slope::Db12).expect("offered");
        assert!(pass.response_db(1_000.0, rate).abs() < 1.0e-9);
        assert!(pass.response_db(4_000.0, rate) < -15.0 && pass.response_db(250.0, rate) < -15.0);
    }
    // A shelf: the region it changes.
    let low = solo(Shape::LowShelf, Slope::Db12).unwrap();
    assert!(low.response_db(100.0, rate).abs() < 0.1 && low.response_db(10_000.0, rate) < -30.0);
    let high = solo(Shape::HighShelf, Slope::Db12).unwrap();
    assert!(high.response_db(10_000.0, rate).abs() < 0.1 && high.response_db(100.0, rate) < -30.0);
    // A cut: what it removes, the mirrored cut at its corner and slope, resonance or not.
    for slope in Slope::ALL {
        let removed = solo(Shape::HighCut, slope).unwrap();
        assert!(removed.response_db(8_000.0, rate).abs() < 0.1, "{slope:?}");
        assert!(removed.response_db(125.0, rate) < -15.0, "{slope:?}");
        let removed = solo(Shape::LowCut, slope).unwrap();
        assert!(removed.response_db(125.0, rate).abs() < 0.1, "{slope:?}");
        assert!(removed.response_db(8_000.0, rate) < -15.0, "{slope:?}");
    }
    // A tilt changes everything: no solo.
    assert!(solo(Shape::TiltShelf, Slope::Db12).is_none());
}

#[test]
fn every_public_entry_point_takes_any_sample_rate() {
    for rate in [
        0.0,
        -48_000.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1.0,
        999.0,
        MIN_SAMPLE_RATE,
        48_000.0,
    ] {
        let usable = usable_sample_rate(rate);
        assert!(usable.is_finite() && usable >= MIN_SAMPLE_RATE, "{rate}");
        let top = max_frequency(rate);
        assert!(top.is_finite() && top >= 10.0, "{rate}: {top}");
        for shape in Shape::ALL {
            let candidate = band(shape, 9_000.0, 7.0, 2.0, Slope::Db48);
            let bounded = candidate.bounded(rate);
            assert!(bounded.freq.is_finite() && bounded.freq <= top);
            for cascade in [
                design(candidate, rate),
                design_running(candidate, rate),
                design_with(Method::Bilinear, candidate, rate),
            ] {
                assert!(cascade.sections().iter().all(stable), "{shape:?} at {rate}");
                for freq in [0.0, 20.0, 400.0, 1.0e9] {
                    assert!(
                        cascade.response_db(freq, rate).is_finite(),
                        "{shape:?} at {rate}"
                    );
                }
            }
            if let Some(solo) = design_solo(candidate, rate) {
                assert!(
                    solo.sections().iter().all(stable),
                    "{shape:?} solo at {rate}"
                );
            }
            assert!(analogue_db(candidate, 300.0, rate).is_finite());
        }
    }
}
