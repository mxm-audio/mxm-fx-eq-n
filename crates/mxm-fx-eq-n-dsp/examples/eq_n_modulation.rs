//! A band under fast parameter changes (`research:effects/parametric-equalizers.md` §4.4, §25.1).
//!
//! ```text
//! cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_modulation
//! ```
//!
//! 1. **Square-wave frequency modulation**: a Q 8, +18 dB bell switched between 300 Hz and 3 kHz
//!    every 64 samples, on noise: the output's peak over the static peak gain. The research
//!    measured the unsmoothed SVF at 1.29×, DF1 2.22×, TDF2 5.17×.
//! 2. **Square-wave gain modulation**: ±15 dB every 64 samples on a 1 kHz, Q 2 bell, a 200 Hz
//!    tone in: the energy above 5 kHz against the tone. The research measured the SVF at
//!    −15.1 dB and DF1 at −23.6 dB, the SVF's known weak case.
//! 3. **Envelope-rate gain modulation**, for F1's dynamic bands: the same bell's gain following
//!    a 1 ms attack, 60 ms release envelope between 0 and −12 dB, retriggered every 100 ms, a 200
//!    Hz tone in. Energy above 5 kHz against the tone, for the shipped bell (its damping follows
//!    the gain, the half-gain Q) and for the fixed-core form, where only a mix gain moves
//!    (`1 + (G − 1)·band-pass` at fixed poles).
//!
//! "Switched" means new coefficients at that sample, no glide — the band's own behaviour, not the
//! engine's smoothing. Item 1 also runs through the engine, whose glides are part of the product.

use mxm_fx_eq_n_dsp::filter::Filter;
use mxm_fx_eq_n_dsp::{Band, Cascade, Design, Engine, Section, Shape, Slope, design};
use mxm_measure::spectrum::fft;
use std::f64::consts::TAU;

const RATE: f64 = 48_000.0;

fn bell(freq: f64, gain_db: f64, q: f64) -> Cascade {
    design(
        Design {
            shape: Shape::Bell,
            slope: Slope::Db12,
            freq,
            gain_db,
            q,
        },
        RATE,
    )
}

fn noise(n: usize) -> Vec<f64> {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    (0..n)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 40) as f64 / (1u64 << 23) as f64 - 1.0) * 0.1
        })
        .collect()
}

/// The fraction of `signal`'s energy above `above` hertz, in dB, from a Hann-windowed transform.
fn energy_above_db(signal: &[f64], above: f64) -> f64 {
    let n = signal.len().next_power_of_two() / 2;
    let mut re: Vec<f64> = signal[..n]
        .iter()
        .enumerate()
        .map(|(i, x)| x * (0.5 - 0.5 * (TAU * i as f64 / n as f64).cos()))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im).expect("a power of two");
    let cut = (above / RATE * n as f64) as usize;
    let power = |k: usize| re[k] * re[k] + im[k] * im[k];
    let high: f64 = (cut..n / 2).map(power).sum();
    let total: f64 = (1..n / 2).map(power).sum();
    10.0 * (high / total).log10()
}

/// A fixed-core bell: the poles of the 0 dB bell at `q`, the gain only in the band-pass mix,
/// `y = x + (G − 1)·k·bp`.
fn fixed_core(freq: f64, q: f64, gain: f64) -> Cascade {
    let Section::Second { g, k, .. } = bell(freq, 1.0e-9, q).sections()[0] else {
        unreachable!("a bell is one second-order section")
    };
    Cascade::from_sections(&[Section::Second {
        g,
        k,
        m0: 1.0,
        m1: k * (gain - 1.0),
        m2: 0.0,
    }])
}

fn main() {
    // 1. Square-wave frequency modulation.
    let input = noise(96_000);
    let static_peak = 10f64.powf(18.0 / 20.0);
    let low = bell(300.0, 18.0, 8.0);
    let high = bell(3_000.0, 18.0, 8.0);
    let mut filter = Filter::fresh(&low);
    let mut peak = 0.0f64;
    for (i, &x) in input.iter().enumerate() {
        if i % 64 == 0 {
            filter.load(if (i / 64) % 2 == 0 { &low } else { &high });
        }
        let [y] = filter.process([x]);
        peak = peak.max(y.abs());
    }
    let input_peak = input.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    // Static reference: the same noise through each bell alone.
    let mut static_out = 0.0f64;
    for cascade in [&low, &high] {
        let mut filter = Filter::fresh(cascade);
        for &x in &input {
            let [y] = filter.process([x]);
            static_out = static_out.max(y.abs());
        }
    }
    let mut engine = Engine::new(RATE);
    let mut buffer: Vec<f32> = input.iter().map(|&x| x as f32).collect();
    for (block, chunk) in buffer.chunks_mut(64).enumerate() {
        let freq = if block % 2 == 0 { 300.0 } else { 3_000.0 };
        engine.set_band(
            0,
            Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq,
                gain_db: 18.0,
                q: 8.0,
                slope: Slope::Db12,
            },
        );
        engine.process_mono(chunk);
    }
    let engine_peak = buffer.iter().fold(0.0f32, |m, x| m.max(x.abs())) as f64;
    println!(
        "1. Square-wave frequency modulation, Q 8 +18 dB bell, 300 Hz ↔ 3 kHz every 64 samples"
    );
    println!(
        "   switched SVF: peak {:.3}× the static bells' own peak on the same noise ({:.3} vs {:.3}; static gain {static_peak:.2})",
        peak / static_out,
        peak,
        static_out
    );
    println!(
        "   through the engine (glides): peak {:.3}× ({:.3}); input peak {input_peak:.3}",
        engine_peak / static_out,
        engine_peak
    );

    // 2. Square-wave gain modulation, switched.
    let n = 1 << 16;
    let tone: Vec<f64> = (0..n + 4_800)
        .map(|i| 0.25 * (TAU * 200.0 * i as f64 / RATE).sin())
        .collect();
    let up = bell(1_000.0, 15.0, 2.0);
    let down = bell(1_000.0, -15.0, 2.0);
    let fixed_up = fixed_core(1_000.0, 2.0, 10f64.powf(15.0 / 20.0));
    let fixed_down = fixed_core(1_000.0, 2.0, 10f64.powf(-15.0 / 20.0));
    for (label, a, b) in [
        ("shipped bell", &up, &down),
        ("fixed core", &fixed_up, &fixed_down),
    ] {
        let mut filter = Filter::fresh(a);
        let mut output = Vec::with_capacity(tone.len());
        for (i, &x) in tone.iter().enumerate() {
            if i % 64 == 0 {
                filter.load(if (i / 64) % 2 == 0 { a } else { b });
            }
            output.push(filter.process([x])[0]);
        }
        println!(
            "2. Square-wave gain ±15 dB every 64 samples, 1 kHz Q 2, {label}: energy above 5 kHz {:.1} dB",
            energy_above_db(&output[4_800..], 5_000.0)
        );
    }

    // 3. Envelope-rate gain modulation, per sample.
    let attack = (-1.0 / (0.001 * RATE)).exp();
    let release = (-1.0 / (0.060 * RATE)).exp();
    let mut envelope = 0.0f64;
    let depth_db = -12.0;
    let mut shipped = Filter::fresh(&bell(1_000.0, 1.0e-9, 2.0));
    let mut fixed = Filter::fresh(&fixed_core(1_000.0, 2.0, 1.0));
    let (mut out_shipped, mut out_fixed, mut out_static) = (vec![], vec![], vec![]);
    let mut static_filter = Filter::fresh(&bell(1_000.0, depth_db / 2.0, 2.0));
    for (i, &x) in tone.iter().enumerate() {
        let gate = (i % 4_800) < 480;
        let target = if gate { 1.0 } else { 0.0 };
        let coefficient = if target > envelope { attack } else { release };
        envelope = target + (envelope - target) * coefficient;
        let gain_db = depth_db * envelope;
        shipped.load(&bell(
            1_000.0,
            if gain_db == 0.0 { -1.0e-9 } else { gain_db },
            2.0,
        ));
        fixed.load(&fixed_core(1_000.0, 2.0, 10f64.powf(gain_db / 20.0)));
        out_shipped.push(shipped.process([x])[0]);
        out_fixed.push(fixed.process([x])[0]);
        out_static.push(static_filter.process([x])[0]);
    }
    println!(
        "3. Envelope-rate gain, 0 to −12 dB, 1 ms attack, 60 ms release, every 100 ms, 1 kHz Q 2, 200 Hz in"
    );
    for (label, output) in [
        ("shipped bell", &out_shipped),
        ("fixed core", &out_fixed),
        ("static −6 dB", &out_static),
    ] {
        println!(
            "   {label:<13} energy above 5 kHz {:.1} dB",
            energy_above_db(&output[4_800..], 5_000.0)
        );
    }
}
