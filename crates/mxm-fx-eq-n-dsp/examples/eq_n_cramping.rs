//! Cramping near Nyquist: each shape's largest error against its analogue prototype up to 20 kHz,
//! for the bilinear SVF and the shipped matched design, at 44.1, 48 and 96 kHz.
//!
//! ```text
//! cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_cramping
//! ```
//!
//! The grid is 4,000 log-spaced frequencies from 10 Hz to 20 kHz. The error is digital minus
//! analogue, in dB, and cut stopbands below −60 dB are left out: there the comparison is between
//! two numbers nobody hears.
//!
//! A second table sweeps each shape over the ordinary range — band frequency 20 Hz to 16 kHz, Q
//! 0.1 to 10, gain ±30 dB, every slope — and reports the worst case of each method.

use mxm_fx_eq_n_dsp::{Design, Method, Shape, Slope, analogue_db, design_with};

fn grid_worst(method: Method, shape: Shape, rate: f64) -> (f64, Design) {
    let mut worst = (0.0f64, band(shape, 1_000.0, 0.0, 1.0, Slope::Db12));
    for slope in Slope::ALL {
        if !shape.has_slope() && slope != Slope::Db12 {
            continue;
        }
        for fi in 0..=30 {
            let freq = 20.0 * 800f64.powf(fi as f64 / 30.0);
            for qi in 0..=12 {
                let q = 0.1 * 100f64.powf(qi as f64 / 12.0);
                for gi in -3..=3 {
                    if !shape.has_gain() && gi != 0 {
                        continue;
                    }
                    let candidate = band(shape, freq, f64::from(gi) * 10.0, q, slope);
                    let cascade = design_with(method, candidate, rate);
                    for i in 0..400 {
                        let f = 10.0 * 2_000f64.powf(i as f64 / 399.0);
                        let analogue = analogue_db(candidate, f, rate);
                        if analogue < -60.0 {
                            continue;
                        }
                        let error = cascade.response_db(f, rate) - analogue;
                        if error.abs() > worst.0.abs() {
                            worst = (error, candidate);
                        }
                    }
                }
            }
        }
    }
    worst
}

const RATES: [f64; 3] = [44_100.0, 48_000.0, 96_000.0];

fn worst_error(method: Method, band: Design, rate: f64) -> f64 {
    let cascade = design_with(method, band, rate);
    let mut worst = 0.0f64;
    for i in 0..4_000 {
        let freq = 10.0 * 2_000f64.powf(i as f64 / 3_999.0);
        let analogue = analogue_db(band, freq, rate);
        if analogue < -60.0 {
            continue;
        }
        let error = cascade.response_db(freq, rate) - analogue;
        if error.abs() > worst.abs() {
            worst = error;
        }
    }
    worst
}

fn row(label: &str, band: Design) {
    print!("{label:<34}");
    for rate in RATES {
        print!(
            " {:>7.2} {:>7.2} |",
            worst_error(Method::Bilinear, band, rate),
            worst_error(Method::Matched, band, rate)
        );
    }
    println!();
}

fn band(shape: Shape, freq: f64, gain_db: f64, q: f64, slope: Slope) -> Design {
    Design {
        shape,
        slope,
        freq,
        gain_db,
        q,
    }
}

fn main() {
    println!("Largest error to 20 kHz, dB (digital − analogue): bilinear, matched");
    println!(
        "{:<34} {:^17}| {:^17}| {:^17}|",
        "", "44.1 kHz", "48 kHz", "96 kHz"
    );
    for freq in [1_000.0, 5_000.0, 10_000.0, 15_000.0, 18_000.0] {
        row(
            &format!("bell +12 dB Q 1 at {freq} Hz"),
            band(Shape::Bell, freq, 12.0, 1.0, Slope::Db12),
        );
    }
    for (gain, q) in [(-12.0, 1.0), (18.0, 4.0), (-24.0, 8.0), (30.0, 0.3)] {
        row(
            &format!("bell {gain:+} dB Q {q} at 12 kHz"),
            band(Shape::Bell, 12_000.0, gain, q, Slope::Db12),
        );
    }
    for shape in [Shape::LowShelf, Shape::HighShelf] {
        for (freq, gain) in [
            (5_000.0, 12.0),
            (10_000.0, 12.0),
            (15_000.0, -12.0),
            (10_000.0, 24.0),
        ] {
            row(
                &format!("{shape:?} {gain:+} dB at {freq} Hz"),
                band(
                    shape,
                    freq,
                    gain,
                    std::f64::consts::FRAC_1_SQRT_2,
                    Slope::Db12,
                ),
            );
        }
    }
    for shape in [Shape::LowCut, Shape::HighCut] {
        for (freq, slope) in [
            (5_000.0, Slope::Db12),
            (12_000.0, Slope::Db6),
            (12_000.0, Slope::Db18),
            (15_000.0, Slope::Db48),
            (18_000.0, Slope::Db96),
        ] {
            row(
                &format!("{shape:?} {} dB/oct at {freq} Hz", slope.db_per_octave()),
                band(shape, freq, 0.0, std::f64::consts::FRAC_1_SQRT_2, slope),
            );
        }
        row(
            &format!("{shape:?} 24 dB/oct Q 4 at 12 kHz"),
            band(shape, 12_000.0, 0.0, 4.0, Slope::Db24),
        );
    }
    for (freq, q) in [(10_000.0, 2.0), (15_000.0, 8.0)] {
        row(
            &format!("notch Q {q} at {freq} Hz"),
            band(Shape::Notch, freq, 0.0, q, Slope::Db12),
        );
        row(
            &format!("band pass Q {q} at {freq} Hz"),
            band(Shape::BandPass, freq, 0.0, q, Slope::Db12),
        );
    }
    for (freq, gain) in [(1_000.0, 6.0), (5_000.0, -9.0), (10_000.0, 12.0)] {
        row(
            &format!("tilt {gain:+} dB at {freq} Hz"),
            band(Shape::TiltShelf, freq, gain, 1.0, Slope::Db12),
        );
    }

    println!();
    println!("Worst over 20 Hz–16 kHz, Q 0.1–10, ±30 dB, every slope (dB, and where)");
    for rate in [44_100.0, 48_000.0] {
        for shape in Shape::ALL {
            let (bilinear, _) = grid_worst(Method::Bilinear, shape, rate);
            let (matched, at) = grid_worst(Method::Matched, shape, rate);
            println!(
                "{rate:>7} {shape:<10?} bilinear {bilinear:>7.2}   matched {matched:>6.2} at {} Hz, Q {:.2}, {:+} dB, {} dB/oct",
                at.freq.round(),
                at.q,
                at.gain_db,
                at.slope.db_per_octave()
            );
        }
    }
}
