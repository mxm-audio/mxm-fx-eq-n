//! The audio-thread cost of the engine, against the brief's A6 budget: the worst v1
//! configuration, 24 bands each a 96 dB/octave cut (eight sections), stereo.
//!
//! ```text
//! cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_cost
//! ```
//!
//! **Run it on an idle machine** (`docs/code-review-notes.md` §3: a timing taken while something
//! else builds is not a measurement). Each figure is the best of several runs of ten seconds of
//! audio, as nanoseconds per stereo frame and as a share of the frame period, which is one core's
//! budget at that rate. The analyser's handoff is the plugin's, not this crate's, and is not in
//! these figures.
//!
//! - **steady**: every band settled.
//! - **all crossfading**: every band switched between a 96 dB/octave low cut and a 96 dB/octave
//!   high cut at every fade's end, in blocks of exactly one fade rounded up to whole control
//!   intervals, so every band always runs two eight-section filters (at 44.1 kHz three samples
//!   in each 885 are between fades).
//! - **all gliding**: every band's frequency retargeted every block, so all 24 redesign and ramp
//!   continuously.
//! - **all crossfading and gliding**, the transition worst case: the crossfading load with every
//!   band's frequency also retargeted every control interval, so both filter banks run while the
//!   new one redesigns and ramps.
//! - **24 parked bells**: twenty-four 0 dB bells, the identity; the budget's idle half.
//! - **8 bells**: a typical session.
//! - **24 dynamic bells** (plan §7.1, F1's worst case, held to the *steady* budget): every band a
//!   bell with Range −30, Threshold −40, Attack 0.5 ms and Release 10 ms, spread from 100 Hz up,
//!   on noise whose level drops 30 dB every other block, so every band's detector runs and its gain
//!   moves and is redesigned all the time.

use mxm_fx_eq_n_dsp::{Band, Dynamics, Engine, SLOTS, Shape, Slope};
use std::hint::black_box;
use std::time::Instant;

#[derive(Clone, Copy)]
enum Load {
    Steady,
    Crossfading,
    Gliding,
    CrossfadingGliding,
    Parked,
    Typical,
    Dynamic,
}

fn cut(freq: f32, slope: Slope) -> Band {
    shaped(Shape::HighCut, freq, slope)
}

fn shaped(shape: Shape, freq: f32, slope: Slope) -> Band {
    Band {
        shape: Some(shape),
        bypass: false,
        freq,
        gain_db: 0.0,
        q: 0.707,
        slope,
    }
}

fn measure(load: Load, rate: f64) -> f64 {
    let mut engine = Box::new(Engine::new(rate));
    for slot in 0..SLOTS {
        let band = match load {
            Load::Parked => Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq: 100.0 + 400.0 * slot as f32,
                gain_db: 0.0,
                q: 2.0,
                slope: Slope::Db12,
            },
            Load::Dynamic => Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq: 100.0 * 1.25f32.powi(slot as i32),
                gain_db: 3.0,
                q: 1.0,
                slope: Slope::Db12,
            },
            Load::Typical if slot >= 8 => Band::OFF,
            Load::Typical => Band {
                shape: Some(Shape::Bell),
                bypass: false,
                freq: 60.0 * (slot + 1) as f32 * 3.0,
                gain_db: 4.0 - slot as f32,
                q: 1.5,
                slope: Slope::Db12,
            },
            _ => cut(2_000.0 + 300.0 * slot as f32, Slope::Db96),
        };
        engine.set_band(slot, band);
        if let Load::Dynamic = load {
            engine.set_dynamics(
                slot,
                Dynamics {
                    range_db: -30.0,
                    threshold_db: -40.0,
                    attack_ms: 0.5,
                    release_ms: 10.0,
                },
            );
        }
    }
    engine.reset();
    let block = match load {
        // One fade, rounded up to whole control intervals, so each block starts a new fade at
        // a control boundary as the last one ends.
        Load::Crossfading | Load::CrossfadingGliding => {
            let interval = engine.control_interval();
            engine.fade_frames().div_ceil(interval) * interval
        }
        _ => 512,
    };
    let seconds = 10.0;
    let blocks = (rate * seconds / block as f64) as usize;
    let mut left = vec![0.0f32; block];
    let mut right = vec![0.0f32; block];
    let mut seed = 5u64;
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        *l = ((seed >> 40) as f32 / (1u32 << 23) as f32 - 1.0) * 0.25;
        *r = -*l;
    }
    let mut work_left = left.clone();
    let mut work_right = right.clone();
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let start = Instant::now();
        for index in 0..blocks {
            match load {
                Load::CrossfadingGliding => {
                    // Shape per block, frequency per control interval, through to the next block.
                    let shape = if index.is_multiple_of(2) {
                        Shape::LowCut
                    } else {
                        Shape::HighCut
                    };
                    let interval = engine.control_interval();
                    work_left.copy_from_slice(&left);
                    work_right.copy_from_slice(&right);
                    for (chunk, (l, r)) in work_left
                        .chunks_mut(interval)
                        .zip(work_right.chunks_mut(interval))
                        .enumerate()
                    {
                        let wobble = if chunk % 2 == 0 { 1.5 } else { 0.7 };
                        for slot in 0..SLOTS {
                            let freq = (2_000.0 + 300.0 * slot as f32) * wobble;
                            engine.set_band(slot, shaped(shape, freq, Slope::Db96));
                        }
                        engine.process_stereo(l, r);
                    }
                    black_box((&work_left, &work_right));
                    continue;
                }
                Load::Crossfading => {
                    let shape = if index.is_multiple_of(2) {
                        Shape::LowCut
                    } else {
                        Shape::HighCut
                    };
                    for slot in 0..SLOTS {
                        engine.set_band(
                            slot,
                            shaped(shape, 2_000.0 + 300.0 * slot as f32, Slope::Db96),
                        );
                    }
                }
                Load::Gliding => {
                    let wobble = if index % 2 == 0 { 1.5 } else { 0.7 };
                    for slot in 0..SLOTS {
                        engine.set_band(
                            slot,
                            cut((2_000.0 + 300.0 * slot as f32) * wobble, Slope::Db96),
                        );
                    }
                }
                _ => {}
            }
            work_left.copy_from_slice(&left);
            work_right.copy_from_slice(&right);
            if matches!(load, Load::Dynamic) && index % 2 == 1 {
                for sample in work_left.iter_mut().chain(work_right.iter_mut()) {
                    *sample *= 0.03;
                }
            }
            engine.process_stereo(&mut work_left, &mut work_right);
            black_box((&work_left, &work_right));
        }
        let per_frame = start.elapsed().as_secs_f64() / (blocks * block) as f64;
        best = best.min(per_frame);
    }
    best
}

fn main() {
    println!(
        "{:<18} {:>20} {:>20} {:>20} {:>20}",
        "load", "44.1 kHz", "48 kHz", "96 kHz", "192 kHz"
    );
    for (label, load) in [
        ("steady", Load::Steady),
        ("all crossfading", Load::Crossfading),
        ("all gliding", Load::Gliding),
        ("crossfade + glide", Load::CrossfadingGliding),
        ("24 parked bells", Load::Parked),
        ("8 bells", Load::Typical),
        ("24 dynamic bells", Load::Dynamic),
    ] {
        print!("{label:<18}");
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            let per_frame = measure(load, rate);
            print!(
                " {:>8.1} ns {:>7.1} %",
                per_frame * 1.0e9,
                per_frame * rate * 100.0
            );
        }
        println!();
    }
}
