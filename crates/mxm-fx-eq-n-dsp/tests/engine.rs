//! The engine against plan §8: bit-exact dry, the drawn curve heard, click-free transitions,
//! parking, solo, the gain scale, tails and numeric recovery.

use mxm_fx_eq_n_dsp::engine::{OUTPUT_BOUND, QUIET_FLOOR};
use mxm_fx_eq_n_dsp::filter::Filter;
use mxm_fx_eq_n_dsp::{Band, Design, Engine, SLOTS, Shape, Slope, design};
use mxm_measure::observe::worst_step;
use std::f64::consts::{PI, TAU};

fn band(shape: Shape, freq: f32, gain_db: f32, q: f32, slope: Slope) -> Band {
    Band {
        shape: Some(shape),
        bypass: false,
        freq,
        gain_db,
        q,
        slope,
    }
}

fn bell(freq: f32, gain_db: f32, q: f32) -> Band {
    band(Shape::Bell, freq, gain_db, q, Slope::Db12)
}

fn design_of(band: Band, scale: f64) -> Design {
    let shape = band.shape.unwrap();
    Design {
        shape,
        slope: band.slope,
        freq: f64::from(band.freq),
        gain_db: if shape.has_gain() {
            f64::from(band.gain_db) * scale
        } else {
            0.0
        },
        q: f64::from(band.q),
    }
}

/// A deterministic noise source.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

fn sine(n: usize, freq: f64, rate: f64, amplitude: f64) -> Vec<f32> {
    (0..n)
        .map(|i| (amplitude * (TAU * freq * i as f64 / rate).sin()) as f32)
        .collect()
}

/// Runs `input` through the engine in a host's uneven blocks, calling `before` with the frame
/// index ahead of each block.
fn run(engine: &mut Engine, input: &[f32], mut before: impl FnMut(&mut Engine, usize)) -> Vec<f32> {
    let mut output = input.to_vec();
    let sizes = [512, 37, 128, 1, 256, 64, 999];
    let mut start = 0;
    let mut which = 0;
    while start < output.len() {
        let end = (start + sizes[which % sizes.len()]).min(output.len());
        before(engine, start);
        engine.process_mono(&mut output[start..end]);
        start = end;
        which += 1;
    }
    output
}

#[test]
fn an_empty_eq_is_bit_exact_dry_on_noise_silence_and_nan() {
    let mut noise = Noise(7);
    let mut input: Vec<f32> = (0..20_000).map(|_| noise.next()).collect();
    input[5_000..6_000].fill(0.0);
    input[7_777] = f32::NAN;
    input[8_888] = f32::INFINITY;
    input[9_999] = f32::MIN_POSITIVE / 4.0;

    let bitwise = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());

    // All Off, at any gain scale.
    for scale in [0.0, 0.4, 1.0, 2.0] {
        let mut engine = Engine::new(48_000.0);
        engine.set_gain_scale(scale);
        engine.reset();
        let mut left = input.clone();
        let mut right = input.clone();
        engine.process_stereo(&mut left, &mut right);
        assert!(
            bitwise(&left, &input) && bitwise(&right, &input),
            "Off at scale {scale}"
        );
        assert!(engine.is_dry());
    }

    // Every slot a gain shape at 0 dB: parked, the identity, untouched.
    let mut engine = Engine::new(48_000.0);
    for slot in 0..SLOTS {
        let shape = [
            Shape::Bell,
            Shape::LowShelf,
            Shape::HighShelf,
            Shape::TiltShelf,
        ][slot % 4];
        engine.set_band(
            slot,
            band(
                shape,
                30.0 + 700.0 * slot as f32,
                0.0,
                0.3 + slot as f32,
                Slope::Db12,
            ),
        );
    }
    engine.reset();
    assert!(engine.is_dry());
    let output = run(&mut engine, &input, |_, _| {});
    assert!(bitwise(&output, &input), "24 parked bands");

    // Every slot boosting, at a gain scale of 0 %: flat, so parked too.
    let mut engine = Engine::new(48_000.0);
    for slot in 0..SLOTS {
        engine.set_band(slot, bell(100.0 + 500.0 * slot as f32, 12.0, 2.0));
    }
    engine.set_gain_scale(0.0);
    engine.reset();
    let output = run(&mut engine, &input, |_, _| {});
    assert!(bitwise(&output, &input), "gain scale 0 %");
}

#[test]
fn a_band_turned_off_or_bypassed_fades_out_and_then_costs_nothing() {
    let input = sine(48_000, 220.0, 48_000.0, 0.5);
    for change in [
        |band: &mut Band| band.shape = None,
        |band: &mut Band| band.bypass = true,
    ] {
        for start in [
            bell(220.0, 9.0, 2.0),
            band(Shape::LowCut, 300.0, 0.0, 0.7, Slope::Db48),
        ] {
            let mut engine = Engine::new(48_000.0);
            engine.set_band(3, start);
            engine.reset();
            assert!(!engine.is_dry());
            let mut changed = start;
            change(&mut changed);
            let output = run(&mut engine, &input, |engine, frame| {
                if frame >= 4_800 {
                    engine.set_band(3, changed);
                }
            });
            assert!(engine.is_dry(), "{start:?} → {changed:?}");
            // Long after the fade the buffer is the input, bit for bit.
            assert!(
                output[24_000..]
                    .iter()
                    .zip(&input[24_000..])
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
        }
    }
}

/// The steady-state gain at each whole-hertz `freqs` of the engine set up by `setup`, measured
/// from a multitone over one second after `settle` seconds.
fn heard_db(rate: f64, freqs: &[f64], setup: impl Fn(&mut Engine)) -> Vec<f64> {
    heard_db_settled(rate, freqs, 1, setup)
}

fn heard_db_settled(
    rate: f64,
    freqs: &[f64],
    settle: usize,
    setup: impl Fn(&mut Engine),
) -> Vec<f64> {
    let n = rate as usize;
    let amplitude = 0.5 / freqs.len() as f64;
    let input: Vec<f32> = (0..(settle + 1) * n)
        .map(|i| {
            freqs
                .iter()
                .map(|f| amplitude * (TAU * f * i as f64 / rate).sin())
                .sum::<f64>() as f32
        })
        .collect();
    let mut engine = Engine::new(rate);
    setup(&mut engine);
    engine.reset();
    let mut left = input.clone();
    let mut right = input;
    engine.process_stereo(&mut left, &mut right);
    assert_eq!(left, right, "both channels alike");
    freqs
        .iter()
        .map(|f| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, &y) in left[settle * n..].iter().enumerate() {
                let phase = TAU * f * (settle * n + i) as f64 / rate;
                re += f64::from(y) * phase.cos();
                im += f64::from(y) * phase.sin();
            }
            20.0 * (2.0 * re.hypot(im) / n as f64 / amplitude).log10()
        })
        .collect()
}

#[test]
fn what_is_heard_is_the_curve_the_editor_draws() {
    let freqs = [40.0, 150.0, 700.0, 2_100.0, 6_300.0, 13_000.0];
    let configurations: [&[Band]; 4] = [
        &[
            bell(700.0, 9.0, 1.5),
            band(Shape::LowCut, 60.0, 0.0, 0.7, Slope::Db24),
        ],
        &[
            band(Shape::HighShelf, 8_000.0, -6.0, 1.1, Slope::Db12),
            band(Shape::Notch, 2_100.0, 0.0, 3.0, Slope::Db12),
        ],
        &[
            band(Shape::TiltShelf, 1_000.0, 4.0, 1.0, Slope::Db12),
            band(Shape::HighCut, 12_000.0, 0.0, 2.0, Slope::Db18),
        ],
        &[
            band(Shape::LowShelf, 200.0, 7.0, 0.7, Slope::Db12),
            band(Shape::BandPass, 2_000.0, 0.0, 0.4, Slope::Db12),
            bell(40.0, -12.0, 4.0),
        ],
    ];
    for rate in [44_100.0, 96_000.0] {
        for bands in configurations {
            let heard = heard_db(rate, &freqs, |engine| {
                for (slot, band) in bands.iter().enumerate() {
                    engine.set_band(slot * 5, *band);
                }
                engine.set_gain_scale(0.8);
                engine.set_output_gain(0.5);
            });
            for (freq, heard) in freqs.iter().zip(heard) {
                let drawn: f64 = bands
                    .iter()
                    .map(|band| design(design_of(*band, 0.8), rate).response_db(*freq, rate))
                    .sum::<f64>()
                    + 20.0 * 0.5f64.log10();
                if drawn > -60.0 {
                    assert!(
                        (heard - drawn).abs() < 0.02,
                        "{bands:?} at {rate}, {freq} Hz: drew {drawn}, heard {heard}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_saved_eq_is_heard_from_the_first_sample() {
    let saved = bell(1_000.0, 12.0, 2.0);
    let mut engine = Engine::new(48_000.0);
    engine.set_band(0, saved);
    engine.set_gain_scale(0.5);
    engine.set_output_gain(2.0);
    engine.reset();
    let mut impulse = vec![0.0f32; 64];
    impulse[0] = 1.0;
    let mut expected = Filter::fresh(&design(design_of(saved, 0.5), 48_000.0));
    let mut heard = impulse.clone();
    engine.process_mono(&mut heard);
    for (i, &x) in impulse.iter().enumerate() {
        let [want] = expected.process([f64::from(x)]);
        assert_eq!(heard[i], (2.0 * want) as f32, "sample {i}");
    }
}

/// The largest step between samples, over the largest a sine of the output's own peak could make.
fn click_ratio(output: &[f32], freq: f64, rate: f64) -> f64 {
    let peak = output.iter().fold(0.0f32, |m, x| m.max(x.abs())) as f64;
    let (_, step) = worst_step(output).expect("finite");
    step / (peak * TAU * freq / rate)
}

#[test]
fn structural_changes_faster_than_the_fade_never_click() {
    // A 100 Hz tone through bands around it, while shapes, slopes, bypass and Off change every
    // 7 ms — faster than the 20 ms fade, so changes wait and the newest wins. At 96 and 192 kHz
    // a ramping section derives its coefficients every second and fourth sample.
    for rate in [48_000.0, 96_000.0, 192_000.0] {
        let input = sine((2.0 * rate) as usize, 100.0, rate, 0.25);
        let period = (0.007 * rate) as usize;
        let shapes = [
            Shape::Bell,
            Shape::LowCut,
            Shape::Notch,
            Shape::LowShelf,
            Shape::HighCut,
            Shape::TiltShelf,
            Shape::BandPass,
            Shape::HighShelf,
        ];
        let mut engine = Engine::new(rate);
        engine.set_band(0, bell(140.0, 9.0, 1.0));
        engine.set_band(1, band(Shape::LowShelf, 90.0, -6.0, 0.7, Slope::Db12));
        engine.reset();
        let mut step = 0usize;
        let output = run(&mut engine, &input, |engine, frame| {
            while frame >= step * period {
                let shape = shapes[step % shapes.len()];
                let mut changed = band(
                    shape,
                    80.0 + 40.0 * (step % 5) as f32,
                    8.0,
                    1.2,
                    Slope::ALL[step % 8],
                );
                changed.bypass = step % 7 == 3;
                if step % 11 == 5 {
                    changed.shape = None;
                }
                engine.set_band(step % 3, changed);
                step += 1;
            }
        });
        let ratio = click_ratio(&output[(0.05 * rate) as usize..], 100.0, rate);
        // A steady tone's ratio is 1; the fades' modulation stays near it. Undoing the fades
        // (every change a switch) measures well over 2.
        assert!(
            ratio < 1.6,
            "{rate} Hz: a step {ratio}× the largest a tone of this peak makes"
        );
    }
}

#[test]
fn a_dc_input_passes_exactly_through_jumping_bells_notches_and_cuts() {
    // DC through a trapezoidal SVF does not depend on its g and k (Wishnick 2014), and every
    // shape here has unity DC gain by construction, so no parameter jump may move it.
    let rate = 48_000.0;
    let input = vec![0.5f32; 48_000];
    let mut engine = Engine::new(rate);
    let mut flip = false;
    let output = run(&mut engine, &input, |engine, frame| {
        if frame % 2_048 < 600 {
            flip = !flip;
        }
        let (freq, gain, q) = if flip {
            (200.0, 12.0, 4.0)
        } else {
            (4_000.0, -12.0, 0.5)
        };
        engine.set_band(0, bell(freq, gain, q));
        engine.set_band(1, band(Shape::Notch, freq * 1.5, 0.0, q, Slope::Db12));
        engine.set_band(2, band(Shape::HighCut, freq * 4.0, 0.0, q, Slope::Db18));
        engine.set_band(3, band(Shape::HighShelf, freq, gain, 1.0 / q, Slope::Db12));
    });
    // Past the first entry fades and the shapes' ringing-in.
    for (i, &y) in output.iter().enumerate().skip(12_000) {
        assert!((f64::from(y) - 0.5).abs() < 1.0e-6, "sample {i}: {y}");
    }
}

#[test]
fn square_wave_frequency_modulation_stays_bounded() {
    // The research's case (§4.4): a Q 8, +18 dB bell switched between 300 Hz and 3 kHz every 64
    // samples. The SVF peaked at 1.29× unsmoothed; the glides keep it lower still.
    let rate = 48_000.0;
    let mut noise = Noise(3);
    let input: Vec<f32> = (0..96_000).map(|_| 0.05 * noise.next()).collect();
    let mut engine = Engine::new(rate);
    let output = run(&mut engine, &input, |engine, frame| {
        let freq = if (frame / 64) % 2 == 0 {
            300.0
        } else {
            3_000.0
        };
        engine.set_band(0, bell(freq, 18.0, 8.0));
    });
    let peak = output.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    // Static, the bell's peak gain is 7.94; noise at 0.05 rarely reaches 3× its RMS.
    assert!(
        output.iter().all(|x| x.is_finite()) && peak < 0.05 * 7.94 * 3.0,
        "{peak}"
    );
}

#[test]
fn a_20_hz_q_10_bell_at_192_khz_sounds_as_designed() {
    // Pole Q 20 at 20 Hz is a 0.32 s time constant: six seconds settle it to −80 dB.
    let heard = heard_db_settled(192_000.0, &[20.0, 40.0], 6, |engine| {
        engine.set_band(0, bell(20.0, 12.0, 10.0))
    });
    assert!((heard[0] - 12.0).abs() < 0.01, "{}", heard[0]);
    let drawn =
        design(design_of(bell(20.0, 12.0, 10.0), 1.0), 192_000.0).response_db(40.0, 192_000.0);
    assert!((heard[1] - drawn).abs() < 0.01);
}

#[test]
fn a_ringing_band_keeps_the_engine_alive_and_then_lets_it_sleep_on_exact_silence() {
    let rate = 48_000.0;
    let mut engine = Engine::new(rate);
    engine.set_band(0, bell(40.0, 12.0, 20.0));
    engine.reset();
    let mut burst = sine(4_800, 40.0, rate, 0.8);
    engine.process_mono(&mut burst);
    assert!(engine.is_ringing());
    let mut quiet_after = None;
    let mut last = vec![];
    for second in 0..120 {
        let mut silence = vec![0.0f32; 48_000];
        engine.process_mono(&mut silence);
        if !engine.is_ringing() {
            quiet_after = Some(second);
            last = silence;
            break;
        }
    }
    let quiet_after = quiet_after.expect("the tail ends");
    // Q 20 at +12 dB is pole Q 80: a 0.64 s time constant, so −200 dB takes about 30 s.
    assert!(quiet_after > 5 && quiet_after < 60, "{quiet_after} s");
    let mut after = vec![0.0f32; 4_800];
    engine.process_mono(&mut after);
    assert!(
        after.iter().all(|x| x.to_bits() == 0),
        "exact zeros, no subnormals"
    );
    let _ = last;
    let _ = QUIET_FLOOR;
}

#[test]
fn a_reset_leaves_no_tail() {
    let mut engine = Engine::new(48_000.0);
    engine.set_band(0, bell(60.0, 18.0, 30.0));
    engine.set_band(1, band(Shape::HighCut, 900.0, 0.0, 6.0, Slope::Db96));
    engine.reset();
    let mut noise = Noise(11);
    let mut audio: Vec<f32> = (0..9_600).map(|_| noise.next()).collect();
    engine.process_mono(&mut audio);
    assert!(engine.is_ringing());
    engine.reset();
    assert!(!engine.is_ringing());
    let mut silence = vec![0.0f32; 9_600];
    engine.process_mono(&mut silence);
    assert!(silence.iter().all(|x| x.to_bits() == 0));
}

/// A random index below `n`.
fn pick(noise: &mut Noise, n: usize) -> usize {
    ((noise.next().abs() * n as f32) as usize).min(n - 1)
}

/// `sane`, or now and then a value no parameter should ever carry.
fn wild(noise: &mut Noise, sane: f32) -> f32 {
    let values = [f32::NAN, f32::INFINITY, -1.0e30, 0.0, 1.0e-30];
    if noise.next() > 0.8 {
        values[pick(noise, values.len())]
    } else {
        sane
    }
}

#[test]
fn hostile_settings_and_input_stay_finite_bounded_and_recover() {
    let mut noise = Noise(99);
    for rate in [1_000.0, 44_100.0, 192_000.0, 768_000.0, f64::NAN, 0.0] {
        let mut engine = Engine::new(rate);
        let mut buffer = vec![0.0f32; 333];
        for block in 0..600 {
            for slot in 0..SLOTS {
                if (block + slot) % 9 == 0 {
                    let freq = 10.0 * 3_000f32.powf(noise.next().abs());
                    let gain_db = 30.0 * noise.next();
                    let q = 0.025 * 1_600f32.powf(noise.next().abs());
                    engine.set_band(
                        slot,
                        Band {
                            shape: if noise.next() > -0.6 {
                                Some(Shape::ALL[pick(&mut noise, 8)])
                            } else {
                                None
                            },
                            bypass: noise.next() > 0.7,
                            freq: wild(&mut noise, freq),
                            gain_db: wild(&mut noise, gain_db),
                            q: wild(&mut noise, q),
                            slope: Slope::ALL[pick(&mut noise, 8)],
                        },
                    );
                }
            }
            engine.set_solo((noise.next() > 0.0).then(|| (noise.next().abs() * 30.0) as usize));
            engine.set_gain_scale(if block % 50 == 0 {
                f32::NAN
            } else {
                1.0 + noise.next()
            });
            engine.set_output_gain(if block % 70 == 0 {
                f32::INFINITY
            } else {
                1.0 + noise.next()
            });
            for sample in &mut buffer {
                *sample = noise.next();
            }
            if block % 40 == 7 {
                buffer[17] = f32::NAN;
                buffer[18] = f32::NEG_INFINITY;
            }
            engine.process_mono(&mut buffer);
            for sample in &buffer {
                assert!(
                    sample.is_finite() && f64::from(sample.abs()) <= OUTPUT_BOUND,
                    "{sample} at {rate}"
                );
            }
        }
        // And a plain band afterwards still works.
        let mut engine_ok = engine;
        for slot in 0..SLOTS {
            engine_ok.set_band(slot, Band::OFF);
        }
        engine_ok.set_solo(None);
        engine_ok.set_gain_scale(1.0);
        engine_ok.set_output_gain(1.0);
        engine_ok.set_band(0, bell(300.0, 6.0, 1.0));
        // A fifth of a second: past every fade the hostile bands left running.
        let rate_used = engine_ok.sample_rate();
        let mut tone = sine((0.2 * rate_used) as usize, 300.0, rate_used, 0.5);
        engine_ok.process_mono(&mut tone);
        assert!(tone.iter().any(|x| x.abs() > 0.1), "recovered at {rate}");
    }
}

#[test]
fn a_parked_band_comes_back_like_a_new_one() {
    // Twenty-four 0 dB bells cost nothing; moving one's gain brings it in click-free, and it then
    // sounds exactly as designed.
    let rate = 48_000.0;
    let mut engine = Engine::new(rate);
    for slot in 0..SLOTS {
        engine.set_band(slot, bell(100.0 * (slot + 1) as f32, 0.0, 4.0));
    }
    engine.reset();
    assert!(engine.is_dry());
    let input = sine(48_000, 300.0, rate, 0.25);
    let output = run(&mut engine, &input, |engine, frame| {
        if frame >= 9_600 {
            engine.set_band(2, bell(300.0, 12.0, 4.0));
        }
    });
    assert!(!engine.is_dry());
    let ratio = click_ratio(&output, 300.0, rate);
    assert!(ratio < 1.2, "{ratio}");
    // Settled: the 300 Hz tone is boosted by the designed 12 dB.
    let tail = &output[40_000..];
    let peak = tail.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let gain = 20.0 * (f64::from(peak) / 0.25).log10();
    assert!((gain - 12.0).abs() < 0.05, "{gain}");
}

#[test]
fn the_gain_scale_flattens_gains_and_leaves_cuts_as_drawn() {
    let rate = 48_000.0;
    let heard = heard_db(rate, &[100.0, 1_000.0], |engine| {
        engine.set_band(0, bell(1_000.0, 12.0, 1.0));
        engine.set_band(1, band(Shape::LowCut, 200.0, 0.0, 0.7, Slope::Db24));
        engine.set_gain_scale(0.0);
    });
    let cut = design(
        design_of(band(Shape::LowCut, 200.0, 0.0, 0.7, Slope::Db24), 0.0),
        rate,
    );
    assert!(
        (heard[1] - cut.response_db(1_000.0, rate)).abs() < 0.01,
        "the bell is flat"
    );
    assert!(
        (heard[0] - cut.response_db(100.0, rate)).abs() < 0.02,
        "the cut is as drawn"
    );
}

/// The gain of `output` at a whole-hertz `freq` relative to `amplitude`, over its last second.
fn tone_db(output: &[f32], freq: f64, rate: f64, amplitude: f64) -> f64 {
    let n = rate as usize;
    let start = output.len() - n;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, &y) in output[start..].iter().enumerate() {
        let phase = TAU * freq * (start + i) as f64 / rate;
        re += f64::from(y) * phase.cos();
        im += f64::from(y) * phase.sin();
    }
    20.0 * (2.0 * re.hypot(im) / n as f64 / amplitude).log10()
}

#[test]
fn solo_auditions_a_0_db_bell_as_a_band_pass_and_nothing_for_a_tilt_or_a_bypassed_band() {
    let rate = 48_000.0;
    let probe = |setup: Band, freq: f64| {
        let mut engine = Engine::new(rate);
        engine.set_band(4, setup);
        engine.set_solo(Some(4));
        engine.reset();
        engine.set_solo(Some(4));
        let input = sine(96_000, freq, rate, 0.25);
        let output = run(&mut engine, &input, |_, _| {});
        (tone_db(&output, freq, rate, 0.25), engine.audition())
    };
    // The owner's narrow sweep: a 0 dB bell, soloed, is a band pass at its frequency and Q.
    let (at, heard) = probe(bell(1_000.0, 0.0, 8.0), 1_000.0);
    assert!(at.abs() < 0.01 && heard == Some(4), "{at} {heard:?}");
    let (off, _) = probe(bell(1_000.0, 0.0, 8.0), 3_000.0);
    assert!(off < -20.0, "{off}");
    // A tilt offers none, and a bypassed band neither: the EQ is heard.
    let (tilt, heard) = probe(
        band(Shape::TiltShelf, 1_000.0, 6.0, 1.0, Slope::Db12),
        3_000.0,
    );
    let tilt_drawn = design(
        design_of(band(Shape::TiltShelf, 1_000.0, 6.0, 1.0, Slope::Db12), 1.0),
        rate,
    )
    .response_db(3_000.0, rate);
    assert!((tilt - tilt_drawn).abs() < 0.01 && heard.is_none());
    let mut bypassed = bell(1_000.0, 0.0, 8.0);
    bypassed.bypass = true;
    let (dry, heard) = probe(bypassed, 3_000.0);
    assert!(dry.abs() < 0.01 && heard.is_none());
}

#[test]
fn solo_fades_in_and_out_and_a_soloed_band_can_change_shape_without_clicking() {
    let rate = 48_000.0;
    let mut engine = Engine::new(rate);
    engine.set_band(0, bell(1_000.0, 0.0, 4.0));
    engine.reset();
    let input = sine(96_000, 300.0, rate, 0.5);
    let output = run(&mut engine, &input, |engine, frame| {
        let soloed = (9_600..60_000).contains(&frame);
        engine.set_solo(soloed.then_some(0));
        if frame >= 30_000 {
            engine.set_band(0, band(Shape::HighCut, 1_000.0, 0.0, 0.7, Slope::Db96));
        }
    });
    assert!(engine.audition().is_none());
    let ratio = click_ratio(&output, 300.0, rate);
    assert!(ratio < 1.3, "{ratio}");
}

#[test]
fn mono_and_stereo_agree() {
    let mut noise = Noise(5);
    let input: Vec<f32> = (0..20_000).map(|_| noise.next()).collect();
    let setup = |engine: &mut Engine| {
        engine.set_band(0, bell(500.0, 6.0, 2.0));
        engine.set_band(9, band(Shape::HighCut, 7_000.0, 0.0, 0.9, Slope::Db36));
        engine.reset();
    };
    let mut mono_engine = Engine::new(44_100.0);
    setup(&mut mono_engine);
    let mut mono = input.clone();
    mono_engine.process_mono(&mut mono);
    let mut stereo_engine = Engine::new(44_100.0);
    setup(&mut stereo_engine);
    let (mut left, mut right) = (input.clone(), input);
    stereo_engine.process_stereo(&mut left, &mut right);
    assert_eq!(mono, left);
    assert_eq!(left, right);
    let _ = PI;
}

/// One automation event: at `frame`, apply it to the engine.
type Event = (usize, fn(&mut Engine));

/// Renders `input` in blocks of `sizes` (cycled), also cut at every event frame so each event
/// lands at the same sample whatever the partition.
fn render_partitioned(
    rate: f64,
    input: &[f32],
    events: &[Event],
    sizes: &[usize],
    stereo: bool,
) -> Vec<f32> {
    let mut engine = Engine::new(rate);
    engine.set_band(0, bell(300.0, 0.0, 1.0));
    engine.reset();
    let mut left = input.to_vec();
    let mut right: Vec<f32> = input.iter().map(|x| -x).collect();
    let mut start = 0;
    let mut which = 0;
    let mut next_event = 0;
    while start < left.len() {
        while next_event < events.len() && events[next_event].0 <= start {
            (events[next_event].1)(&mut engine);
            next_event += 1;
        }
        let mut end = (start + sizes[which % sizes.len()]).min(left.len());
        if next_event < events.len() {
            end = end.min(events[next_event].0.max(start + 1));
        }
        if stereo {
            engine.process_stereo(&mut left[start..end], &mut right[start..end]);
        } else {
            engine.process_mono(&mut left[start..end]);
        }
        start = end;
        which += 1;
    }
    left
}

#[test]
fn the_output_does_not_depend_on_how_the_host_splits_its_blocks() {
    let events: [Event; 8] = [
        (1_000, |e| e.set_band(0, bell(300.0, 12.0, 2.0))),
        (2_345, |e| {
            e.set_band(1, band(Shape::HighCut, 5_000.0, 0.0, 3.0, Slope::Db48))
        }),
        (2_346, |e| e.set_output_gain(0.7)),
        (4_000, |e| e.set_band(0, bell(1_300.0, -9.0, 0.5))),
        (5_555, |e| e.set_solo(Some(1))),
        (6_001, |e| e.set_gain_scale(1.6)),
        (9_000, |e| e.set_solo(None)),
        (11_111, |e| e.set_band(1, Band::OFF)),
    ];
    let mut noise = Noise(21);
    let input: Vec<f32> = (0..16_000).map(|_| 0.3 * noise.next()).collect();
    for rate in [44_100.0, 48_000.0] {
        for stereo in [false, true] {
            let whole = render_partitioned(rate, &input, &events, &[16_000], stereo);
            for sizes in [&[1][..], &[17][..], &[512, 3, 64, 1, 999, 15][..]] {
                let split = render_partitioned(rate, &input, &events, sizes, stereo);
                let same = whole
                    .iter()
                    .zip(&split)
                    .all(|(a, b)| a.to_bits() == b.to_bits());
                assert!(same, "{rate} Hz, stereo {stereo}, blocks {sizes:?}");
            }
        }
    }
}

#[test]
fn a_band_fading_out_never_jumps_at_the_end_of_its_fade() {
    // At 44.1 kHz the 882-sample fade is not a whole number of 15-sample control intervals, so
    // its last interval holds 12 of its samples, and the gain ramp must end with them. A +60 dB
    // low shelf at 5 kHz on a DC input follows its gain closely: the output slides smoothly down
    // to the input, and a ramp cut short would leave a final drop several times its own steps.
    let rate = 44_100.0;
    for off_at in [10_000, 10_007, 10_013] {
        let mut engine = Engine::new(rate);
        let shelf = band(Shape::LowShelf, 5_000.0, 30.0, 0.7, Slope::Db12);
        engine.set_band(0, shelf);
        engine.set_gain_scale(2.0);
        engine.reset();
        let mut output = vec![0.01f32; 20_000];
        engine.process_mono(&mut output[..off_at]);
        // Off as the plugin turns a band off: its type, every other value kept.
        engine.set_band(
            0,
            Band {
                shape: None,
                ..shelf
            },
        );
        engine.process_mono(&mut output[off_at..]);
        let end = output.iter().rposition(|&y| y != 0.01).expect("the fade") + 1;
        let step = |range: std::ops::Range<usize>| {
            output[range]
                .windows(2)
                .map(|w| (f64::from(w[1]) - f64::from(w[0])).abs())
                .fold(0.0f64, f64::max)
        };
        // The final step against the two intervals before it, whose linear steps are larger
        // than the last interval's because the ramp is linear in dB.
        let last = step(end - 3..end + 3);
        let before = step(end - 45..end - 15);
        assert!(
            last < 1.3 * before,
            "off at {off_at}: final step {last}, steps before it {before}"
        );
    }
}

#[test]
fn an_output_gain_change_starts_at_its_sample_even_on_an_empty_eq() {
    // An empty EQ leaves whole intervals untouched; an output-gain ramp must still begin at the
    // sample it was set at, at every offset into an interval, as it does with a band running.
    let rate = 48_000.0;
    let interval = Engine::new(rate).control_interval();
    for offset in 0..=interval {
        let mut engine = Engine::new(rate);
        engine.reset();
        let mut buffer = vec![0.5f32; 4 * interval];
        engine.process_mono(&mut buffer[..offset]);
        engine.set_output_gain(2.0);
        engine.process_mono(&mut buffer[offset..]);
        assert!(
            buffer[..offset].iter().all(|&x| x == 0.5),
            "offset {offset}: untouched before"
        );
        let first_step = 0.5 * (1.0 + 1.0 / engine.fade_frames() as f64);
        assert_eq!(
            buffer[offset], first_step as f32,
            "offset {offset}: the ramp's first sample"
        );
    }
}

#[test]
fn processed_output_is_never_subnormal() {
    // The engine works in f64, where 1e-40 is an ordinary number; cast to f32 it would be
    // subnormal, which a host must never be handed. A tiny signal through a deep cut lands there.
    // A −30 dB bell on its own centre takes a tone a little above f32's smallest normal number
    // to about 1.5e-39, inside the subnormal range.
    let mut engine = Engine::new(48_000.0);
    engine.set_band(0, bell(1_000.0, -30.0, 1.0));
    engine.reset();
    let mut buffer: Vec<f32> = sine(48_000, 1_000.0, 48_000.0, 1.0)
        .iter()
        .map(|x| x * f32::MIN_POSITIVE * 4.0)
        .collect();
    engine.process_mono(&mut buffer);
    assert!(
        buffer
            .iter()
            .all(|x| *x == 0.0 || x.abs() >= f32::MIN_POSITIVE),
        "a subnormal reached the output"
    );
    assert!(buffer.contains(&0.0), "the case does reach the flush");
}
