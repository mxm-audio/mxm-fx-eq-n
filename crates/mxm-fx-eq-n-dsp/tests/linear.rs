//! Linear phase (plan §7.7): the kernel's magnitude and symmetry, placed bands exact, an empty EQ a
//! pure delay, the latency, a kernel change on the same past, offline determinism, the tail,
//! dynamics held and solo lined up.

use mxm_fx_eq_n_dsp::linear::Sizes;
use mxm_fx_eq_n_dsp::{
    Band, Design, Dynamics, Engine, Phase, Placement, Resolution, Shape, Slope, design,
};

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

/// White noise in [−0.5, 0.5] (xorshift), reproducible.
fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as f32 / u32::MAX as f32 - 0.5
        })
        .collect()
}

/// An engine in Linear phase at `resolution`, `setup` applied, primed, offline.
fn linear(resolution: Resolution, setup: impl Fn(&mut Engine)) -> Engine {
    let mut engine = Engine::with_phase(RATE, Phase::Linear(resolution), 2);
    setup(&mut engine);
    engine.prime();
    engine.reset();
    engine.set_offline(true);
    engine
}

/// `left` and `right` through `engine` in blocks of `block`, `each` called before each block with
/// its first sample's index.
fn run(
    engine: &mut Engine,
    left: &[f32],
    right: &[f32],
    block: usize,
    mut each: impl FnMut(&mut Engine, usize),
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (left.to_vec(), right.to_vec());
    let mut start = 0;
    while start < l.len() {
        let end = (start + block).min(l.len());
        each(engine, start);
        engine.process_stereo(&mut l[start..end], &mut r[start..end]);
        start = end;
    }
    (l, r)
}

/// The magnitude of `signal` at `freq` (a single-bin DFT), in dB.
fn level_db(signal: &[f32], freq: f64) -> f64 {
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (n, &x) in signal.iter().enumerate() {
        let phase = std::f64::consts::TAU * freq * n as f64 / RATE;
        re += f64::from(x) * phase.cos();
        im -= f64::from(x) * phase.sin();
    }
    20.0 * (re.hypot(im)).max(1.0e-30).log10()
}

/// **An empty EQ is the input delayed by the latency, bit for bit** (plan §7.7), at every
/// resolution.
#[test]
fn an_empty_eq_is_a_pure_delay() {
    for resolution in Resolution::ALL {
        let mut engine = linear(resolution, |_| {});
        let latency = engine.latency() as usize;
        assert_eq!(latency, Sizes::new(RATE, resolution).latency());
        let input = noise(3 * latency + 777, 3);
        let (l, r) = run(&mut engine, &input, &input, 512, |_, _| {});
        assert!(l[..latency].iter().all(|x| *x == 0.0));
        assert_eq!(
            &l[latency..],
            &input[..input.len() - latency],
            "{resolution:?}"
        );
        assert_eq!(l, r);
    }
}

/// **The latency** (plan §7.7): `N/2 + B`, the kernel's time kept across the rate families.
#[test]
fn the_latency_is_the_kernels_centre_and_a_partition() {
    let medium = |rate| Sizes::new(rate, Resolution::Medium);
    assert_eq!(medium(48_000.0).latency(), 4_096 + 256);
    assert_eq!(medium(44_100.0).latency(), 4_096 + 256);
    assert_eq!(medium(96_000.0).latency(), 8_192 + 512);
    assert_eq!(medium(192_000.0).latency(), 16_384 + 1_024);
    assert_eq!(Sizes::new(48_000.0, Resolution::Low).kernel, 4_096);
    assert_eq!(Sizes::new(48_000.0, Resolution::High).kernel, 16_384);
    for rate in [22_050.0, 32_000.0, 44_100.0, 88_200.0, 176_400.0] {
        let sizes = medium(rate);
        assert!(sizes.kernel.is_power_of_two() && sizes.block.is_power_of_two());
        assert_eq!(sizes.parts(), 32, "{rate}");
    }
    let engine = Engine::with_phase(RATE, Phase::ZeroLatency, 2);
    assert_eq!(engine.latency(), 0);
}

/// **The kernel is linear phase and hears the bands' magnitude** (plan §7.7): its impulse response
/// symmetric about the latency, and a band wider than eight bins heard within 0.25 dB of Zero
/// latency's magnitude.
#[test]
fn the_kernel_is_symmetric_and_hears_the_bands_magnitude() {
    for resolution in Resolution::ALL {
        let mut engine = linear(resolution, |engine| {
            engine.set_band(0, bell(1_000.0, 6.0, 1.0));
            engine.set_band(1, bell(5_000.0, -4.0, 2.0));
        });
        let sizes = Sizes::new(RATE, resolution);
        let latency = engine.latency() as usize;
        let mut impulse = vec![0.0f32; latency + sizes.kernel];
        impulse[0] = 1.0;
        let (response, _) = run(&mut engine, &impulse.clone(), &impulse, 256, |_, _| {});
        let peak = response.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        for j in 1..sizes.kernel / 2 {
            let (a, b) = (response[latency - j], response[latency + j]);
            assert!(
                (a - b).abs() <= peak * 3.0e-5,
                "{resolution:?}: tap ±{j}: {a} against {b}"
            );
        }
        let kernel = &response[latency - sizes.kernel / 2..latency + sizes.kernel / 2];
        for freq in [200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0] {
            let want: f64 = [(1_000.0, 6.0, 1.0), (5_000.0, -4.0, 2.0)]
                .iter()
                .map(|&(f, g, q)| {
                    design(
                        Design {
                            shape: Shape::Bell,
                            slope: Slope::Db12,
                            freq: f,
                            gain_db: g,
                            q,
                        },
                        RATE,
                    )
                    .response_db(freq, RATE)
                })
                .sum();
            let got = level_db(kernel, freq);
            assert!(
                (got - want).abs() <= 0.25,
                "{resolution:?}, {freq} Hz: {got} against {want}"
            );
        }
    }
}

/// **A Mid band in Linear phase moves sound across only by its magnitude** (plan §7.7, §6): a
/// hard-left sine through a Mid bell puts `(|m| − 1)/2` of it into the right channel, within
/// 0.1 dB, at 100 Hz, 1 kHz and 10 kHz.
#[test]
fn a_mid_band_moves_sound_across_only_by_its_magnitude() {
    let mid = bell(1_000.0, 6.0, 1.0);
    let m = |freq: f64| {
        10f64.powf(
            design(
                Design {
                    shape: Shape::Bell,
                    slope: Slope::Db12,
                    freq: 1_000.0,
                    gain_db: 6.0,
                    q: 1.0,
                },
                RATE,
            )
            .response_db(freq, RATE)
                / 20.0,
        )
    };
    for freq in [100.0, 1_000.0, 10_000.0] {
        let mut engine = linear(Resolution::Medium, |engine| {
            engine.set_band(0, mid);
            engine.set_placement(0, Placement::Mid);
        });
        let n = 3 * RATE as usize;
        let left: Vec<f32> = (0..n)
            .map(|i| (0.5 * (std::f64::consts::TAU * freq * i as f64 / RATE).sin()) as f32)
            .collect();
        let silent = vec![0.0f32; n];
        let (_, right) = run(&mut engine, &left, &silent, 512, |_, _| {});
        let window = RATE as usize;
        let got = level_db(&right[n - window..], freq) - level_db(&left[n - window..], freq);
        let want = 20.0 * ((m(freq) - 1.0).abs() / 2.0).log10();
        assert!(
            (got - want).abs() <= 0.1,
            "{freq} Hz: {got} dB against {want} dB"
        );
    }
}

/// **A new kernel hears the same past, is heard on its boundaries, and fades without a click**
/// (plan §7.7): after 2 s of noise a change — offline — leaves the output the old kernel's until
/// the second boundary after it, is the new kernel's convolution of the whole input within −90 dB
/// of the output's RMS by the third, steps no more during the fade than the two kernels' own
/// outputs plus 1 % of the peak, and is the same on every run.
#[test]
fn a_kernel_change_hears_the_same_past_on_its_boundaries() {
    let n = 3 * RATE as usize;
    let (left, right) = (noise(n, 11), noise(n, 23));
    let at = 2 * RATE as usize;
    let before = bell(800.0, 9.0, 1.5);
    let after = bell(3_000.0, -9.0, 0.8);
    let changed = || {
        let mut engine = linear(Resolution::Medium, |engine| engine.set_band(0, before));
        run(&mut engine, &left, &right, 256, |engine, start| {
            if start == at {
                engine.set_band(0, after);
            }
        })
    };
    let (a, a_right) = changed();
    let (again, _) = changed();
    assert_eq!(a, again, "offline, the same every run");
    let mut old = linear(Resolution::Medium, |engine| engine.set_band(0, before));
    let (c, _) = run(&mut old, &left, &right, 256, |_, _| {});
    let mut new = linear(Resolution::Medium, |engine| engine.set_band(0, after));
    let (b, b_right) = run(&mut new, &left, &right, 256, |_, _| {});
    let update = Sizes::new(RATE, Resolution::Medium).update();
    assert_eq!(
        &a[..at + update],
        &c[..at + update],
        "the old kernel's until then"
    );
    let rms = (b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / n as f64).sqrt();
    for (index, (x, y)) in a.iter().zip(&b).enumerate().skip(at + 3 * update).chain(
        a_right
            .iter()
            .zip(&b_right)
            .enumerate()
            .skip(at + 3 * update),
    ) {
        let error = f64::from(x - y).abs();
        assert!(error <= rms * 3.2e-5, "sample {index}: {x} against {y}");
    }
    let peak = a.iter().fold(0.0f32, |p, x| p.max(x.abs()));
    let step = |signal: &[f32], index: usize| (signal[index] - signal[index - 1]).abs();
    for index in at + update..at + 3 * update {
        let bound = step(&b, index).max(step(&c, index)) + 0.01 * peak;
        assert!(step(&a, index) <= bound, "sample {index} clicks");
    }
}

/// **The tail** (plan §7.7): from the kernel's length and two partitions after the input ends, the
/// output is exactly silent, and the engine says it rings no more.
#[test]
fn the_tail_ends_in_exact_silence() {
    let mut engine = linear(Resolution::Low, |engine| {
        engine.set_band(0, bell(300.0, 12.0, 4.0));
    });
    let sizes = Sizes::new(RATE, Resolution::Low);
    let sound = 20_000;
    let mut input = noise(sound + sizes.tail() + 4_096, 5);
    input[sound..].fill(0.0);
    let (out, _) = run(&mut engine, &input.clone(), &input, 333, |_, _| {});
    let last = input.iter().rposition(|x| *x != 0.0).unwrap();
    assert!(out[last + sizes.tail()..].iter().all(|x| *x == 0.0));
    assert!(out[..last + sizes.tail()].iter().any(|x| *x != 0.0));
    assert!(!engine.is_ringing());
}

/// **A dynamic band is held at its drawn gain in Linear phase** (decision 37), and moves in Zero
/// latency.
#[test]
fn a_dynamic_band_is_held_in_linear_phase() {
    let setup = |engine: &mut Engine| {
        engine.set_band(0, bell(1_000.0, 0.0, 1.0));
        engine.set_dynamics(
            0,
            Dynamics {
                range_db: -12.0,
                threshold_db: -40.0,
                ..Dynamics::STATIC
            },
        );
    };
    let loud = noise(RATE as usize, 9);
    let mut held = linear(Resolution::Medium, setup);
    run(&mut held, &loud, &loud, 512, |_, _| {});
    let (drawn, heard) = held.gains_db(0).unwrap();
    assert_eq!(drawn, heard, "held");
    let mut moving = Engine::new(RATE);
    setup(&mut moving);
    moving.reset();
    run(&mut moving, &loud, &loud, 512, |_, _| {});
    let (drawn, heard) = moving.gains_db(0).unwrap();
    assert!(
        heard < drawn - 1.0,
        "moves in Zero latency: {drawn} {heard}"
    );
}

/// **Solo lines up with what it auditions** (plan §7.7): in Linear phase the audition is Zero
/// latency's, delayed by the latency, once both have faded in.
#[test]
fn solo_plays_on_the_delayed_input() {
    let setup = |engine: &mut Engine| engine.set_band(0, bell(1_000.0, 6.0, 2.0));
    let input = noise(RATE as usize, 17);
    // A reset ends solo, so it is asked for after.
    let mut late = linear(Resolution::Low, setup);
    late.set_solo(Some(0));
    let latency = late.latency() as usize;
    let (out, _) = run(&mut late, &input, &input, 256, |_, _| {});
    let mut now = Engine::new(RATE);
    setup(&mut now);
    now.reset();
    now.set_solo(Some(0));
    let (direct, _) = run(&mut now, &input, &input, 256, |_, _| {});
    let settled = latency + 2 * now.fade_frames() + 2 * now.control_interval();
    for index in settled..input.len() {
        assert!(
            (out[index] - direct[index - latency]).abs() <= 1.0e-6,
            "sample {index}"
        );
    }
}

/// **The drawn curve is what the kernel does** (plan §7.7): `heard_lines`, read between bins as the
/// editor draws it, within 0.1 dB of the kernel's measured response for bands wider than eight
/// bins, on and between bins.
#[test]
fn the_drawn_curve_is_what_the_kernel_does() {
    use mxm_fx_eq_n_dsp::linear::{Heard, heard_lines};
    let bands = [(800.0f32, 6.0f32, 1.0f32), (4_000.0, -5.0, 1.5)];
    let mut engine = linear(Resolution::Medium, |engine| {
        for (slot, &(freq, gain, q)) in bands.iter().enumerate() {
            engine.set_band(slot, bell(freq, gain, q));
        }
    });
    let sizes = Sizes::new(RATE, Resolution::Medium);
    let latency = engine.latency() as usize;
    let mut impulse = vec![0.0f32; latency + sizes.kernel];
    impulse[0] = 1.0;
    let (response, _) = run(&mut engine, &impulse.clone(), &impulse, 256, |_, _| {});
    let kernel = &response[latency - sizes.kernel / 2..latency + sizes.kernel / 2];
    let mut heard = Heard::EMPTY;
    for (slot, &(freq, gain_db, q)) in bands.iter().enumerate() {
        heard.bands[slot] = Some((
            design(
                Design {
                    shape: Shape::Bell,
                    slope: Slope::Db12,
                    freq: f64::from(freq),
                    gain_db: f64::from(gain_db),
                    q: f64::from(q),
                },
                RATE,
            ),
            Placement::Stereo,
        ));
    }
    let bin = RATE / sizes.kernel as f64;
    for freq in [
        300.0,
        800.0,
        800.0 + bin / 3.0,
        2_000.0 + bin / 2.0,
        4_000.0,
        9_000.0,
    ] {
        let [drawn, ..] = heard_lines(&heard, freq, RATE, sizes);
        let drawn = 20.0 * drawn.log10();
        let measured = level_db(kernel, freq);
        assert!(
            (drawn - measured).abs() <= 0.1,
            "{freq} Hz: drawn {drawn}, heard {measured}"
        );
    }
}

/// **Linear phase keeps the whole gain range** (plan §7.7): five coincident +60 dB bells
/// (+30 dB under a 200 % gain scale), +300 dB, take a 1e−14 tone at their centre to the level the
/// kernel's drawn response says, within 0.5 dB, more than +290 dB (the top of so steep a peak is
/// narrower than Low's bins, so it is read at High); and 24 of them, +1,440 dB, stay finite inside
/// the output's bound, loud, not silenced.
#[test]
fn the_whole_gain_range_is_kept() {
    let stack = |count: usize| {
        move |engine: &mut Engine| {
            for slot in 0..count {
                engine.set_band(slot, bell(1_000.0, 30.0, 1.0));
            }
            engine.set_gain_scale(2.0);
        }
    };
    let n = RATE as usize;
    let tone: Vec<f32> = (0..n)
        .map(|i| (1.0e-14 * (std::f64::consts::TAU * 1_000.0 * i as f64 / RATE).sin()) as f32)
        .collect();
    let mut late = linear(Resolution::High, stack(5));
    let (out, _) = run(&mut late, &tone.clone(), &tone, 512, |_, _| {});
    let mut heard_eq = mxm_fx_eq_n_dsp::linear::Heard::EMPTY;
    for slot in 0..5 {
        heard_eq.bands[slot] = Some((
            design(
                Design {
                    shape: Shape::Bell,
                    slope: Slope::Db12,
                    freq: 1_000.0,
                    gain_db: 60.0,
                    q: 1.0,
                },
                RATE,
            ),
            Placement::Stereo,
        ));
    }
    let [drawn, ..] = mxm_fx_eq_n_dsp::linear::heard_lines(
        &heard_eq,
        1_000.0,
        RATE,
        Sizes::new(RATE, Resolution::High),
    );
    let window = n / 2;
    let gain = level_db(&out[n - window..], 1_000.0) - level_db(&tone[n - window..], 1_000.0);
    let want = 20.0 * drawn.log10();
    assert!((gain - want).abs() <= 0.5, "{gain} dB against {want} dB");
    assert!(gain > 290.0, "{gain} dB");

    let mut worst = linear(Resolution::Low, stack(mxm_fx_eq_n_dsp::SLOTS));
    let input = noise(n / 2, 13);
    let (out, _) = run(&mut worst, &input.clone(), &input, 512, |_, _| {});
    let latency = worst.latency() as usize;
    let bound = mxm_fx_eq_n_dsp::engine::OUTPUT_BOUND as f32;
    assert!(out.iter().all(|x| x.is_finite() && x.abs() <= bound));
    assert!(
        out[latency + 4_096..].iter().any(|x| x.abs() > 1.0),
        "loud, not silenced"
    );
}

/// **Any rate the plugin is given sizes a kernel** (plan §7.7): powers of two, never past the
/// largest, never a panic, from the DSP's lowest rate to the largest finite one.
#[test]
fn every_rate_sizes_a_kernel() {
    let largest = Sizes::new(768_000.0, Resolution::High);
    for rate in [
        1.0,
        1_000.0,
        22_050.0,
        768_000.0,
        1.0e7,
        f64::from(f32::MAX),
        f64::MAX,
    ] {
        for resolution in Resolution::ALL {
            let sizes = Sizes::new(rate, resolution);
            assert!(sizes.kernel.is_power_of_two() && sizes.block.is_power_of_two());
            assert!(sizes.kernel <= largest.kernel && sizes.kernel >= 8 * sizes.block);
        }
    }
}

/// **A reset hears the settings as they stand** (plan §7.7): a band changed just before a reset is
/// heard from the reset's first sample, bit for bit as a fresh engine primed with it — offline at
/// once, waiting for its kernel; in realtime never waiting, its kernel taken unfaded once designed
/// while nothing sounding has been convolved.
#[test]
fn a_reset_hears_the_settings_as_they_stand() {
    let changed = |engine: &mut Engine| engine.set_band(0, bell(1_000.0, 12.0, 1.0));
    let input = noise(RATE as usize / 4, 21);
    let mut fresh = linear(Resolution::Low, changed);
    let (want, _) = run(&mut fresh, &input.clone(), &input, 256, |_, _| {});

    // Offline: the reset waits.
    let mut offline = linear(Resolution::Low, |_| {});
    let _ = run(&mut offline, &input.clone(), &input, 256, |_, _| {});
    changed(&mut offline);
    offline.reset();
    let (heard, _) = run(&mut offline, &input.clone(), &input, 256, |_, _| {});
    assert_eq!(heard, want, "offline");

    // Realtime: silence until the kernel is in, then the same input.
    let mut realtime = linear(Resolution::Low, |_| {});
    realtime.set_offline(false);
    let _ = run(&mut realtime, &input.clone(), &input, 256, |_, _| {});
    changed(&mut realtime);
    realtime.reset();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut silent = 0;
    while !realtime.is_settled() {
        assert!(std::time::Instant::now() < until, "the kernel never came");
        let (mut l, mut r) = ([0.0f32; 64], [0.0f32; 64]);
        realtime.process_stereo(&mut l, &mut r);
        assert!(l.iter().chain(&r).all(|x| *x == 0.0));
        silent += 64;
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut reference = linear(Resolution::Low, changed);
    let quiet = vec![0.0f32; silent];
    let _ = run(&mut reference, &quiet.clone(), &quiet, 64, |_, _| {});
    let (want, _) = run(&mut reference, &input.clone(), &input, 256, |_, _| {});
    let (heard, _) = run(&mut realtime, &input.clone(), &input, 256, |_, _| {});
    assert_eq!(heard, want, "realtime, after {silent} silent frames");
}
