//! How long a band keeps the engine awake after its input stops: a tenth of a second of noise,
//! then silence until [`Engine::is_ringing`] turns false, at 48 kHz.
//!
//! ```text
//! cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_tail
//! ```
//!
//! The tail ends when every state is below the engine's `QUIET_FLOOR` with silence in. A band's
//! ring time is about 2.2·Q_pole/f₀ to −60 dB (research §8.3); the floor is −200 dB, so the
//! figures here are about 3.3 ring times.

use mxm_fx_eq_n_dsp::{Band, Engine, Shape, Slope};

fn main() {
    let rate = 48_000.0;
    let cases = [
        (
            "bell 1 kHz Q 1 +6 dB",
            Shape::Bell,
            1_000.0,
            1.0,
            6.0,
            Slope::Db12,
        ),
        (
            "bell 100 Hz Q 4 +12 dB",
            Shape::Bell,
            100.0,
            4.0,
            12.0,
            Slope::Db12,
        ),
        (
            "bell 20 Hz Q 10 +12 dB",
            Shape::Bell,
            20.0,
            10.0,
            12.0,
            Slope::Db12,
        ),
        (
            "bell 10 Hz Q 40 +30 dB",
            Shape::Bell,
            10.0,
            40.0,
            30.0,
            Slope::Db12,
        ),
        (
            "low shelf 60 Hz Q 0.7 +12 dB",
            Shape::LowShelf,
            60.0,
            0.7,
            12.0,
            Slope::Db12,
        ),
        (
            "low cut 30 Hz 96 dB/oct",
            Shape::LowCut,
            30.0,
            0.707,
            0.0,
            Slope::Db96,
        ),
        (
            "high cut 200 Hz 96 dB/oct Q 4",
            Shape::HighCut,
            200.0,
            4.0,
            0.0,
            Slope::Db96,
        ),
        (
            "notch 50 Hz Q 40",
            Shape::Notch,
            50.0,
            40.0,
            0.0,
            Slope::Db12,
        ),
        (
            "band pass 20 Hz Q 40",
            Shape::BandPass,
            20.0,
            40.0,
            0.0,
            Slope::Db12,
        ),
    ];
    for (label, shape, freq, q, gain_db, slope) in cases {
        let mut engine = Engine::new(rate);
        engine.set_band(
            0,
            Band {
                shape: Some(shape),
                bypass: false,
                freq,
                gain_db,
                q,
                slope,
            },
        );
        engine.reset();
        let mut seed = 1u64;
        let mut burst: Vec<f32> = (0..4_800)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                ((seed >> 40) as f32 / (1u32 << 23) as f32 - 1.0) * 0.5
            })
            .collect();
        engine.process_mono(&mut burst);
        let block = 480;
        let mut silence = vec![0.0f32; block];
        let mut elapsed = 0usize;
        while engine.is_ringing() && elapsed < 3_600 * 48_000 {
            silence.fill(0.0);
            engine.process_mono(&mut silence);
            elapsed += block;
        }
        println!("{label:<32} quiet after {:>8.3} s", elapsed as f64 / rate);
    }
}
