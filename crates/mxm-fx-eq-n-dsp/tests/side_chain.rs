//! The side chain in the engine against plan §7.2: a keyed band hears the key, not the input,
//! linked across the key's channels; no band keyed is F1 bit for bit; a source switch never steps
//! the gain; no key is silence; a key over a silent input keeps the band moving; solo plays what
//! the band listens to, and fades through a source switch.

use mxm_fx_eq_n_dsp::{Band, Dynamics, Engine, Shape, Slope};
use std::f64::consts::TAU;

const RATE: f64 = 48_000.0;

fn bell() -> Band {
    Band {
        shape: Some(Shape::Bell),
        bypass: false,
        freq: 1_000.0,
        gain_db: 0.0,
        q: 2.0,
        slope: Slope::Db12,
    }
}

/// A fast cut of up to 12 dB over −40 dB.
const CUT: Dynamics = Dynamics {
    range_db: -12.0,
    threshold_db: -40.0,
    attack_ms: 1.0,
    release_ms: 50.0,
};

fn keyed_engine() -> Engine {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell());
    engine.set_dynamics(0, CUT);
    engine.set_side_chain(0, true);
    engine
}

fn sine(n: usize, freq: f64, amplitude: f64) -> Vec<f32> {
    (0..n)
        .map(|i| (amplitude * (TAU * freq * i as f64 / RATE).sin()) as f32)
        .collect()
}

/// A tone's level in dB relative to `amplitude`, over the last `n` samples of `output`.
fn tone_db(output: &[f32], freq: f64, amplitude: f64, n: usize) -> f64 {
    let start = output.len() - n;
    let (mut c, mut s) = (0.0, 0.0);
    for (i, &y) in output[start..].iter().enumerate() {
        let phase = TAU * freq * (start + i) as f64 / RATE;
        c += f64::from(y) * phase.cos();
        s += f64::from(y) * phase.sin();
    }
    20.0 * (2.0 * (c * c + s * s).sqrt() / n as f64 / amplitude).log10()
}

/// **The key triggers a keyed band, not the input**: a loud key cuts a quiet input, and a loud
/// input with a silent key leaves the band as drawn.
#[test]
fn a_keyed_band_hears_the_key_not_the_input() {
    let n = 48_000;
    let quiet = 0.005;
    let mut engine = keyed_engine();
    let mut input = sine(n, 1_000.0, quiet);
    let key = sine(n, 1_000.0, 0.5);
    engine.process_mono_keyed(&mut input, Some(&key));
    let cut = tone_db(&input, 1_000.0, quiet, 24_000);
    assert!(
        (-12.0..-6.0).contains(&cut),
        "a loud key cuts a quiet input: {cut} dB"
    );

    let mut engine = keyed_engine();
    let mut loud = sine(n, 1_000.0, 0.5);
    let silent = vec![0.0f32; n];
    engine.process_mono_keyed(&mut loud, Some(&silent));
    let heard = tone_db(&loud, 1_000.0, 0.5, 24_000);
    assert!(
        heard.abs() < 0.05,
        "a loud input does not trigger a keyed band: {heard} dB"
    );
}

/// **Linked across the key's channels**: a key in its left channel only cuts both channels alike.
#[test]
fn the_key_is_linked_across_its_channels() {
    let n = 48_000;
    let mut engine = keyed_engine();
    let mut left = sine(n, 1_000.0, 0.005);
    let mut right = left.clone();
    let key_left = sine(n, 1_000.0, 0.5);
    let key_right = vec![0.0f32; n];
    engine.process_stereo_keyed(&mut left, &mut right, Some([&key_left, &key_right]));
    assert_eq!(left, right);
    assert!(tone_db(&left, 1_000.0, 0.005, 24_000) < -6.0);
}

/// **No band keyed is F1, bit for bit**, whatever the key carries; a dynamic band on its own
/// input included.
#[test]
fn with_no_band_keyed_the_key_changes_nothing() {
    let n = 20_000;
    let run = |key: Option<&[f32]>| {
        let mut engine = Engine::new(RATE);
        engine.set_band(0, bell());
        engine.set_dynamics(0, CUT);
        engine.set_band(
            1,
            Band {
                freq: 200.0,
                gain_db: 6.0,
                ..bell()
            },
        );
        let mut buffer = sine(n, 1_000.0, 0.5);
        for block in buffer.chunks_mut(256) {
            match key {
                Some(key) => engine.process_mono_keyed(block, Some(&key[..block.len()])),
                None => engine.process_mono(block),
            }
        }
        buffer
    };
    let noise: Vec<f32> = (0..n)
        .map(|i| ((i * 7919) % 2000) as f32 / 1000.0 - 1.0)
        .collect();
    assert_eq!(run(None), run(Some(&noise)));
}

/// **No key is silence**: a keyed band given no key is the band as drawn.
#[test]
fn a_keyed_band_with_no_key_is_the_band_as_drawn() {
    let mut engine = keyed_engine();
    let mut loud = sine(48_000, 1_000.0, 0.5);
    engine.process_mono(&mut loud);
    assert!(tone_db(&loud, 1_000.0, 0.5, 24_000).abs() < 0.05);
    assert_eq!(engine.gains_db(0).map(|(_, heard)| heard), Some(0.0));
}

/// **A source switch never steps the gain**: a band cutting its own loud input switched to a
/// silent key releases through its ballistics, and its heard gain moves by small steps.
#[test]
fn a_source_switch_releases_smoothly() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell());
    engine.set_dynamics(0, CUT);
    let interval = engine.control_interval();
    let silent = vec![0.0f32; interval];
    let mut gains = Vec::new();
    for block in 0..3_000 {
        if block == 1_000 {
            engine.set_side_chain(0, true);
        }
        let mut input = sine(interval, 1_000.0, 0.5);
        engine.process_mono_keyed(&mut input, Some(&silent));
        gains.push(engine.gains_db(0).unwrap().1);
    }
    assert!(
        gains[999] < -6.0,
        "cutting before the switch: {}",
        gains[999]
    );
    assert_eq!(gains[2_999], 0.0, "released to rest on the silent key");
    // From just before the switch: the 1 ms attack at the start moves faster, as it should.
    let step = gains[990..]
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f64::max);
    assert!(
        step < 1.0,
        "the largest step between intervals is {step} dB"
    );
}

/// **A key over a silent input keeps the band moving**: the tail is not cleared, the engine keeps
/// the host calling, and the input's return meets the band where the key put it.
#[test]
fn a_key_over_a_silent_input_keeps_the_band_where_it_put_it() {
    let mut engine = keyed_engine();
    let key = sine(24_000, 1_000.0, 0.5);
    let mut silence = vec![0.0f32; 24_000];
    engine.process_mono_keyed(&mut silence, Some(&key));
    assert!(silence.iter().all(|sample| *sample == 0.0));
    let (_, heard) = engine.gains_db(0).unwrap();
    assert!(heard < -6.0, "moving on the key alone: {heard}");
    assert!(engine.is_ringing(), "a moving band keeps the host calling");
    let interval = engine.control_interval();
    let mut back = sine(interval, 1_000.0, 0.005);
    engine.process_mono_keyed(&mut back, Some(&key[..interval]));
    assert!(
        engine.gains_db(0).unwrap().1 < -6.0,
        "the input returns to the cut"
    );
}

/// **Solo plays what the band listens to**: a keyed band's solo is the key through the band's
/// region; a switch of source fades through, with no step, to the input's region.
#[test]
fn solo_plays_the_key_for_a_keyed_band_and_fades_through_a_switch() {
    let n = 48_000;
    let mut engine = keyed_engine();
    engine.set_solo(Some(0));
    let mut input = sine(n, 300.0, 0.25);
    let key = sine(n, 1_000.0, 0.25);
    engine.process_mono_keyed(&mut input, Some(&key));
    let key_heard = tone_db(&input, 1_000.0, 0.25, 24_000);
    let input_heard = tone_db(&input, 300.0, 0.25, 24_000);
    assert!(
        key_heard > -1.0,
        "the key at the band's centre: {key_heard} dB"
    );
    assert!(
        input_heard < -10.0,
        "the input is not soloed: {input_heard} dB"
    );

    engine.set_side_chain(0, false);
    let mut after = sine(n, 300.0, 0.25);
    engine.process_mono_keyed(&mut after, Some(&key));
    let mut previous = input[input.len() - 1];
    let mut largest = 0.0f32;
    for &sample in &after[..4_800] {
        largest = largest.max((sample - previous).abs());
        previous = sample;
    }
    // A 1 kHz or 300 Hz tone at 0.25 moves at most 0.033 a sample: a click would be far more.
    assert!(largest < 0.05, "the switch stepped the solo by {largest}");
    assert!(
        tone_db(&after, 1_000.0, 0.25, 24_000) < -20.0,
        "the key has faded out"
    );
}

/// **Only a dynamic shape listens to the key**: a keyed bell changed to a low cut keeps its switch,
/// and its solo plays the input it removes, not the absent key's silence.
#[test]
fn a_switch_kept_from_a_dynamic_shape_does_not_key_another_shape() {
    let mut engine = keyed_engine();
    engine.set_band(
        0,
        Band {
            shape: Some(Shape::LowCut),
            ..bell()
        },
    );
    engine.set_solo(Some(0));
    let mut input = sine(48_000, 100.0, 0.25);
    engine.process_mono(&mut input);
    let heard = tone_db(&input, 100.0, 0.25, 24_000);
    assert!(
        heard > -3.0,
        "the cut's solo plays the lows it removes: {heard} dB"
    );
}
