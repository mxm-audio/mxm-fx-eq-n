//! Dynamic bands in the engine against plan §7.1 and research §25.4: the law heard on a steady
//! tone, the idle rule, exact rest for a host's sleep, Range automation, the bound, the detector
//! following the band, and slot order.

use mxm_fx_eq_n_dsp::dynamics::law;
use mxm_fx_eq_n_dsp::{Band, Dynamics, Engine, Shape, Slope};
use std::f64::consts::TAU;

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

fn dynamics(range_db: f32, threshold_db: f32) -> Dynamics {
    Dynamics {
        range_db,
        threshold_db,
        attack_ms: 10.0,
        release_ms: 150.0,
    }
}

fn sine(n: usize, start: usize, freq: f64, amplitude: f64) -> Vec<f32> {
    (start..start + n)
        .map(|i| (amplitude * (TAU * freq * i as f64 / RATE).sin()) as f32)
        .collect()
}

/// A tone's level in dB in `output`, by correlation, against full scale.
fn tone_db(output: &[f32], freq: f64, start: usize) -> f64 {
    let (mut c, mut s) = (0.0, 0.0);
    for (i, &y) in output.iter().enumerate() {
        let phase = TAU * freq * (start + i) as f64 / RATE;
        c += f64::from(y) * phase.cos();
        s += f64::from(y) * phase.sin();
    }
    let amplitude = 2.0 * (c * c + s * s).sqrt() / output.len() as f64;
    20.0 * amplitude.log10()
}

/// Plays `seconds` of a tone through `engine` in blocks of one control interval, returning the
/// output and each interval's heard gain of `slot`.
fn play(
    engine: &mut Engine,
    slot: usize,
    seconds: f64,
    freq: f64,
    amplitude: f64,
    start: usize,
) -> (Vec<f32>, Vec<f64>) {
    let interval = engine.control_interval();
    let frames = (seconds * RATE) as usize / interval * interval;
    let mut output = sine(frames, start, freq, amplitude);
    let mut gains = Vec::new();
    for block in output.chunks_mut(interval) {
        engine.process_mono(block);
        gains.push(engine.gains_db(slot).unwrap().1);
    }
    (output, gains)
}

/// **The law heard**: a bell at a steady tone's frequency takes the law's gain for the tone's
/// level, cut and lift. The detector's band pass passes the tone at its centre unchanged, so its
/// peak is the tone's.
#[test]
fn a_steady_tone_takes_the_laws_gain_cut_and_lift() {
    let amplitude = 0.5f64;
    let level = 20.0 * amplitude.log10();
    for range in [-12.0f32, -6.0, 6.0, 12.0] {
        let mut engine = Engine::new(RATE);
        engine.set_band(0, bell(5_000.0, 0.0, 1.0));
        engine.set_dynamics(0, dynamics(range, -30.0));
        engine.reset();
        let (output, gains) = play(&mut engine, 0, 2.0, 5_000.0, amplitude, 0);
        let expected = law(level, -30.0, f64::from(range));
        let heard = *gains.last().unwrap();
        assert!(
            (heard - expected).abs() < 0.3,
            "Range {range}: heard {heard:.2} dB, the law {expected:.2}"
        );
        let tail = output.len() - (0.5 * RATE) as usize;
        let tone = tone_db(&output[tail..], 5_000.0, tail) - level;
        assert!(
            (tone - expected).abs() < 0.4,
            "Range {range}: the tone moved {tone:.2} dB, the law {expected:.2}"
        );
    }
}

/// (The buffers are refilled with silence before each block: `process_mono` works in place, and a
/// reused buffer would feed the output back in.)
///
/// **The idle rule** (plan §7.1): a band with Range listens, so it never parks, even at a 0 dB
/// drawn gain on silence; Range 0, or the gain scale at 0 %, parks it as a static band.
#[test]
fn a_band_with_range_never_parks_until_nothing_it_does_can_be_heard() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(1_000.0, 0.0, 1.0));
    engine.set_dynamics(0, dynamics(-6.0, -30.0));
    engine.reset();
    let mut silence = vec![0.0f32; 4_800];
    for _ in 0..10 {
        silence.fill(0.0);
        engine.process_mono(&mut silence);
    }
    assert!(!engine.is_dry(), "a band with Range runs");

    engine.set_gain_scale(0.0);
    for _ in 0..20 {
        silence.fill(0.0);
        engine.process_mono(&mut silence);
    }
    assert!(
        engine.is_dry(),
        "at a gain scale of 0 % nothing it does is heard"
    );

    engine.set_gain_scale(1.0);
    for _ in 0..20 {
        silence.fill(0.0);
        engine.process_mono(&mut silence);
    }
    assert!(!engine.is_dry());
    engine.set_dynamics(0, dynamics(0.0, -30.0));
    for _ in 0..20 {
        silence.fill(0.0);
        engine.process_mono(&mut silence);
    }
    assert!(
        engine.is_dry(),
        "Range back at 0: static, and the 0 dB bell parks"
    );
}

/// **A host's sleep starts from rest** (plan §7.1): after loud audio, silence brings every moving
/// part to exactly 0 and every state to silence within a bounded time, the engine then stops
/// ringing, and the heard gain is the drawn gain exactly.
#[test]
fn after_loud_audio_silence_brings_the_engine_to_exact_rest() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(5_000.0, 3.0, 1.0));
    engine.set_dynamics(0, dynamics(-12.0, -30.0));
    engine.reset();
    let (_, gains) = play(&mut engine, 0, 1.0, 5_000.0, 0.5, 0);
    assert!(*gains.last().unwrap() < -4.0, "the band was cutting");
    let mut silence = vec![0.0f32; 480];
    let mut blocks = 0;
    while engine.is_ringing() {
        silence.fill(0.0);
        engine.process_mono(&mut silence);
        blocks += 1;
        // Release 150 ms from about 9 dB to the rest floor is ln(9 / 1e−4) ≈ 11.4 time
        // constants, 1.7 s; then the filters' tail.
        assert!(blocks < 400, "still ringing after 4 s of silence");
    }
    let (drawn, heard) = engine.gains_db(0).unwrap();
    assert_eq!(heard, drawn, "at rest the band is as drawn");
    assert_eq!(heard, 3.0);
}

/// **Range automation is smooth through both signs**: Range jumps between ±30 every few
/// intervals, but the heard gain moves at most an attack step an interval, since only the moving
/// part changes, and it is the ballistics'.
#[test]
fn range_automation_through_both_signs_moves_the_heard_gain_smoothly() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(5_000.0, 0.0, 1.0));
    engine.set_dynamics(0, dynamics(-30.0, -40.0));
    engine.reset();
    let interval = engine.control_interval();
    // The largest step an interval's attack allows from the furthest the law asks: all of Range,
    // twice over across a sign change.
    let attack_step = 60.0 * (1.0 - (-(interval as f64 / RATE) / 0.010).exp());
    let mut input = sine(interval, 0, 5_000.0, 0.5);
    let mut last = engine.gains_db(0).unwrap().1;
    for step in 0..6_000usize {
        if step % 7 == 0 {
            let range = [-30.0f32, 30.0, -6.0, 6.0, 0.0, -30.0][(step / 7) % 6];
            engine.set_dynamics(0, dynamics(range, -40.0));
        }
        input.copy_from_slice(&sine(interval, step * interval, 5_000.0, 0.5));
        engine.process_mono(&mut input);
        let heard = engine.gains_db(0).unwrap().1;
        assert!(
            (heard - last).abs() <= attack_step + 1e-9,
            "interval {step}: {last:.3} to {heard:.3} dB"
        );
        assert!(heard.abs() <= 30.0);
        last = heard;
    }
}

/// **The bound under a drawn-gain edit** (plan §7.1): a +30 dB bell lifting by up to 20 dB more,
/// and then dragged to −30 while it is cutting, never leaves ±30 dB heard.
#[test]
fn the_heard_gain_stays_inside_the_bound_under_a_drawn_gain_edit() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(5_000.0, 30.0, 1.0));
    engine.set_dynamics(0, dynamics(20.0, -40.0));
    engine.reset();
    let (_, gains) = play(&mut engine, 0, 0.5, 5_000.0, 0.1, 0);
    assert!(gains.iter().all(|gain| *gain <= 30.0));
    assert_eq!(
        *gains.last().unwrap(),
        30.0,
        "the lift is held at the bound"
    );
    engine.set_dynamics(0, dynamics(-20.0, -40.0));
    engine.set_band(0, bell(5_000.0, -30.0, 1.0));
    let (_, gains) = play(&mut engine, 0, 1.0, 5_000.0, 0.1, 24_000);
    assert!(gains.iter().all(|gain| (-30.0..=30.0).contains(gain)));
    assert_eq!(
        *gains.last().unwrap(),
        -30.0,
        "the cut is held at the bound"
    );
}

/// **A reset during a cut** leaves the band as drawn at once, its audio too: the filter is built
/// after the moving part is cleared, so a quiet tone after it is not cut by stale coefficients.
#[test]
fn a_reset_during_a_cut_leaves_the_band_as_drawn() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(5_000.0, 0.0, 1.0));
    engine.set_dynamics(0, dynamics(-12.0, -30.0));
    engine.reset();
    let (_, gains) = play(&mut engine, 0, 1.0, 5_000.0, 0.5, 0);
    assert!(*gains.last().unwrap() < -6.0, "cutting");
    engine.reset();
    assert_eq!(engine.gains_db(0).unwrap(), (0.0, 0.0));
    // A tone 30 dB under the threshold: nothing to cut, so it must pass as drawn, at once.
    let quiet = 0.001;
    let (output, gains) = play(&mut engine, 0, 0.2, 5_000.0, quiet, 48_000);
    assert!(gains.iter().all(|gain| *gain == 0.0));
    let tone = tone_db(&output, 5_000.0, 48_000) - 20.0 * quiet.log10();
    assert!(tone.abs() < 0.05, "the quiet tone moved {tone:.3} dB");
}

/// **A slot turned Off keeps no stale Range**: Range set to 0 while it is Off, the band comes back
/// static over a loud tone, never a moment of the old cut.
#[test]
fn a_band_back_from_off_starts_at_its_configured_range() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(5_000.0, 0.0, 1.0));
    engine.set_dynamics(0, dynamics(-30.0, -40.0));
    engine.reset();
    let _ = play(&mut engine, 0, 1.0, 5_000.0, 0.5, 0);
    engine.set_band(
        0,
        Band {
            shape: None,
            ..bell(5_000.0, 0.0, 1.0)
        },
    );
    let _ = play(&mut engine, 0, 0.2, 5_000.0, 0.5, 48_000);
    engine.set_dynamics(0, dynamics(0.0, -40.0));
    engine.set_band(0, bell(5_000.0, 0.0, 1.0));
    let (_, gains) = play(&mut engine, 0, 0.5, 5_000.0, 0.5, 57_600);
    assert!(
        gains.iter().all(|gain| *gain == 0.0),
        "the old Range cut a static band: {:?}",
        gains.iter().copied().fold(0.0f64, f64::min)
    );
}

/// **Settled means nothing waits**: one interval of a loud tone gathers a peak the next control
/// step will act on, so the band is not settled; on a tone whose every interval peaks alike (3 kHz
/// at 48 kHz, one cycle an interval) it arrives and is settled; and a gain-scale glide toward 0 %
/// is heard by a band with Range, which will stop listening and park.
#[test]
fn settled_waits_for_a_pending_peak_and_a_scale_a_range_hears() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(3_000.0, 0.0, 1.0));
    engine.set_dynamics(0, dynamics(-12.0, -30.0));
    engine.reset();
    let interval = engine.control_interval();
    assert_eq!(interval, 16);
    let mut silence = vec![0.0f32; 480];
    engine.process_mono(&mut silence);
    assert!(engine.is_settled(), "a band with Range at rest on silence");
    let mut loud = sine(interval, 0, 3_000.0, 0.5);
    engine.process_mono(&mut loud);
    assert!(
        !engine.is_settled(),
        "a peak waits for the next control step"
    );
    let mut start = interval;
    let mut settled = false;
    for _ in 0..(3.0 * RATE) as usize / interval {
        let mut block = sine(interval, start, 3_000.0, 0.5);
        start += interval;
        engine.process_mono(&mut block);
        if engine.is_settled() {
            settled = true;
            break;
        }
    }
    assert!(settled, "a steady tone's cut never arrived");
    // Then silence: the cut releases, moving every interval with nothing in the peak.
    for _ in 0..3 {
        let mut quiet = vec![0.0f32; interval];
        engine.process_mono(&mut quiet);
    }
    assert!(!engine.is_settled(), "a release in progress is not settled");

    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(1_000.0, 0.0, 1.0));
    engine.set_dynamics(0, dynamics(-12.0, -30.0));
    engine.reset();
    silence.fill(0.0);
    engine.process_mono(&mut silence);
    assert!(engine.is_settled());
    engine.set_gain_scale(0.0);
    assert!(
        !engine.is_settled(),
        "the scale glides and the band will park"
    );
}

/// A dynamics edit is a setting unread until the next interval, as a band edit is.
#[test]
fn a_dynamics_edit_is_unsettled_until_read() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(1_000.0, 0.0, 1.0));
    engine.reset();
    let mut silence = vec![0.0f32; 480];
    silence.fill(0.0);
    engine.process_mono(&mut silence);
    assert!(engine.is_settled());
    engine.set_dynamics(0, dynamics(-12.0, -30.0));
    assert!(!engine.is_settled(), "an unread Range edit");
}

/// The detector is the band's region and follows it: moved off the tone, the band stops hearing
/// it and releases to the drawn gain.
#[test]
fn the_detector_follows_the_band() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(5_000.0, 0.0, 4.0));
    engine.set_dynamics(0, dynamics(-12.0, -30.0));
    engine.reset();
    let (_, gains) = play(&mut engine, 0, 1.0, 5_000.0, 0.5, 0);
    assert!(*gains.last().unwrap() < -6.0);
    engine.set_band(0, bell(300.0, 0.0, 4.0));
    let (_, gains) = play(&mut engine, 0, 3.0, 5_000.0, 0.5, 48_000);
    assert!(
        gains.last().unwrap().abs() < 0.05,
        "far from the tone, released: {}",
        gains.last().unwrap()
    );
}

/// **Slot order** (plan §7.1): every detector hears the EQ's input, so the gain each band takes is
/// the same in either order, interval by interval. The sound differs only by moving filters not
/// commuting, measured here: under −60 dB of the signal.
#[test]
fn the_gains_never_depend_on_slot_order_and_the_sound_barely() {
    let noise = {
        let mut state = 7u64;
        (0..(2.0 * RATE) as usize)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                0.5 * ((state >> 40) as f32 / (1u64 << 23) as f32 - 1.0)
            })
            .collect::<Vec<f32>>()
    };
    let render = |first: usize, second: usize| {
        let mut engine = Engine::new(RATE);
        engine.set_band(first, bell(2_000.0, 0.0, 2.0));
        engine.set_dynamics(first, dynamics(-12.0, -40.0));
        engine.set_band(second, bell(2_500.0, 3.0, 1.0));
        engine.set_dynamics(
            second,
            Dynamics {
                attack_ms: 1.0,
                release_ms: 40.0,
                ..dynamics(9.0, -45.0)
            },
        );
        engine.reset();
        let interval = engine.control_interval();
        let mut output = noise.clone();
        let mut gains = Vec::new();
        for block in output.chunks_mut(interval) {
            engine.process_mono(block);
            gains.push((
                engine.gains_db(first).unwrap(),
                engine.gains_db(second).unwrap(),
            ));
        }
        (output, gains)
    };
    let (a, gains_a) = render(0, 1);
    let (b, gains_b) = render(1, 0);
    assert_eq!(gains_a, gains_b, "the gains depend on slot order");
    assert!(
        gains_a.iter().any(|((_, heard), _)| *heard < -3.0),
        "the cut was working"
    );
    let power = |x: &[f32]| x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
    let difference: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
    let db = 10.0 * (power(&difference) / power(&a)).log10();
    println!("slot order: the swapped render differs by {db:.1} dB");
    assert!(db < -60.0, "the swapped render differs by {db:.1} dB");
}
