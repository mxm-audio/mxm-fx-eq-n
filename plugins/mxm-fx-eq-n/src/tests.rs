//! The shell against plan §8 and brief A1: identity, layouts, the empty EQ, parameters into sound,
//! solo's lifecycle, the analyser and meter handoffs, activity and the realtime rules.

use super::*;
use mxm_fx_eq_n_dsp::{Design, Shape, Slope, design};
use mxm_telemetry::PeakMeter;
use nice_plug::params::{InternalParamMut, Param};
use params::{BandParams, slope_value, type_of};
use std::f64::consts::TAU;

fn set_float(param: &FloatParam, value: f32) {
    unsafe {
        let _ = param._internal_set_normalized_value(param.preview_normalized(value));
    }
}

fn set_int(param: &IntParam, value: i32) {
    unsafe {
        let _ = param._internal_set_normalized_value(param.preview_normalized(value));
    }
}

fn set_bool(param: &BoolParam, value: bool) {
    unsafe {
        let _ = param._internal_set_normalized_value(if value { 1.0 } else { 0.0 });
    }
}

/// A slot set as the editor or a host would, through its parameters.
fn set_band(
    band: &BandParams,
    shape: Option<Shape>,
    freq: f32,
    gain_db: f32,
    q: f32,
    slope: Slope,
) {
    set_int(&band.kind, type_of(shape));
    set_float(&band.freq, freq);
    set_float(&band.gain, gain_db);
    set_float(&band.q, q);
    set_int(&band.slope, slope_value(slope));
    set_bool(&band.bypass, false);
}

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    f32::from(*seed as u16) / f32::from(u16::MAX) * 2.0 - 1.0
}

fn prepared(channels: usize) -> MxmFxEqN {
    let mut plugin = MxmFxEqN::default();
    assert!(plugin.prepare(48_000.0, channels, false));
    plugin
}

fn run(plugin: &mut MxmFxEqN, buffers: &mut [Vec<f32>]) -> ProcessStatus {
    let mut channels: Vec<&mut [f32]> = buffers.iter_mut().map(|b| b.as_mut_slice()).collect();
    plugin.process_block(&mut channels)
}

/// The gain at a whole-hertz `freq` over the last second of `output` for an input of `amplitude`.
fn tone_db(output: &[f32], freq: f64, amplitude: f64) -> f64 {
    let n = 48_000;
    let start = output.len() - n;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (i, &y) in output[start..].iter().enumerate() {
        let phase = TAU * freq * (start + i) as f64 / 48_000.0;
        re += f64::from(y) * phase.cos();
        im += f64::from(y) * phase.sin();
    }
    20.0 * (2.0 * re.hypot(im) / n as f64 / amplitude).log10()
}

#[test]
fn permanent_identity_and_layouts_are_fixed() {
    assert_eq!(NAME, "mxm-fx-eq-n");
    assert_eq!(CLAP_ID, "dk.mxm.mxm-fx-eq-n");
    mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
    // Four, their order permanent: a configuration's id is its index (plan §7.2).
    let layouts: Vec<(u32, u32, Vec<u32>, &[&str])> = MxmFxEqN::AUDIO_IO_LAYOUTS
        .iter()
        .map(|layout| {
            (
                layout.main_input_channels.map_or(0, NonZeroU32::get),
                layout.main_output_channels.map_or(0, NonZeroU32::get),
                layout
                    .aux_input_ports
                    .iter()
                    .map(|port| port.get())
                    .collect(),
                layout.names.aux_inputs,
            )
        })
        .collect();
    let keyed: &[&str] = &["Side chain"];
    assert_eq!(
        layouts,
        [
            (2, 2, vec![2], keyed),
            (1, 1, vec![1], keyed),
            (2, 2, vec![], &[][..]),
            (1, 1, vec![], &[][..]),
        ],
        "stereo and mono, each with a side chain first, then without (brief A1)"
    );
    assert!(
        MxmFxEqN::AUDIO_IO_LAYOUTS
            .iter()
            .all(|layout| layout.aux_output_ports.is_empty())
    );
    assert_eq!(MxmFxEqN::MIDI_INPUT, MidiConfig::None);
}

#[test]
fn init_is_bit_exact_dry_in_both_layouts() {
    for channels in [2, 1] {
        let mut plugin = prepared(channels);
        let mut seed = 99;
        let mut buffers: Vec<Vec<f32>> = (0..channels)
            .map(|_| (0..5_000).map(|_| noise(&mut seed)).collect())
            .collect();
        buffers[0][10] = f32::NAN;
        buffers[channels - 1][20] = f32::INFINITY;
        buffers[0][30] = f32::MIN_POSITIVE / 8.0;
        let dry: Vec<Vec<u32>> = buffers
            .iter()
            .map(|b| b.iter().map(|x| x.to_bits()).collect())
            .collect();
        run(&mut plugin, &mut buffers);
        let wet: Vec<Vec<u32>> = buffers
            .iter()
            .map(|b| b.iter().map(|x| x.to_bits()).collect())
            .collect();
        assert_eq!(dry, wet, "{channels} channel(s)");
    }
}

#[test]
fn a_band_set_through_its_parameters_is_heard_as_designed() {
    let mut plugin = prepared(2);
    set_band(
        &plugin.params.bands[4],
        Some(Shape::Bell),
        1_000.0,
        12.0,
        2.0,
        Slope::Db12,
    );
    set_float(&plugin.params.gain_scale, 0.5);
    let input: Vec<f32> = (0..96_000)
        .map(|i| (0.25 * (TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let mut buffers = vec![input.clone(), input];
    run(&mut plugin, &mut buffers);
    // +12 dB at 50 %: the drawn 6 dB at the centre.
    let drawn = design(
        Design {
            shape: Shape::Bell,
            slope: Slope::Db12,
            freq: 1_000.0,
            gain_db: 6.0,
            q: 2.0,
        },
        48_000.0,
    )
    .response_db(1_000.0, 48_000.0);
    let heard = tone_db(&buffers[0], 1_000.0, 0.25);
    assert!((heard - drawn).abs() < 0.01, "heard {heard}, drawn {drawn}");
    assert_eq!(buffers[0], buffers[1]);
}

#[test]
fn a_saved_eq_is_heard_from_its_first_sample() {
    let mut plugin = MxmFxEqN::default();
    set_band(
        &plugin.params.bands[0],
        Some(Shape::Bell),
        1_000.0,
        12.0,
        1.0,
        Slope::Db12,
    );
    assert!(plugin.prepare(48_000.0, 1, false));
    let mut impulse = vec![vec![0.0f32; 64]];
    impulse[0][0] = 1.0;
    run(&mut plugin, &mut impulse);
    assert_ne!(
        impulse[0][0], 1.0,
        "the saved bell shapes the first sample, not dry"
    );
    let mut reference = mxm_fx_eq_n_dsp::filter::Filter::fresh(&mxm_fx_eq_n_dsp::design_running(
        Design {
            shape: Shape::Bell,
            slope: Slope::Db12,
            freq: 1_000.0,
            gain_db: 12.0,
            q: 1.0,
        },
        48_000.0,
    ));
    let [first] = reference.process([1.0]);
    assert_eq!(impulse[0][0], first as f32);
}

#[test]
fn solo_needs_an_editor_never_plays_offline_and_ends_with_the_shell() {
    let mut plugin = prepared(2);
    set_band(
        &plugin.params.bands[2],
        Some(Shape::Bell),
        800.0,
        0.0,
        4.0,
        Slope::Db12,
    );
    let mut buffers = vec![vec![0.1f32; 1_024], vec![0.1f32; 1_024]];
    let ask = |plugin: &MxmFxEqN, slot| {
        let (endings, _) = plugin.audition.state();
        assert!(plugin.audition.request(endings, Some(slot)));
    };

    // Asked with no editor open: not heard.
    ask(&plugin, 2);
    run(&mut plugin, &mut buffers);
    assert_eq!(plugin.engine.audition(), None);

    // With an editor: heard.
    plugin.telemetry.connect(true);
    run(&mut plugin, &mut buffers);
    assert_eq!(plugin.engine.audition(), Some(2));

    // A reset ends it, and the editor's request from before cannot bring it back.
    let (stale, _) = plugin.audition.state();
    plugin.reset();
    assert_eq!(plugin.audition.slot(), None);
    assert!(!plugin.audition.request(stale, Some(2)));
    run(&mut plugin, &mut buffers);
    assert_eq!(plugin.engine.audition(), None);

    // Deactivation ends it.
    ask(&plugin, 2);
    run(&mut plugin, &mut buffers);
    assert_eq!(plugin.engine.audition(), Some(2));
    plugin.deactivate();
    assert_eq!(plugin.audition.slot(), None);

    // An offline render never solos, editor or not.
    assert!(plugin.prepare(48_000.0, 2, true));
    ask(&plugin, 2);
    for _ in 0..10 {
        run(&mut plugin, &mut buffers);
    }
    assert_eq!(plugin.engine.audition(), None);
}

#[test]
fn with_an_editor_open_processing_never_allocates() {
    let mut plugin = prepared(2);
    plugin.telemetry.connect(true);
    // Another track's editor watches it, and auto gain follows the bands (plan §7.8).
    let _watch = plugin.instance.watch();
    set_bool(&plugin.params.auto_gain, true);
    let consumer = plugin.analyser.take();
    let mut consumer = consumer.expect("the ring's editor half, once");
    for (slot, band) in plugin.params.bands.iter().enumerate() {
        let shape = mxm_fx_eq_n_dsp::Shape::ALL[slot % 8];
        set_band(
            band,
            Some(shape),
            100.0 * (slot + 1) as f32,
            6.0,
            1.5,
            Slope::Db96,
        );
    }
    let (endings, _) = plugin.audition.state();
    assert!(plugin.audition.request(endings, Some(3)));
    let mut seed = 3;
    let mut buffers = vec![vec![0.0f32; 2_048], vec![0.0f32; 2_048]];
    let mut blocks = 0;
    for block in 0..100 {
        for buffer in &mut buffers {
            for sample in buffer.iter_mut() {
                *sample = noise(&mut seed) * 0.25;
            }
        }
        if block == 40 {
            // Structural changes mid-run: a shape, a deletion, another solo.
            set_int(&plugin.params.bands[0].kind, type_of(Some(Shape::HighCut)));
            set_int(&plugin.params.bands[1].kind, 0);
            let (endings, _) = plugin.audition.state();
            assert!(plugin.audition.request(endings, Some(4)));
        }
        let (left, right) = buffers.split_at_mut(1);
        let mut channels: [&mut [f32]; 2] = [&mut left[0], &mut right[0]];
        nice_assert_no_alloc::assert_no_alloc(|| plugin.process_block(&mut channels));
        while consumer.pop().is_ok() {
            blocks += 1;
        }
    }
    assert!(buffers.iter().flatten().all(|x| x.is_finite()));
    assert_eq!(
        blocks,
        100 * 2_048 / analyser::BLOCK_FRAMES,
        "every block reached the editor"
    );
    assert!(
        plugin.analyser.take().is_none(),
        "the ring's half is handed out once"
    );
}

#[test]
fn the_analyser_copies_nothing_without_an_editor_and_carries_pre_and_post_with_one() {
    let mut plugin = prepared(1);
    set_band(
        &plugin.params.bands[0],
        Some(Shape::Bell),
        1_000.0,
        -12.0,
        1.0,
        Slope::Db12,
    );
    let consumer = plugin.analyser.take();
    let mut consumer = consumer.unwrap();
    let input: Vec<f32> = (0..4_096)
        .map(|i| (0.5 * (TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let mut buffers = vec![input.clone()];
    run(&mut plugin, &mut buffers);
    assert!(consumer.pop().is_err(), "no editor, no work");

    plugin.telemetry.connect(true);
    let mut buffers = vec![input.clone()];
    run(&mut plugin, &mut buffers);
    let block = consumer.pop().expect("a block with the editor open");
    assert_eq!(block.channels, 1);
    assert_eq!(block.sample_rate, 48_000.0);
    // Mono carries its channel in both; pre is the input, post the output.
    assert_eq!(block.pre[0], block.pre[1]);
    assert_eq!(&block.pre[0][..], &input[..analyser::BLOCK_FRAMES]);
    assert_eq!(&block.post[0][..], &buffers[0][..analyser::BLOCK_FRAMES]);

    plugin.telemetry.set_analyser_on(false);
    while consumer.pop().is_ok() {}
    let mut buffers = vec![input];
    run(&mut plugin, &mut buffers);
    assert!(
        consumer.pop().is_err(),
        "the analyser switched off copies nothing"
    );
}

#[test]
fn the_out_meter_reads_only_with_an_editor_and_holds_its_peak() {
    let mut plugin = prepared(2);
    let mut buffers = vec![vec![0.5f32; 512], vec![-0.25f32; 512]];
    run(&mut plugin, &mut buffers);
    assert_eq!(
        plugin.telemetry.output.each_ref().map(PeakMeter::take),
        [0.0; 2]
    );
    plugin.telemetry.connect(true);
    run(&mut plugin, &mut buffers);
    let mut quieter = vec![vec![0.1f32; 512], vec![0.1f32; 512]];
    run(&mut plugin, &mut quieter);
    assert_eq!(
        plugin.telemetry.output.each_ref().map(PeakMeter::take),
        [0.5, 0.25]
    );
    let mut loud = vec![vec![1.5f32; 64], vec![0.0f32; 64]];
    run(&mut plugin, &mut loud);
    assert_eq!(
        plugin.telemetry.output.each_ref().map(PeakMeter::clipped),
        [true, false]
    );
}

#[test]
fn a_non_finite_output_latches_the_clip() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut plugin = prepared(2);
        plugin.telemetry.connect(true);
        // The empty EQ passes the input bit for bit, the bad sample included.
        let mut buffers = vec![vec![0.25f32; 64], vec![0.25f32; 64]];
        buffers[0][10] = bad;
        run(&mut plugin, &mut buffers);
        assert_eq!(
            plugin.telemetry.output.each_ref().map(PeakMeter::clipped),
            [true, false],
            "{bad}"
        );
    }
}

#[test]
fn a_ringing_band_keeps_the_host_calling_until_it_has_settled() {
    let mut plugin = prepared(1);
    set_band(
        &plugin.params.bands[0],
        Some(Shape::Bell),
        200.0,
        12.0,
        4.0,
        Slope::Db12,
    );
    let mut burst = vec![
        (0..4_800)
            .map(|i| ((i as f32) * 0.1).sin())
            .collect::<Vec<f32>>(),
    ];
    assert!(matches!(
        run(&mut plugin, &mut burst),
        ProcessStatus::KeepAlive
    ));
    let mut status = ProcessStatus::KeepAlive;
    for _ in 0..200 {
        let mut silence = vec![vec![0.0f32; 4_800]];
        status = run(&mut plugin, &mut silence);
        if matches!(status, ProcessStatus::Normal) {
            // The tail has been cleared: what follows is exact silence, no subnormals.
            let mut after = vec![vec![0.0f32; 4_800]];
            assert!(matches!(
                run(&mut plugin, &mut after),
                ProcessStatus::Normal
            ));
            assert!(after[0].iter().all(|x| x.to_bits() == 0));
            break;
        }
    }
    assert!(matches!(status, ProcessStatus::Normal), "the tail ends");
}

#[test]
fn a_rate_the_engine_cannot_hold_is_refused() {
    let mut plugin = MxmFxEqN::default();
    for rate in [0.0, 999.0, f32::NAN, f32::INFINITY] {
        assert!(!plugin.prepare(rate, 2, false), "{rate}");
    }
    assert!(plugin.prepare(MIN_SAMPLE_RATE as f32, 2, false));
}

#[test]
fn a_hosts_block_sizes_change_nothing() {
    // Sample-accurate automation splits a host's buffer at every event, and the shell reads the
    // parameters at each piece; with nothing changing, the pieces must not change the sound.
    let render = |sizes: &[usize]| {
        let mut plugin = prepared(2);
        set_band(
            &plugin.params.bands[0],
            Some(Shape::LowShelf),
            300.0,
            6.0,
            0.7,
            Slope::Db12,
        );
        set_band(
            &plugin.params.bands[1],
            Some(Shape::HighCut),
            9_000.0,
            0.0,
            0.7,
            Slope::Db48,
        );
        let mut seed = 5;
        let input: Vec<f32> = (0..10_000).map(|_| noise(&mut seed) * 0.3).collect();
        let mut left = input.clone();
        let mut right = input;
        let mut start = 0;
        let mut which = 0;
        while start < left.len() {
            let end = (start + sizes[which % sizes.len()]).min(left.len());
            let mut channels: Vec<&mut [f32]> = vec![&mut left[start..end], &mut right[start..end]];
            plugin.process_block(&mut channels);
            start = end;
            which += 1;
        }
        left
    };
    let whole = render(&[10_000]);
    for sizes in [&[1][..], &[37][..], &[512, 3, 999, 64][..]] {
        assert_eq!(render(sizes), whole, "{sizes:?}");
    }
}

/// **The shell feeds the dynamics to the engine, and reports what they do only to an editor**
/// (plan §7.1, §4): a bell at 0 dB with Range −12 dB cuts a loud tone at its centre, inside its
/// Range; the drawn and heard gains are published only while an editor is connected, and a
/// connect clears them.
#[test]
fn a_dynamic_band_cuts_what_gets_loud_and_tells_only_an_editor() {
    let mut plugin = prepared(2);
    set_band(
        &plugin.params.bands[4],
        Some(Shape::Bell),
        1_000.0,
        0.0,
        2.0,
        Slope::Db12,
    );
    let dynamics = &plugin.params.dynamics[4];
    set_float(&dynamics.range, -12.0);
    set_float(&dynamics.threshold, -40.0);
    set_float(&dynamics.attack, 1.0);
    set_float(&dynamics.release, 50.0);
    let input: Vec<f32> = (0..96_000)
        .map(|i| (0.5 * (TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let mut buffers = vec![input.clone(), input.clone()];
    run(&mut plugin, &mut buffers);
    let heard = tone_db(&buffers[0], 1_000.0, 0.5);
    assert!(
        (-12.0..-6.0).contains(&heard),
        "a loud tone is cut: {heard} dB"
    );
    assert_eq!(
        plugin.telemetry.gains(4),
        (0.0, 0.0),
        "no editor, nothing published"
    );

    plugin.telemetry.connect(true);
    let mut again = vec![input.clone(), input];
    run(&mut plugin, &mut again);
    let (drawn, heard) = plugin.telemetry.gains(4);
    assert!(drawn.abs() < 1e-3, "the drawn gain stays 0 dB: {drawn}");
    assert!(
        (-12.0..-6.0).contains(&heard),
        "the heard gain is the cut: {heard}"
    );
    plugin.telemetry.connect(true);
    assert_eq!(
        plugin.telemetry.gains(4),
        (0.0, 0.0),
        "a connect clears them"
    );
}

/// **The heard scale is the engine's, where it has glided** (plan §7.1): published with the gains
/// while an editor is connected, between the old and new scale during the glide, and at the new
/// one once it has arrived.
#[test]
fn the_editor_hears_the_gain_scale_where_it_has_glided() {
    let mut plugin = prepared(2);
    plugin.telemetry.connect(true);
    let mut quiet = vec![vec![0.0f32; 512], vec![0.0f32; 512]];
    run(&mut plugin, &mut quiet);
    assert_eq!(plugin.telemetry.heard_scale(), 1.0);
    set_float(&plugin.params.gain_scale, 0.5);
    let mut short = vec![vec![0.0f32; 64], vec![0.0f32; 64]];
    run(&mut plugin, &mut short);
    let gliding = plugin.telemetry.heard_scale();
    assert!(0.5 < gliding && gliding < 1.0, "mid-glide: {gliding}");
    let mut long = vec![vec![0.0f32; 48_000], vec![0.0f32; 48_000]];
    run(&mut plugin, &mut long);
    assert!((plugin.telemetry.heard_scale() - 0.5).abs() < 1e-4);
}

/// **The shell hands the side chain to a keyed band** (plan §7.2): through the parameters, a loud
/// key cuts a quiet input, in whatever pieces the host's blocks arrive; with no port the band is as
/// drawn.
#[test]
fn a_keyed_band_hears_the_side_chain_through_the_shell() {
    let render = |chunk: usize, with_key: bool| {
        let mut plugin = prepared(2);
        set_band(
            &plugin.params.bands[0],
            Some(Shape::Bell),
            1_000.0,
            0.0,
            2.0,
            Slope::Db12,
        );
        let dynamics = &plugin.params.dynamics[0];
        set_float(&dynamics.range, -12.0);
        set_float(&dynamics.threshold, -40.0);
        set_float(&dynamics.attack, 1.0);
        set_bool(&plugin.params.keys[0].side_chain, true);
        let quiet: Vec<f32> = (0..48_000)
            .map(|i| (0.005 * (TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
            .collect();
        let loud: Vec<f32> = quiet.iter().map(|sample| sample * 100.0).collect();
        let mut left = quiet.clone();
        let mut right = quiet;
        let mut key_left = loud.clone();
        let mut key_right = loud;
        for start in (0..48_000).step_by(chunk) {
            let end = (start + chunk).min(48_000);
            let mut channels = [&mut left[start..end], &mut right[start..end]];
            let keys = [&mut key_left[start..end], &mut key_right[start..end]];
            plugin.process_keyed(&mut channels, with_key.then_some(&keys[..]));
        }
        left
    };
    let whole = render(48_000, true);
    assert!(
        tone_db(&whole, 1_000.0, 0.005) < -6.0,
        "the key cuts the quiet input"
    );
    assert_eq!(
        render(300, true),
        whole,
        "a host's block sizes change nothing"
    );
    assert!(tone_db(&render(48_000, false), 1_000.0, 0.005).abs() < 0.05);
}

/// **The shell feeds each slot's placement to the engine** (plan §7.3): a Left bell through the
/// parameters leaves the right bit for bit; and the editor learns the layout's channels.
#[test]
fn a_placement_set_through_its_parameter_is_heard_on_its_channel() {
    let mut plugin = prepared(2);
    set_band(
        &plugin.params.bands[0],
        Some(Shape::Bell),
        1_000.0,
        12.0,
        2.0,
        Slope::Db12,
    );
    set_int(&plugin.params.places[0].placement, 1);
    let input: Vec<f32> = (0..96_000)
        .map(|i| (0.25 * (TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let mut buffers = vec![input.clone(), input.clone()];
    run(&mut plugin, &mut buffers);
    assert_eq!(buffers[1], input, "the right is untouched");
    assert!(tone_db(&buffers[0][..], 1_000.0, 0.25) > 11.0);
    assert!(!plugin.telemetry.mono());
    let mono = prepared(1);
    assert!(mono.telemetry.mono(), "a mono activation is published");
}

/// **The shell copies the side chain only while the editor shows it** (plan §7.4): an editor, the
/// analyser on, a layout with the port and the Side chain view on; otherwise the block's key is
/// silence.
#[test]
fn the_side_chain_is_copied_only_while_its_view_wants_it() {
    let mut plugin = prepared(2);
    let mut consumer = plugin.analyser.take().unwrap();
    plugin.telemetry.connect(true);
    let input = vec![0.1f32; 512];
    let mut key = [vec![0.5f32; 512], vec![0.5f32; 512]];
    let mut keyed = |plugin: &mut MxmFxEqN| {
        let mut channels = [input.clone(), input.clone()];
        let mut views: Vec<&mut [f32]> = channels.iter_mut().map(|c| c.as_mut_slice()).collect();
        let keys: Vec<&mut [f32]> = key.iter_mut().map(|c| c.as_mut_slice()).collect();
        plugin.process_keyed(&mut views, Some(&keys));
    };
    keyed(&mut plugin);
    let block = consumer.pop().expect("a block");
    assert_eq!(
        block.key[0],
        [0.0; analyser::BLOCK_FRAMES],
        "no port, no view: silence"
    );
    while consumer.pop().is_ok() {}
    plugin.telemetry.publish_side_chain_port(true);
    plugin.telemetry.set_side_chain_view(true);
    assert!(plugin.telemetry.side_chain_wanted());
    keyed(&mut plugin);
    let block = consumer.pop().expect("a block");
    assert_eq!(
        block.key[0],
        [0.5; analyser::BLOCK_FRAMES],
        "the key while it is shown"
    );
    plugin.telemetry.set_analyser_on(false);
    assert!(
        !plugin.telemetry.side_chain_wanted(),
        "the analyser off wants none"
    );
}

/// **The key's copy allocates nothing** (plan §7.4): with the Side chain view wanted, keyed
/// processing with an editor open never allocates.
#[test]
fn copying_the_side_chain_never_allocates() {
    let mut plugin = prepared(2);
    plugin.telemetry.connect(true);
    plugin.telemetry.publish_side_chain_port(true);
    plugin.telemetry.set_side_chain_view(true);
    assert!(plugin.telemetry.side_chain_wanted());
    let mut consumer = plugin.analyser.take().expect("the ring's editor half");
    let mut seed = 5;
    let mut channels = [vec![0.0f32; 2_048], vec![0.0f32; 2_048]];
    let mut key = [vec![0.0f32; 2_048], vec![0.0f32; 2_048]];
    let mut blocks = 0;
    for _ in 0..40 {
        for buffer in channels.iter_mut().chain(key.iter_mut()) {
            for sample in buffer.iter_mut() {
                *sample = noise(&mut seed) * 0.25;
            }
        }
        let mut views: Vec<&mut [f32]> = channels.iter_mut().map(|c| c.as_mut_slice()).collect();
        let keys: Vec<&mut [f32]> = key.iter_mut().map(|c| c.as_mut_slice()).collect();
        nice_assert_no_alloc::assert_no_alloc(|| plugin.process_keyed(&mut views, Some(&keys)));
        while let Ok(block) = consumer.pop() {
            assert!(block.key[0].iter().any(|&s| s != 0.0), "the key rides");
            blocks += 1;
        }
    }
    assert!(blocks > 0);
}

// ---- Linear phase (plan §7.7) -----------------------------------------------------------------

/// An activation context that remembers the latency it was told.
#[derive(Default)]
struct Activation {
    latency: std::cell::Cell<Option<u32>>,
}

impl ActivateContext<MxmFxEqN> for Activation {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute(&self, _task: ()) {}
    fn set_latency_samples(&self, samples: u32) {
        self.latency.set(Some(samples));
    }
    fn set_current_voice_capacity(&self, _capacity: u32) {}
}

fn activated(phase: i32, resolution: i32, rate: f32) -> (MxmFxEqN, Option<u32>) {
    let mut plugin = MxmFxEqN::default();
    set_int(&plugin.params.phase, phase);
    set_int(&plugin.params.resolution, resolution);
    let mut context = Activation::default();
    let config = BufferConfig {
        sample_rate: rate,
        min_buffer_size: None,
        max_buffer_size: 1_024,
        process_mode: ProcessMode::Realtime,
    };
    assert!(plugin.activate(&MxmFxEqN::AUDIO_IO_LAYOUTS[2], &config, &mut context));
    (plugin, context.latency.get())
}

/// **Activation builds the phase the parameters hold and reports its latency** (plan §7.7), and
/// publishes it as the phase in force.
#[test]
fn activation_reports_the_latency_of_the_phase_held() {
    use mxm_fx_eq_n_dsp::linear::Sizes;
    use mxm_fx_eq_n_dsp::{Phase, Resolution};
    for rate in [48_000.0f32, 96_000.0] {
        let (plugin, latency) = activated(0, 1, rate);
        assert_eq!(latency, Some(0), "Zero latency");
        assert_eq!(plugin.telemetry.phase(), Phase::ZeroLatency);
        for (value, resolution) in Resolution::ALL.into_iter().enumerate() {
            let (plugin, latency) = activated(1, value as i32, rate);
            let want = Sizes::new(f64::from(rate), resolution).latency() as u32;
            assert_eq!(latency, Some(want), "{resolution:?} at {rate}");
            assert_eq!(plugin.engine.latency(), want);
            assert_eq!(plugin.telemetry.phase(), Phase::Linear(resolution));
        }
    }
}

/// **A change while active asks for a restart, and the phase in force plays on** (plan §7.7): its
/// latency is reported once; the engine and the editor's phase stay as activated until the host
/// restarts the plugin, which then builds the phase asked for.
#[test]
fn a_change_while_active_waits_for_the_restart() {
    use mxm_fx_eq_n_dsp::linear::Sizes;
    use mxm_fx_eq_n_dsp::{Phase, Resolution};
    let mut plugin = prepared(2);
    assert_eq!(plugin.phase_change(), None);
    set_int(&plugin.params.phase, 1);
    let want = Sizes::new(48_000.0, Resolution::Medium).latency() as u32;
    assert_eq!(plugin.phase_change(), Some(want), "reported once");
    assert_eq!(plugin.phase_change(), None);
    // A resolution change in Zero latency asks for nothing; in Linear phase it does.
    let mut buffers = vec![vec![0.25f32; 512], vec![-0.25f32; 512]];
    run(&mut plugin, &mut buffers);
    assert_eq!(
        buffers[0],
        vec![0.25f32; 512],
        "still Zero latency, bit-exact dry"
    );
    assert_eq!(plugin.telemetry.phase(), Phase::ZeroLatency);
    set_int(&plugin.params.resolution, 2);
    assert_eq!(
        plugin.phase_change(),
        Some(Sizes::new(48_000.0, Resolution::High).latency() as u32)
    );
    // The host's restart.
    assert!(plugin.prepare(48_000.0, 2, false));
    assert_eq!(plugin.telemetry.phase(), Phase::Linear(Resolution::High));
    assert_eq!(plugin.phase_change(), None);
}

/// **Through the shell, Linear phase delays an empty EQ by its latency, bit for bit, and a band
/// rings until its tail is out** (plan §7.7).
#[test]
fn linear_phase_through_the_shell() {
    let mut plugin = MxmFxEqN::default();
    set_int(&plugin.params.phase, 1);
    set_int(&plugin.params.resolution, 0);
    assert!(plugin.prepare(48_000.0, 2, false));
    let latency = plugin.engine.latency() as usize;
    let mut seed = 7u32;
    let input: Vec<f32> = (0..3 * latency).map(|_| 0.5 * noise(&mut seed)).collect();
    let mut buffers = vec![input.clone(), input.clone()];
    run(&mut plugin, &mut buffers);
    assert!(buffers[0][..latency].iter().all(|x| *x == 0.0));
    assert_eq!(&buffers[0][latency..], &input[..input.len() - latency]);

    set_band(
        &plugin.params.bands[0],
        Some(Shape::Bell),
        1_000.0,
        9.0,
        2.0,
        Slope::Db12,
    );
    // Silence in, every block (processing is in place).
    let silent = |plugin: &mut MxmFxEqN| {
        let mut quiet = vec![vec![0.0f32; 256], vec![0.0f32; 256]];
        run(plugin, &mut quiet)
    };
    let mut rang = false;
    for _ in 0..40 {
        if silent(&mut plugin) == ProcessStatus::KeepAlive {
            rang = true;
        }
    }
    assert!(rang, "kept alive while the tail was out");
    assert_eq!(
        silent(&mut plugin),
        ProcessStatus::Normal,
        "then free to sleep"
    );
}

/// **The asked phase is recorded as asked** (plan §7.7): activation in Linear phase reports no
/// further change while the parameters stay put, whatever the engine could build, so a design
/// thread that cannot start never loops the host's restarts.
#[test]
fn activation_records_the_phase_asked() {
    let mut plugin = MxmFxEqN::default();
    set_int(&plugin.params.phase, 1);
    assert!(plugin.prepare(48_000.0, 2, false));
    assert_eq!(plugin.phase_change(), None);
    assert_eq!(plugin.asked_phase, plugin.params.phase_mode());
}

/// **Auto gain through its parameter** (plan §7.8): a low and a high shelf, +6 dB each at 1 kHz,
/// come out at the input's level within 0.2 dB, and the make-up reaches an editor.
#[test]
fn auto_gain_makes_up_a_broadband_boost_through_the_shell() {
    let mut plugin = prepared(2);
    set_band(
        &plugin.params.bands[0],
        Some(Shape::LowShelf),
        1_000.0,
        6.0,
        0.71,
        Slope::Db12,
    );
    set_band(
        &plugin.params.bands[1],
        Some(Shape::HighShelf),
        1_000.0,
        6.0,
        0.71,
        Slope::Db12,
    );
    set_bool(&plugin.params.auto_gain, true);
    plugin.telemetry.connect(true);
    let mut seed = 11;
    let input: Vec<f32> = (0..48_000).map(|_| noise(&mut seed) * 0.25).collect();
    let mut buffers = [input.clone(), input.clone()];
    for at in (0..input.len()).step_by(512) {
        let end = (at + 512).min(input.len());
        let (l, r) = buffers.split_at_mut(1);
        let mut channels: [&mut [f32]; 2] = [&mut l[0][at..end], &mut r[0][at..end]];
        plugin.process_block(&mut channels);
    }
    let rms = |signal: &[f32]| {
        (signal.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / signal.len() as f64).sqrt()
    };
    let half = input.len() / 2;
    let change = 20.0 * (rms(&buffers[0][half..]) / rms(&input[half..])).log10();
    assert!(change.abs() < 0.2, "{change} dB");
    assert!(
        (plugin.telemetry.make_up() + 6.02).abs() < 0.1,
        "{}",
        plugin.telemetry.make_up()
    );
}

/// **The share tap** (plan §7.8): nothing while no editor watches; with a watch, post-EQ blocks
/// alone, pre silent; a watch from none starts a new generation.
#[test]
fn the_share_tap_copies_post_only_while_watched() {
    let mut plugin = prepared(1);
    set_band(
        &plugin.params.bands[0],
        Some(Shape::Bell),
        1_000.0,
        -12.0,
        1.0,
        Slope::Db12,
    );
    let input: Vec<f32> = (0..4_096)
        .map(|i| (0.5 * (TAU * 1_000.0 * i as f64 / 48_000.0).sin()) as f32)
        .collect();
    let mut after = None;
    let mut blocks: Vec<analyser::Block> = Vec::new();
    run(&mut plugin, &mut [input.clone()]);
    plugin
        .instance
        .read(&mut after, |block| blocks.push(*block));
    assert!(blocks.is_empty(), "unwatched, nothing");

    let watch = plugin.instance.watch();
    let mut buffers = vec![input.clone()];
    run(&mut plugin, &mut buffers);
    plugin
        .instance
        .read(&mut after, |block| blocks.push(*block));
    assert_eq!(blocks.len(), 4_096 / analyser::BLOCK_FRAMES);
    assert_eq!(
        &blocks[0].post[0][..],
        &buffers[0][..analyser::BLOCK_FRAMES]
    );
    assert_eq!(blocks[0].post[0], blocks[0].post[1], "mono in both");
    assert!(blocks[0].pre.iter().flatten().all(|x| *x == 0.0));
    let generation = blocks[0].generation;

    drop(watch);
    blocks.clear();
    run(&mut plugin, &mut [input.clone()]);
    plugin
        .instance
        .read(&mut after, |block| blocks.push(*block));
    assert!(blocks.is_empty(), "the watch ended");
    let _watch = plugin.instance.watch();
    run(&mut plugin, &mut [input]);
    plugin
        .instance
        .read(&mut after, |block| blocks.push(*block));
    assert!(!blocks.is_empty() && blocks.iter().all(|b| b.generation != generation));
}

/// The share tap's cost on the audio thread (plan §7.8), at 192 kHz, watched and not, as a share
/// of real time: `cargo test -p mxm-fx-eq-n --release --lib share_cost -- --ignored --nocapture`.
#[test]
#[ignore = "a release measurement, run by hand"]
fn share_cost() {
    let rate = 192_000.0;
    let measure = |watched: bool| {
        let mut plugin = MxmFxEqN::default();
        assert!(plugin.prepare(rate, 2, false));
        for (slot, band) in plugin.params.bands.iter().enumerate().take(8) {
            set_band(
                band,
                Some(Shape::Bell),
                100.0 * (slot + 1) as f32,
                3.0,
                1.0,
                Slope::Db12,
            );
        }
        let watch = watched.then(|| plugin.instance.watch());
        let mut after = None;
        let mut seed = 5;
        let mut buffers = vec![vec![0.0f32; 1_024], vec![0.0f32; 1_024]];
        let seconds = 10.0;
        let calls = (seconds * f64::from(rate) / 1_024.0) as usize;
        let mut spent = std::time::Duration::ZERO;
        for _ in 0..calls {
            for buffer in &mut buffers {
                for sample in buffer.iter_mut() {
                    *sample = noise(&mut seed) * 0.25;
                }
            }
            let (left, right) = buffers.split_at_mut(1);
            let mut channels: [&mut [f32]; 2] = [&mut left[0], &mut right[0]];
            let start = std::time::Instant::now();
            plugin.process_block(&mut channels);
            spent += start.elapsed();
            // The watching editor drains it, as each frame does.
            plugin.instance.read(&mut after, |block| {
                std::hint::black_box(block);
            });
        }
        drop(watch);
        spent.as_secs_f64() / seconds * 100.0
    };
    let (quiet, watched) = (measure(false), measure(true));
    println!(
        "192 kHz, 8 bells: {quiet:.2} % unwatched, {watched:.2} % watched, {:+.2} points",
        watched - quiet
    );
}
