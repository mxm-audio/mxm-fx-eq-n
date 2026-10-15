//! Stereo placement in the engine against plan §7.3: each placement filters only its channels;
//! changes step nothing, within a family, across it, and two at once in opposite directions; the
//! family order makes slot order irrelevant; a placed dynamic band detects its channel; solo plays
//! the band's channel where it sits; mono hears only Stereo bands.

use mxm_fx_eq_n_dsp::{Band, Design, Dynamics, Engine, Placement, Shape, Slope, design};
use std::f64::consts::TAU;

const RATE: f64 = 48_000.0;

fn bell(freq: f32, gain_db: f32) -> Band {
    Band {
        shape: Some(Shape::Bell),
        bypass: false,
        freq,
        gain_db,
        q: 2.0,
        slope: Slope::Db12,
    }
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

fn placed(placement: Placement, band: Band) -> Engine {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, band);
    engine.set_placement(0, placement);
    engine.reset();
    engine
}

/// **Each placement filters only its channels**: a Left band leaves the right bit for bit, a Right
/// band the left; a Mid band leaves a side-only input bit for bit, a Side band a mid-only one.
#[test]
fn each_placement_filters_only_its_channels() {
    let n = 24_000;
    let tone = sine(n, 1_000.0, 0.25);
    for (placement, untouched) in [(Placement::Left, 1), (Placement::Right, 0)] {
        let mut engine = placed(placement, bell(1_000.0, 12.0));
        let (mut left, mut right) = (tone.clone(), tone.clone());
        engine.process_stereo(&mut left, &mut right);
        let (heard, kept) = if untouched == 1 {
            (&left, &right)
        } else {
            (&right, &left)
        };
        assert_eq!(kept, &tone, "{placement:?} touched the other channel");
        assert!(
            tone_db(heard, 1_000.0, 0.25, 12_000) > 11.0,
            "{placement:?}"
        );
    }
    let negative: Vec<f32> = tone.iter().map(|sample| -sample).collect();
    let mut engine = placed(Placement::Mid, bell(1_000.0, 12.0));
    let (mut left, mut right) = (tone.clone(), negative.clone());
    engine.process_stereo(&mut left, &mut right);
    assert_eq!(
        (left, right),
        (tone.clone(), negative),
        "a Mid band on a side-only input"
    );
    let mut engine = placed(Placement::Side, bell(1_000.0, 12.0));
    let (mut left, mut right) = (tone.clone(), tone.clone());
    engine.process_stereo(&mut left, &mut right);
    assert_eq!(
        (left, right),
        (tone.clone(), tone),
        "a Side band on a mid-only input"
    );
}

/// The largest sample-to-sample step of a render with `changes` applied at block 40 of 80.
fn largest_step(start: &[(usize, Placement)], changes: &[(usize, Placement)]) -> f32 {
    let mut engine = Engine::new(RATE);
    for (slot, placement) in start {
        engine.set_band(*slot, bell(500.0 * (*slot + 1) as f32, 12.0));
        engine.set_placement(*slot, *placement);
    }
    engine.reset();
    let (mut previous, mut largest) = ([0.0f32; 2], 0.0f32);
    for block in 0..80 {
        if block == 40 {
            for (slot, placement) in changes {
                engine.set_placement(*slot, *placement);
            }
        }
        let offset = block * 256;
        let mut left: Vec<f32> = (offset..offset + 256)
            .map(|i| (0.25 * (TAU * 400.0 * i as f64 / RATE).sin()) as f32)
            .collect();
        let mut right: Vec<f32> = (offset..offset + 256)
            .map(|i| (0.25 * (TAU * 700.0 * i as f64 / RATE).sin()) as f32)
            .collect();
        engine.process_stereo(&mut left, &mut right);
        for (l, r) in left.iter().zip(&right) {
            if block >= 30 {
                largest = largest
                    .max((l - previous[0]).abs())
                    .max((r - previous[1]).abs());
            }
            previous = [*l, *r];
        }
    }
    largest
}

/// **A placement change steps nothing**: within a family, across it, and two bands crossing in
/// opposite directions at once. Tones of 0.25 through +12 dB bells move at most about 0.15 a
/// sample; a step from a switch would be far more.
#[test]
fn placement_changes_step_nothing() {
    use Placement::*;
    let bound = 0.2;
    for (start, changes) in [
        (vec![(0, Stereo)], vec![(0, Left)]),
        (vec![(0, Left)], vec![(0, Right)]),
        (vec![(0, Mid)], vec![(0, Side)]),
        (vec![(0, Left)], vec![(0, Mid)]),
        (vec![(0, Side)], vec![(0, Stereo)]),
        (vec![(0, Left), (1, Mid)], vec![(0, Mid), (1, Left)]),
    ] {
        let step = largest_step(&start, &changes);
        assert!(step < bound, "{start:?} → {changes:?} stepped {step}");
    }
}

/// **Slot order never matters**: a Left band and a Mid band give the same output, bit for bit,
/// whichever slot holds which (decision 22).
#[test]
fn the_family_order_makes_slot_order_irrelevant() {
    let render = |left_slot: usize, mid_slot: usize| {
        let mut engine = Engine::new(RATE);
        engine.set_band(left_slot, bell(800.0, 9.0));
        engine.set_placement(left_slot, Placement::Left);
        engine.set_band(mid_slot, bell(1_200.0, -6.0));
        engine.set_placement(mid_slot, Placement::Mid);
        engine.reset();
        let mut left = sine(9_600, 900.0, 0.25);
        let mut right = sine(9_600, 1_100.0, 0.25);
        engine.process_stereo(&mut left, &mut right);
        (left, right)
    };
    assert_eq!(render(0, 1), render(1, 0));
}

/// **A placed dynamic band detects its own channel** (decision 20): a loud left with a quiet right
/// moves a Left band and not a Right one; a Side band keyed from a mono key hears no side.
#[test]
fn a_placed_dynamic_band_detects_its_own_channel() {
    let cut = Dynamics {
        range_db: -12.0,
        threshold_db: -40.0,
        attack_ms: 1.0,
        release_ms: 50.0,
    };
    let mut engine = Engine::new(RATE);
    for (slot, placement) in [(0, Placement::Left), (1, Placement::Right)] {
        engine.set_band(slot, bell(1_000.0, 0.0));
        engine.set_dynamics(slot, cut);
        engine.set_placement(slot, placement);
    }
    let mut left = sine(24_000, 1_000.0, 0.5);
    let mut right = sine(24_000, 1_000.0, 0.001);
    engine.process_stereo(&mut left, &mut right);
    assert!(engine.gains_db(0).unwrap().1 < -6.0, "the Left band moves");
    assert_eq!(
        engine.gains_db(1).unwrap().1,
        0.0,
        "the Right band hears a quiet right"
    );

    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(1_000.0, 0.0));
    engine.set_dynamics(0, cut);
    engine.set_placement(0, Placement::Side);
    engine.set_side_chain(0, true);
    let key = sine(24_000, 1_000.0, 0.5);
    let (mut left, mut right) = (sine(24_000, 1_000.0, 0.1), sine(24_000, 1_000.0, 0.1));
    engine.process_stereo_keyed(&mut left, &mut right, Some([&key, &key]));
    assert_eq!(engine.gains_db(0).unwrap().1, 0.0, "a mono key has no side");
}

/// **Solo plays the band's channel where it sits**: a Left band's region in the left only, a Mid
/// band's in both alike, a Side band's as side.
#[test]
fn solo_plays_the_band_s_channel_where_it_sits() {
    let tone = sine(48_000, 1_000.0, 0.25);
    for placement in [Placement::Left, Placement::Mid, Placement::Side] {
        let mut engine = placed(placement, bell(1_000.0, 0.0));
        engine.set_solo(Some(0));
        let (mut left, mut right) = (tone.clone(), sine(48_000, 1_000.0, 0.1));
        engine.process_stereo(&mut left, &mut right);
        let tail = |samples: &[f32]| samples[24_000..].to_vec();
        match placement {
            Placement::Left => {
                assert!(tone_db(&left, 1_000.0, 0.25, 24_000).abs() < 0.5);
                assert!(
                    tail(&right).iter().all(|sample| sample.abs() < 1.0e-6),
                    "right silent"
                );
            }
            Placement::Mid => assert_eq!(tail(&left), tail(&right)),
            _ => {
                let negated: Vec<f32> = tail(&right).iter().map(|sample| -sample).collect();
                assert_eq!(tail(&left), negated);
            }
        }
    }
}

/// **Mono hears only Stereo bands**: a placed band in a mono block passes it bit for bit.
#[test]
fn in_mono_a_placed_band_does_nothing() {
    for placement in [
        Placement::Left,
        Placement::Right,
        Placement::Mid,
        Placement::Side,
    ] {
        let mut engine = placed(placement, bell(1_000.0, 12.0));
        let mut buffer = sine(9_600, 1_000.0, 0.25);
        engine.process_mono(&mut buffer);
        assert_eq!(buffer, sine(9_600, 1_000.0, 0.25), "{placement:?}");
    }
}

/// **The complex response is the magnitude the audio runs**, for every shape: the graph's lines
/// combine it (plan §7.3).
#[test]
fn the_complex_response_has_the_shipped_magnitude() {
    for shape in Shape::ALL {
        let cascade = design(
            Design {
                shape,
                slope: Slope::Db48,
                freq: 1_000.0,
                gain_db: 9.0,
                q: 3.0,
            },
            RATE,
        );
        for step in 0..=60 {
            let freq = 20.0 * 1_000f64.powf(f64::from(step) / 60.0);
            let (re, im) = cascade.response(freq, RATE);
            let magnitude = 10.0 * (re * re + im * im).log10();
            let shipped = cascade.response_db(freq, RATE);
            assert!((magnitude - shipped).abs() < 1.0e-6, "{shape:?} at {freq}");
        }
    }
}

/// **In mono a placed band does no work**: an engine whose one band is placed is dry, and passes
/// ordinary samples, NaN and subnormals bit for bit (the wet path would zero the last two).
#[test]
fn in_mono_a_placed_band_leaves_the_engine_dry() {
    let mut engine = placed(Placement::Left, bell(1_000.0, 12.0));
    let mut settle = sine(4_800, 1_000.0, 0.25);
    engine.process_mono(&mut settle);
    let input = [0.25f32, f32::NAN, f32::from_bits(1), -1.0e-40, 0.0, -0.5];
    let mut buffer = input.repeat(64);
    engine.process_mono(&mut buffer);
    assert!(engine.is_dry(), "a mono engine of placed bands is dry");
    let bits = |samples: &[f32]| samples.iter().map(|s| s.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&buffer), bits(&input.repeat(64)));
}

/// **A placement change waits with the band's own transition**: set while another fade runs, the
/// detector keeps the old channel until that fade ends and the placement's own begins.
#[test]
fn a_queued_placement_change_keeps_the_old_channel_until_it_starts() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, bell(1_000.0, 0.0));
    engine.set_dynamics(
        0,
        Dynamics {
            range_db: -12.0,
            threshold_db: -40.0,
            attack_ms: 0.5,
            release_ms: 50.0,
        },
    );
    engine.set_placement(0, Placement::Left);
    engine.reset();
    let interval = engine.control_interval();
    let quiet_left_loud_right = |engine: &mut Engine, frames: usize| {
        let mut left = sine(frames, 1_000.0, 0.001);
        let mut right = sine(frames, 1_000.0, 0.5);
        engine.process_stereo(&mut left, &mut right);
    };
    // A structural fade in progress: the shape changes, and its fade takes over a control step.
    engine.set_band(
        0,
        Band {
            shape: Some(Shape::LowShelf),
            ..bell(1_000.0, 0.0)
        },
    );
    quiet_left_loud_right(&mut engine, interval);
    engine.set_placement(0, Placement::Right);
    let fade = engine.fade_frames();
    quiet_left_loud_right(&mut engine, fade.saturating_sub(2 * interval));
    assert_eq!(
        engine.gains_db(0).unwrap().1,
        0.0,
        "still listening to the quiet left during the shape's fade"
    );
    quiet_left_loud_right(&mut engine, 4 * fade + 4_800);
    assert!(
        engine.gains_db(0).unwrap().1 < -6.0,
        "on the loud right once its own transition ran"
    );
}

const CUT: Dynamics = Dynamics {
    range_db: -12.0,
    threshold_db: -40.0,
    attack_ms: 0.5,
    release_ms: 50.0,
};

/// **In mono a placed band's fades still run their length**: a Left band that changes shape and is
/// then set to Stereo is heard once both transitions have run.
#[test]
fn in_mono_a_placed_band_s_fades_run_out_and_stereo_returns() {
    let mut engine = placed(Placement::Left, bell(1_000.0, 12.0));
    let fade = engine.fade_frames();
    let mut buffer = sine(4_800, 1_000.0, 0.25);
    engine.process_mono(&mut buffer);
    engine.set_band(
        0,
        Band {
            shape: Some(Shape::HighShelf),
            ..bell(1_000.0, 12.0)
        },
    );
    let mut buffer = sine(fade / 2, 1_000.0, 0.25);
    engine.process_mono(&mut buffer);
    engine.set_placement(0, Placement::Stereo);
    let mut buffer = sine(48_000, 1_000.0, 0.25);
    engine.process_mono(&mut buffer);
    assert!(
        tone_db(&buffer, 1_000.0, 0.25, 24_000) > 5.0,
        "the band came back as Stereo and is heard"
    );
}

/// **A bypassed placed band keeps its channel through its exit**: a dynamic Left band on a quiet
/// left and a loud right does not move while it fades out.
#[test]
fn an_exiting_placed_band_keeps_listening_to_its_channel() {
    let mut engine = placed(Placement::Left, bell(1_000.0, 0.0));
    engine.set_dynamics(0, CUT);
    let interval = engine.control_interval();
    let run = |engine: &mut Engine| {
        let mut left = sine(interval, 1_000.0, 0.001);
        let mut right = sine(interval, 1_000.0, 0.5);
        engine.process_stereo(&mut left, &mut right);
    };
    for _ in 0..200 {
        run(&mut engine);
    }
    engine.set_band(
        0,
        Band {
            bypass: true,
            ..bell(1_000.0, 0.0)
        },
    );
    for _ in 0..engine.fade_frames() / interval + 2 {
        run(&mut engine);
        let heard = engine.gains_db(0).unwrap().1;
        assert_eq!(heard, 0.0, "the exit heard the right: {heard}");
    }
}

/// **A split pair renders its Stereo band** (plan §7.3): a static bell and its Left/Right pair bit
/// for bit, its Mid/Side pair within rounding.
#[test]
fn a_static_split_pair_renders_its_stereo_band() {
    let render = |placements: &[Placement]| {
        let mut engine = Engine::new(RATE);
        for (slot, placement) in placements.iter().enumerate() {
            engine.set_band(slot, bell(1_000.0, 9.0));
            engine.set_placement(slot, *placement);
        }
        engine.reset();
        let mut left = sine(9_600, 700.0, 0.25);
        let mut right = sine(9_600, 1_300.0, 0.2);
        engine.process_stereo(&mut left, &mut right);
        (left, right)
    };
    let stereo = render(&[Placement::Stereo]);
    assert_eq!(render(&[Placement::Left, Placement::Right]), stereo);
    let (left, right) = render(&[Placement::Mid, Placement::Side]);
    for (a, b) in left
        .iter()
        .chain(&right)
        .zip(stereo.0.iter().chain(&stereo.1))
    {
        assert!((a - b).abs() < 1.0e-6, "{a} against {b}");
    }
}

/// **A dynamic split pair moves by its channels** (decision 20): a Left/Right pair as one on equal
/// channels and apart on a louder left; a Mid/Side pair on equal channels with only Mid moving.
#[test]
fn a_dynamic_split_pair_moves_by_its_channels() {
    let pair = |placements: [Placement; 2], left_amplitude: f64| {
        let mut engine = Engine::new(RATE);
        for (slot, placement) in placements.into_iter().enumerate() {
            engine.set_band(slot, bell(1_000.0, 0.0));
            engine.set_dynamics(slot, CUT);
            engine.set_placement(slot, placement);
        }
        let mut left = sine(24_000, 1_000.0, left_amplitude);
        let mut right = sine(24_000, 1_000.0, 0.05);
        engine.process_stereo(&mut left, &mut right);
        (engine.gains_db(0).unwrap().1, engine.gains_db(1).unwrap().1)
    };
    let (left, right) = pair([Placement::Left, Placement::Right], 0.05);
    assert_eq!(left, right, "equal channels move a Left/Right pair as one");
    let (left, right) = pair([Placement::Left, Placement::Right], 0.5);
    assert!(
        left < right - 1.0,
        "a louder left moves the left further: {left} {right}"
    );
    let (mid, side) = pair([Placement::Mid, Placement::Side], 0.05);
    assert!(
        mid < -3.0 && side == 0.0,
        "equal channels have no side: {mid} {side}"
    );
}

/// **A placed band edited in mono settles** though it is never heard: its coefficient ramps move
/// on over the frames the engine skips.
#[test]
fn a_placed_band_edited_in_mono_settles() {
    let mut engine = placed(Placement::Left, bell(1_000.0, 12.0));
    let mut buffer = sine(9_600, 1_000.0, 0.25);
    engine.process_mono(&mut buffer);
    engine.set_band(0, bell(2_000.0, 12.0));
    let mut buffer = sine(96_000, 1_000.0, 0.25);
    engine.process_mono(&mut buffer);
    assert_eq!(buffer, sine(96_000, 1_000.0, 0.25), "never heard");
    assert!(
        engine.is_settled(),
        "and settled once its glides have landed"
    );
}
