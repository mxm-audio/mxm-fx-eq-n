//! Dynamic bands measured (plan §7.1; `research:effects/parametric-equalizers.md` §25.4).
//!
//! ```text
//! cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_dynamics
//! ```
//!
//! 1. **The detector's delay against the audio** (Wise's "conundrum", research §7): a tone at a
//!    dynamic cut's centre starts from silence; the time until the cut is half in, against the
//!    attack alone (0.5 ms, half in after `ln 2 · 0.5 ms`). The excess is the detector's band pass
//!    rising, roughly its group delay `2Q / ω₀`, and through it the onset passes uncut.
//! 2. **Broadband energy at the fastest attack**: a 200 Hz tone through a 1 kHz, Q 2 dynamic cut
//!    (Range −30, Threshold −40, Attack 0.5 ms, Release 10 ms) whose detector Hann-shaped 1 kHz
//!    bursts trigger every 100 ms; the energy above 5 kHz against the tone, with the dynamics and
//!    with the band static, so the difference is what the moving gain adds.

use mxm_fx_eq_n_dsp::{Band, Dynamics, Engine, Shape, Slope};
use mxm_measure::spectrum::fft;
use std::f64::consts::{PI, TAU};

const RATE: f64 = 48_000.0;

fn bell(freq: f32, gain_db: f32, q: f32) -> Band {
    Band {
        shape: Some(Shape::Bell),
        bypass: false,
        freq,
        gain_db,
        q,
        slope: Slope::Db12,
    }
}

/// The fraction of `signal`'s energy above `above` hertz against its energy below, in dB, from a
/// Hann-windowed transform.
fn energy_above_db(signal: &[f64], above: f64) -> f64 {
    let n = signal.len().next_power_of_two() / 2;
    let mut re: Vec<f64> = signal[..n]
        .iter()
        .enumerate()
        .map(|(i, x)| x * (0.5 - 0.5 * (TAU * i as f64 / n as f64).cos()))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im).expect("a power of two");
    let bin = |k: usize| re[k] * re[k] + im[k] * im[k];
    let edge = (above / RATE * n as f64) as usize;
    let high: f64 = (edge..n / 2).map(bin).sum();
    let low: f64 = (1..edge).map(bin).sum();
    10.0 * (high / low).log10()
}

fn detector_delay(freq: f32, q: f32) -> (f64, f64) {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(freq, 0.0, q));
    engine.set_dynamics(
        0,
        Dynamics {
            range_db: -12.0,
            threshold_db: -40.0,
            attack_ms: 0.5,
            release_ms: 150.0,
        },
    );
    engine.reset();
    let interval = engine.control_interval();
    let mut gains = Vec::new();
    let mut n = 0usize;
    for _ in 0..((2.0 * RATE) as usize / interval) {
        let mut block: Vec<f32> = (0..interval)
            .map(|i| (0.5 * (TAU * f64::from(freq) * (n + i) as f64 / RATE).sin()) as f32)
            .collect();
        n += interval;
        engine.process_mono(&mut block);
        gains.push(engine.gains_db(0).unwrap().1);
    }
    let steady = *gains.last().unwrap();
    let half = gains.iter().position(|gain| *gain <= steady / 2.0).unwrap();
    let measured = (half + 1) as f64 * interval as f64 / RATE * 1_000.0;
    let attack_alone = 0.5 * std::f64::consts::LN_2;
    let group_delay = 2.0 * f64::from(q) / (TAU * f64::from(freq)) * 1_000.0;
    (measured - attack_alone, group_delay)
}

fn broadband(dynamic: bool) -> f64 {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(1_000.0, 0.0, 2.0));
    engine.set_dynamics(
        0,
        Dynamics {
            range_db: if dynamic { -30.0 } else { 0.0 },
            threshold_db: -40.0,
            attack_ms: 0.5,
            release_ms: 10.0,
        },
    );
    engine.reset();
    let seconds = 4.0;
    let frames = (seconds * RATE) as usize;
    let burst = (0.010 * RATE) as usize;
    let period = (0.100 * RATE) as usize;
    let mut signal: Vec<f32> = (0..frames)
        .map(|i| {
            let tone = 0.25 * (TAU * 200.0 * i as f64 / RATE).sin();
            let within = i % period;
            let trigger = if within < burst {
                let w = 0.5 - 0.5 * (2.0 * PI * within as f64 / burst as f64).cos();
                0.5 * w * (TAU * 1_000.0 * i as f64 / RATE).sin()
            } else {
                0.0
            };
            (tone + trigger) as f32
        })
        .collect();
    for block in signal.chunks_mut(256) {
        engine.process_mono(block);
    }
    let output: Vec<f64> = signal[frames / 4..].iter().map(|x| f64::from(*x)).collect();
    energy_above_db(&output, 5_000.0)
}

fn main() {
    println!("1. The detector's delay against the audio, beyond a 0.5 ms attack:");
    for freq in [100.0f32, 1_000.0] {
        for q in [1.0f32, 4.0, 10.0, 40.0] {
            let (excess, group) = detector_delay(freq, q);
            println!(
                "   {freq:>6.0} Hz, Q {q:>4.0}: {excess:>6.2} ms later (band pass 2Q/ω₀ = {group:.2} ms)"
            );
        }
    }
    let with = broadband(true);
    let without = broadband(false);
    println!(
        "2. Energy above 5 kHz, fastest attack: {with:.1} dB with the dynamics, {without:.1} dB static, {:.1} dB added",
        with - without
    );
}
