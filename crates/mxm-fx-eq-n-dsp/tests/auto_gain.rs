//! Auto gain (plan §7.8): the make-up's estimate, its weighting and limits, placed bands and mono,
//! its ramps, an empty EQ left bit-exact, and a bounce that repeats.

use mxm_fx_eq_n_dsp::linear::{Heard, stage_powers_at};
use mxm_fx_eq_n_dsp::loudness::{AutoGain, MAKE_UP_RANGE_DB};
use mxm_fx_eq_n_dsp::{Band, Design, Engine, Phase, Placement, SLOTS, Shape, Slope, design};

const RATE: f64 = 48_000.0;
/// +6 dB as a power gain.
const SIX_DB: f64 = 3.981_071_705_534_972;

fn band(shape: Shape, freq: f32, gain_db: f32, q: f32) -> Band {
    Band {
        shape: Some(shape),
        bypass: false,
        freq,
        gain_db,
        q,
        slope: Slope::Db12,
    }
}

/// A curve heard as the engine would hand it to the estimate, each band at `placement`.
fn heard(bands: &[(Shape, f64, f64, f64, Placement)]) -> Heard {
    let mut heard = Heard::EMPTY;
    for (slot, &(shape, freq, gain_db, q, placement)) in bands.iter().enumerate() {
        let inputs = Design {
            shape,
            slope: if shape == Shape::HighCut {
                Slope::Db96
            } else {
                Slope::Db12
            },
            freq,
            gain_db,
            q,
        };
        heard.bands[slot] = Some((design(inputs, RATE), placement));
    }
    heard
}

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

/// **Nothing to make up** for an empty EQ, exactly, and for bands that cancel; and an engine with
/// auto gain on and nothing in the sound stays bit-exact dry.
#[test]
fn an_empty_eq_or_one_that_cancels_makes_up_nothing() {
    let auto = AutoGain::new(RATE);
    assert_eq!(auto.make_up_db(&Heard::EMPTY), 0.0);
    let cancel = heard(&[
        (Shape::Bell, 1_000.0, 6.0, 1.0, Placement::Stereo),
        (Shape::Bell, 1_000.0, -6.0, 1.0, Placement::Stereo),
    ]);
    let db = auto.make_up_db(&cancel);
    assert!(db.abs() < 1e-6, "{db}");

    let mut engine = Engine::new(RATE);
    engine.set_auto_gain(true);
    engine.reset();
    let input = noise(4_800, 3);
    let (mut left, mut right) = (input.clone(), input.clone());
    engine.process_stereo(&mut left, &mut right);
    assert_eq!(left, input);
    assert_eq!(engine.make_up_db(), 0.0);
}

/// **A broadband boost is made up by its size**: a flat +6 dB power gain by 6 dB within 10⁻⁹ (the
/// weights normalised), and a low and a high shelf at one frequency, complementary analogue
/// shelves, within 0.1 dB of it.
#[test]
fn a_broadband_boost_is_made_up_by_its_size() {
    let auto = AutoGain::new(RATE);
    let want = -10.0 * SIX_DB.log10();
    let flat = auto.make_up_for(|_| SIX_DB);
    assert!((flat - want).abs() < 1e-9, "{flat}");
    let shelves = heard(&[
        (Shape::LowShelf, 1_000.0, 6.0, 0.71, Placement::Stereo),
        (Shape::HighShelf, 1_000.0, 6.0, 0.71, Placement::Stereo),
    ]);
    let db = auto.make_up_db(&shelves);
    assert!((db - want).abs() < 0.1, "{db}");
}

/// **The corpus weighting** (decision 39): a +6 dB high shelf at 4 kHz, where an average mix
/// carries little power, is made up by less than 1 dB; the same low shelf at 100 Hz by more.
#[test]
fn the_estimate_weighs_where_a_mix_carries_its_power() {
    let auto = AutoGain::new(RATE);
    let treble = auto.make_up_db(&heard(&[(
        Shape::HighShelf,
        4_000.0,
        6.0,
        0.71,
        Placement::Stereo,
    )]));
    let bass = auto.make_up_db(&heard(&[(
        Shape::LowShelf,
        100.0,
        6.0,
        0.71,
        Placement::Stereo,
    )]));
    assert!(treble < 0.0 && treble > -1.0, "{treble}");
    assert!(bass < -1.0, "{bass}");
}

/// **Held inside ±24 dB**: every slot a broad +30 dB bell is made up by exactly −24 dB, a steep
/// high cut at 20 Hz, which leaves almost nothing, by exactly +24 dB.
#[test]
fn the_make_up_is_held_inside_its_range() {
    let auto = AutoGain::new(RATE);
    let loud: Vec<_> = (0..SLOTS)
        .map(|_| (Shape::Bell, 1_000.0, 30.0, 0.3, Placement::Stereo))
        .collect();
    assert_eq!(auto.make_up_db(&heard(&loud)), -MAKE_UP_RANGE_DB);
    let silent = heard(&[(Shape::HighCut, 20.0, 0.0, 0.71, Placement::Stereo)]);
    assert_eq!(auto.make_up_db(&silent), MAKE_UP_RANGE_DB);
}

/// **Placed bands count by their share of an uncorrelated stereo signal**: a Mid or a Left band of
/// power gain G counts `(1 + G)/2` where the same band in Stereo counts G.
#[test]
fn a_placed_band_counts_half_the_power() {
    let auto = AutoGain::new(RATE);
    let stereo = heard(&[(Shape::Bell, 1_000.0, 6.0, 1.0, Placement::Stereo)]);
    let want = auto.make_up_for(|t| 0.5 * (1.0 + stage_powers_at(&stereo, t)[0]));
    for placement in [
        Placement::Mid,
        Placement::Left,
        Placement::Side,
        Placement::Right,
    ] {
        let placed = heard(&[(Shape::Bell, 1_000.0, 6.0, 1.0, placement)]);
        let db = auto.make_up_db(&placed);
        assert!(
            (db - want).abs() < 1e-12,
            "{placement:?}: {db} against {want}"
        );
    }
    assert!(auto.make_up_db(&stereo) < want);
}

/// **In mono only Stereo bands are in the curve**: a Left band makes up nothing there, and
/// something in stereo.
#[test]
fn mono_counts_only_stereo_bands() {
    let made_up = |channels: usize| {
        let mut engine = Engine::with_phase(RATE, Phase::ZeroLatency, channels);
        engine.set_band(0, band(Shape::Bell, 1_000.0, 6.0, 1.0));
        engine.set_placement(0, Placement::Left);
        engine.set_auto_gain(true);
        engine.reset();
        engine.make_up_db()
    };
    assert_eq!(made_up(1), 0.0);
    assert!(made_up(2) < -0.1, "{}", made_up(2));
}

/// The output of DC 0.25 through the low and high shelf pair, `frames` long.
fn dc_through(engine: &mut Engine, frames: usize) -> Vec<f32> {
    let (mut left, mut right) = (vec![0.25f32; frames], vec![0.25f32; frames]);
    engine.process_stereo(&mut left, &mut right);
    left
}

/// **The make-up ramps**, switching auto gain on and off as on a new estimate: over one fade, no
/// step between samples larger than the ramp's own, and settled where the estimate puts it.
#[test]
fn the_make_up_ramps_on_and_off() {
    let mut engine = Engine::new(RATE);
    engine.set_band(0, band(Shape::LowShelf, 1_000.0, 6.0, 0.71));
    engine.set_band(1, band(Shape::HighShelf, 1_000.0, 6.0, 0.71));
    engine.reset();
    let fade = engine.fade_frames();
    let before = dc_through(&mut engine, 4_800);
    let loud = f64::from(*before.last().unwrap());
    assert!((loud / 0.25 - SIX_DB.sqrt()).abs() < 0.01, "{loud}");
    let largest_step = |out: &[f32]| {
        out.windows(2)
            .map(|pair| f64::from((pair[1] - pair[0]).abs()))
            .fold(0.0, f64::max)
    };
    for (on, settle) in [(true, 0.25), (false, loud)] {
        let from = f64::from(*dc_through(&mut engine, 1).last().unwrap());
        engine.set_auto_gain(on);
        let out = dc_through(&mut engine, 4 * fade);
        let to = f64::from(*out.last().unwrap());
        assert!(
            (to - settle).abs() < 0.01 * settle,
            "{on}: {to} against {settle}"
        );
        let ramp = (from - to).abs() / fade as f64;
        assert!(
            largest_step(&out) <= ramp * 1.01 + 1e-9 * loud,
            "{on}: {} against {ramp}",
            largest_step(&out)
        );
        assert!(engine.is_settled());
    }
}

/// **Output gain and the make-up are each held at ±24 dB**: both at their top, the make-up is
/// +24 dB and the output stays finite inside its bound.
#[test]
fn output_gain_and_the_make_up_are_held_apart() {
    let mut engine = Engine::new(RATE);
    engine.set_band(
        0,
        Band {
            slope: Slope::Db96,
            ..band(Shape::HighCut, 20.0, 0.0, 0.71)
        },
    );
    engine.set_output_gain(10f32.powf(30.0 / 20.0));
    engine.set_auto_gain(true);
    engine.reset();
    assert_eq!(engine.make_up_db(), MAKE_UP_RANGE_DB);
    let out = {
        let input = noise(9_600, 5);
        let (mut left, mut right) = (input.clone(), input);
        engine.process_stereo(&mut left, &mut right);
        left
    };
    let bound = mxm_fx_eq_n_dsp::engine::OUTPUT_BOUND as f32;
    assert!(out.iter().all(|x| x.is_finite() && x.abs() <= bound));
}

/// **A bounce repeats** with auto gain on and a band moving every block.
#[test]
fn a_bounce_with_auto_gain_repeats() {
    let render = || {
        let mut engine = Engine::new(RATE);
        engine.set_auto_gain(true);
        engine.reset();
        let input = noise(48_000, 9);
        let (mut left, mut right) = (input.clone(), input);
        for (block, (l, r)) in left.chunks_mut(256).zip(right.chunks_mut(256)).enumerate() {
            engine.set_band(0, band(Shape::Bell, 200.0 + 40.0 * block as f32, 9.0, 1.0));
            engine.process_stereo(l, r);
        }
        (left, engine.make_up_db())
    };
    let (first, made_up) = render();
    assert!(made_up < -0.1, "{made_up}");
    assert_eq!(first, render().0);
}
