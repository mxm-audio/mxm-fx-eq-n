//! `mxm-fx-eq-n` through MXM Player's ordinary discovery, its state path and its serial effect
//! chain, against the release bundle (plan §8, *A loadable, valid plugin*).
//!
//! Product code is not linked into the host: a band is set the way a host restores a saved EQ,
//! through the CLAP state. These hold the shipping boundary: the bundle is classified as an effect
//! and offers its editor; inserted at Init it changes nothing, to the bit; a restored band is read
//! back under its slot's own name, is heard, and rings down to exact silence; a dynamic band
//! (F1) cuts the loud note, and a state from before F1 comes back with every band static; a
//! restored Linear phase (F6) delays the render by its latency, to the bit; and the player's bypass
//! restores the dry render to the bit.

use mxm_player_harness::app_harness;
use mxm_player_harness::harness;

use clack_extensions::state::PluginState;
use clack_host::prelude::*;
use harness::{CHANNELS, Harness};
use mxm_player::engine::Engine;
use mxm_player::events::input::Payload;
use mxm_player::host::MxmHost;
use mxm_player::params::ParamSet;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

const SOURCE_ID: &str = "dk.mxm.mxm-mono-01";
const EFFECT_ID: &str = "dk.mxm.mxm-fx-eq-n";
const BLOCK: usize = 256;
// A held note and its release, then room for the source's release and the band's ringing (well
// under a second at 48 kHz) to end in exact silence.
const BLOCKS: usize = 1_600;

/// This plugin's release bundle, or `None` when it has not been built: **the one skip**.
fn eq_n() -> Option<PathBuf> {
    let found = mxm_player_harness::host_test::plugin_bundle("mxm-fx-eq-n").map(|(_, file)| file);
    if found.is_none() {
        eprintln!("skipping: run `cargo xtask bundle mxm-fx-eq-n --release`");
    }
    found
}

/// The source the chain plays, fetched beside the bundle (`test-bundles.txt`). Required once the
/// bundle exists, so a fetch that failed fails the tests rather than passing them unrun.
fn source() -> PathBuf {
    let path = app_harness::workspace_root().join("target/bundled/mxm-mono-01.clap");
    assert!(
        path.exists(),
        "{} is missing: run `cargo xtask fetch`",
        path.display()
    );
    path
}

#[test]
fn ordinary_discovery_classifies_the_bundle_as_an_effect_with_no_map() {
    let Some(bundle) = eq_n() else {
        return;
    };
    let directory = bundle.parent().unwrap().to_owned();
    let mut app = app_harness::AppHarness::new("fx-eq-n-discovery", vec![directory]);
    app.harness.state_mut().rescan();
    app.run();
    let state = app.state();
    let found = state
        .found
        .iter()
        .find(|found| found.id == EFFECT_ID)
        .unwrap_or_else(|| panic!("ordinary discovery did not find {EFFECT_ID}"));
    assert!(
        found.effect,
        "the one-input bundle was not classified as an effect"
    );
    // No collection role fits on-demand bands, so no control map ships (plan §6).
    assert!(!app.app().control_map().knows_instrument(EFFECT_ID));
}

#[test]
fn the_editor_is_offered_through_the_players_effect_path() {
    let Some(bundle) = eq_n() else {
        return;
    };
    let mut engine = Engine::new();
    engine.add_fx(&bundle, EFFECT_ID).expect("the EQ loads");
    assert!(
        engine.fx_editor_floating_supported(0),
        "the editor does not support the player's floating effect window"
    );
    // The player has no side chains: it takes the first configuration with one input, the plain
    // stereo one, after the two keyed ones (plan §7.2).
    let selection = &engine.fx_envelope(0).expect("an envelope").selection;
    assert!(
        selection.starts_with("configuration 2 "),
        "the player chose {selection}"
    );
}

fn save_state(instance: &mut PluginInstance<MxmHost>) -> Value {
    let state: PluginState = instance
        .plugin_shared_handle()
        .get_extension()
        .expect("state extension");
    let mut bytes = Vec::new();
    state
        .save(&mut instance.plugin_handle(), &mut bytes)
        .expect("state saves");
    let size = u64::from_le_bytes(bytes[..8].try_into().expect("CLAP state length prefix"));
    assert_eq!(size as usize, bytes.len() - 8);
    serde_json::from_slice(&bytes[8..]).expect("uncompressed nice-plug state")
}

fn load_state(instance: &mut PluginInstance<MxmHost>, value: &Value) {
    let json = serde_json::to_vec(value).expect("state JSON");
    let mut bytes = Vec::with_capacity(json.len() + 8);
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&json);
    let state: PluginState = instance
        .plugin_shared_handle()
        .get_extension()
        .expect("state extension");
    state
        .load(&mut instance.plugin_handle(), &mut &bytes[..])
        .expect("state loads");
}

/// What the chain holds for the render.
#[derive(Clone, Copy, PartialEq)]
enum Chain {
    /// The source alone.
    Dry,
    /// The EQ as inserted, at Init.
    Init,
    /// Slot 1 restored as a 96 dB/octave high cut at 50 Hz: every partial the source plays (C4 and
    /// a sub an octave under it) lies more than an octave above the corner, and only what moves
    /// slower — the note's envelope, any offset — passes.
    HighCut,
    /// Slot 1 restored as a broad dynamic bell at the note, drawn at 0 dB, Range −24 dB from a
    /// −60 dB threshold: it cuts only what gets loud, and the note does.
    Dynamic,
    /// The high cut, placed Left (plan §7.3): the left is cut and the right is the dry one.
    LeftCut,
    /// The empty EQ in Linear phase at Low (plan §7.7): the dry render, delayed by the latency.
    Linear,
    /// A low and a high shelf, +6 dB each at 1 kHz, with auto gain on (plan §7.8): the dry level.
    AutoGain,
}

/// Linear phase's latency at Low at the harness's 48 kHz: the kernel's centre and a partition.
const LINEAR_LOW_LATENCY: usize = 2_048 + 256;

/// The restored band, written into the instance's own saved state, values plain as nice-plug
/// stores them. The default slot is checked first, so a changed state format fails here by name.
fn restore_high_cut(instance: &mut PluginInstance<MxmHost>) {
    let mut state = save_state(instance);
    let params = state["params"].as_object_mut().expect("parameter map");
    assert_eq!(params.len(), 293);
    assert_eq!(
        params["type_1"],
        json!({"i32": 0}),
        "slot 1 is not Off at Init"
    );
    params.insert("type_1".into(), json!({"i32": 5}));
    params.insert("slope_1".into(), json!({"i32": 7}));
    params.insert("freq_1".into(), json!({"f32": 50.0}));
    load_state(instance, &state);

    // The host reads the band back under its slot's name, never its shape's.
    let read = ParamSet::read(instance);
    let text = |name: &str| {
        read.params
            .iter()
            .find(|param| param.name == name)
            .unwrap_or_else(|| panic!("no parameter called {name}"))
            .text
            .clone()
    };
    assert_eq!(text("Band 1 type"), "High cut");
    assert_eq!(text("Band 1 slope"), "96 dB/oct");
    assert_eq!(text("Band 1 frequency"), "50.0 Hz");
    assert_eq!(text("Band 2 type"), "Off");
}

/// The dynamic bell (`Chain::Dynamic`), written into the instance's own saved state.
fn dynamic_bell(instance: &mut PluginInstance<MxmHost>) -> Value {
    let mut state = save_state(instance);
    let params = state["params"].as_object_mut().expect("parameter map");
    assert_eq!(
        params["range_1"],
        json!({"f32": 0.0}),
        "slot 1 is not static at Init"
    );
    params.insert("type_1".into(), json!({"i32": 1}));
    params.insert("freq_1".into(), json!({"f32": 262.0}));
    params.insert("q_1".into(), json!({"f32": 0.5}));
    params.insert("range_1".into(), json!({"f32": -24.0}));
    params.insert("threshold_1".into(), json!({"f32": -60.0}));
    params.insert("attack_1".into(), json!({"f32": 0.5}));
    params.insert("release_1".into(), json!({"f32": 10.0}));
    state
}

/// A host parameter's text, by the name the host shows.
fn text(instance: &mut PluginInstance<MxmHost>, name: &str) -> String {
    ParamSet::read(instance)
        .params
        .iter()
        .find(|param| param.name == name)
        .unwrap_or_else(|| panic!("no parameter called {name}"))
        .text
        .clone()
}

fn render(chain: Chain, bypassed: bool) -> Option<Vec<f32>> {
    let effect = eq_n()?;
    let source = source();
    let effects: Vec<(&Path, &str)> = if chain == Chain::Dry {
        vec![]
    } else {
        vec![(effect.as_path(), EFFECT_ID)]
    };
    let mut harness = Harness::with_fx(&source, SOURCE_ID, 1, &effects).expect("the chain builds");
    if let Some(effect) = harness.fx.first_mut() {
        if chain == Chain::HighCut {
            restore_high_cut(&mut effect.instance);
        }
        if chain == Chain::LeftCut {
            let mut state = save_state(&mut effect.instance);
            let params = state["params"].as_object_mut().expect("parameter map");
            assert_eq!(params.len(), 293);
            params.insert("type_1".into(), json!({"i32": 5}));
            params.insert("slope_1".into(), json!({"i32": 7}));
            params.insert("freq_1".into(), json!({"f32": 50.0}));
            params.insert("place_1".into(), json!({"i32": 1}));
            load_state(&mut effect.instance, &state);
            assert_eq!(text(&mut effect.instance, "Band 1 placement"), "Left");
        }
        if chain == Chain::Dynamic {
            let state = dynamic_bell(&mut effect.instance);
            load_state(&mut effect.instance, &state);
            assert_eq!(text(&mut effect.instance, "Band 1 range"), "-24.0 dB");
        }
        if chain == Chain::Linear {
            let mut state = save_state(&mut effect.instance);
            let params = state["params"].as_object_mut().expect("parameter map");
            assert_eq!(params["phase"], json!({"i32": 0}), "Init is Zero latency");
            params.insert("phase".into(), json!({"i32": 1}));
            params.insert("resolution".into(), json!({"i32": 0}));
            load_state(&mut effect.instance, &state);
            assert_eq!(text(&mut effect.instance, "Phase"), "Linear phase");
            assert_eq!(text(&mut effect.instance, "Resolution"), "Low");
        }
        if chain == Chain::AutoGain {
            let mut state = save_state(&mut effect.instance);
            let params = state["params"].as_object_mut().expect("parameter map");
            assert_eq!(params["autogain"], json!({"bool": false}), "Init is off");
            for (slot, kind) in [(1, 2), (2, 3)] {
                params.insert(format!("type_{slot}"), json!({"i32": kind}));
                params.insert(format!("freq_{slot}"), json!({"f32": 1_000.0}));
                params.insert(format!("gain_{slot}"), json!({"f32": 6.0}));
                params.insert(format!("q_{slot}"), json!({"f32": 0.71}));
            }
            params.insert("autogain".into(), json!({"bool": true}));
            load_state(&mut effect.instance, &state);
            assert_eq!(text(&mut effect.instance, "Band 1 type"), "Low shelf");
            assert_eq!(text(&mut effect.instance, "Auto gain"), "On");
        }
        effect.bypassed.store(bypassed, Ordering::Relaxed);
    }

    let mut output = Vec::with_capacity(BLOCKS * BLOCK * CHANNELS);
    for block in 0..BLOCKS {
        if block == 2 {
            assert!(harness.push_at(
                0,
                0,
                Payload::NoteOn {
                    channel: 0,
                    key: 60,
                    velocity: 100.0 / 127.0,
                },
            ));
        }
        if block == 28 {
            assert!(harness.push_at(
                0,
                0,
                Payload::NoteOff {
                    channel: 0,
                    key: 60,
                    velocity: 0.0,
                },
            ));
        }
        output.extend_from_slice(harness.render(BLOCK));
    }
    harness.shutdown();
    Some(output)
}

fn rms(samples: &[f32]) -> f64 {
    (samples
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt()
}

/// **Inserting the plugin changes nothing** (decision 2): Init is the empty EQ, bit-exact dry.
#[test]
fn inserted_at_init_it_changes_nothing_to_the_bit() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let wet = render(Chain::Init, false).expect("the bundle existed above");
    assert!(
        dry.iter().any(|sample| *sample != 0.0),
        "the source is silent"
    );
    assert_eq!(wet, dry);
}

/// **A saved EQ is heard through the host from its restore**: the high cut takes the note down by
/// more than 20 dB (about 39 measured, 2026-10-14), adds nothing before the source, and rings down
/// to exact silence after it.
#[test]
fn a_restored_band_is_heard_and_ends_in_exact_silence() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let wet = render(Chain::HighCut, false).expect("the bundle existed above");
    assert!(wet.iter().all(|sample| sample.is_finite()));
    assert!(
        rms(&wet) < rms(&dry) / 10.0,
        "the high cut left {} of {} RMS",
        rms(&wet),
        rms(&dry)
    );
    let first = dry
        .chunks(CHANNELS)
        .position(|frame| frame.iter().any(|sample| *sample != 0.0))
        .expect("the source sounds");
    assert!(
        wet[..first * CHANNELS].iter().all(|sample| *sample == 0.0),
        "the EQ sounded before its source"
    );
    let end = BLOCKS * BLOCK;
    assert!(
        wet[(end - 2 * BLOCK) * CHANNELS..]
            .iter()
            .all(|sample| *sample == 0.0),
        "the band must ring down to exact silence"
    );
}

#[test]
fn player_bypass_restores_the_dry_render_to_the_bit() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let bypassed = render(Chain::HighCut, true).expect("the bundle existed above");
    assert_eq!(bypassed, dry);
}

/// **A dynamic band cuts what gets loud, through the host** (F1, plan §7.1): drawn at 0 dB, the bell
/// takes the held note down, and still ends in exact silence.
#[test]
fn a_restored_dynamic_band_cuts_the_loud_note() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let wet = render(Chain::Dynamic, false).expect("the bundle existed above");
    assert!(wet.iter().all(|sample| sample.is_finite()));
    assert!(
        rms(&wet) < rms(&dry) * 0.7,
        "the dynamic bell left {} of {} RMS",
        rms(&wet),
        rms(&dry)
    );
    let end = BLOCKS * BLOCK;
    assert!(
        wet[(end - 2 * BLOCK) * CHANNELS..]
            .iter()
            .all(|sample| *sample == 0.0),
        "the band must come to rest in exact silence"
    );
}

/// **A state from before F1 comes back static** (plan §7.1): the wrapper's own load runs
/// `filter_state`, so a state without the dynamics ids restores Range 0 over an instance that had
/// one, rather than keeping it.
#[test]
fn an_older_state_restores_every_band_static_through_the_host() {
    let Some(effect) = eq_n() else {
        return;
    };
    let effects = [(effect.as_path(), EFFECT_ID)];
    let mut harness =
        Harness::with_fx(&source(), SOURCE_ID, 1, &effects).expect("the chain builds");
    let instance = &mut harness.fx[0].instance;
    let dynamic = dynamic_bell(instance);
    load_state(instance, &dynamic);
    assert_eq!(text(instance, "Band 1 range"), "-24.0 dB");

    let mut older = dynamic.clone();
    let params = older["params"].as_object_mut().expect("parameter map");
    params.retain(|id, _| {
        ![
            "range_",
            "threshold_",
            "attack_",
            "release_",
            "key_",
            "place_",
        ]
        .iter()
        .any(|field| id.starts_with(field))
            && !["phase", "resolution", "autogain"].contains(&id.as_str())
    });
    assert_eq!(params.len(), 146);
    load_state(instance, &older);
    assert_eq!(text(instance, "Band 1 type"), "Bell");
    assert_eq!(text(instance, "Band 1 range"), "0.0 dB");
    assert_eq!(text(instance, "Band 1 release"), "150 ms");
    harness.shutdown();
}

/// **A placed band is heard on its channel, through the host** (plan §7.3): the Left high cut takes
/// the left down and leaves the right exactly the dry render's.
#[test]
fn a_restored_placed_band_is_heard_on_its_channel_only() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let wet = render(Chain::LeftCut, false).expect("the bundle existed above");
    let channel = |samples: &[f32], index: usize| -> Vec<f32> {
        samples.chunks(CHANNELS).map(|frame| frame[index]).collect()
    };
    assert_eq!(
        channel(&wet, 1),
        channel(&dry, 1),
        "the right is the dry render"
    );
    assert!(
        rms(&channel(&wet, 0)) < rms(&channel(&dry, 0)) / 10.0,
        "the left is cut"
    );
}

/// **A restored Linear phase delays the render by its latency, to the bit** (plan §7.7): the empty
/// EQ in Linear phase is a pure delay, so through the player the render is the dry one, later by
/// exactly the latency activation reported.
#[test]
fn a_restored_linear_phase_eq_delays_the_render_by_its_latency() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let wet = render(Chain::Linear, false).expect("the bundle existed above");
    let shift = LINEAR_LOW_LATENCY * CHANNELS;
    assert!(wet[..shift].iter().all(|sample| *sample == 0.0));
    assert_eq!(&wet[shift..], &dry[..dry.len() - shift]);
}

/// **Auto gain through the host** (plan §7.8): a restored low and high shelf, +6 dB each, with auto
/// gain on, renders within 0.1 dB of the dry level.
#[test]
fn a_restored_broadband_boost_with_auto_gain_renders_at_the_dry_level() {
    let Some(dry) = render(Chain::Dry, false) else {
        return;
    };
    let wet = render(Chain::AutoGain, false).expect("the bundle existed above");
    let change = 20.0 * (rms(&wet) / rms(&dry)).log10();
    assert!(change.abs() < 0.1, "{change} dB");
}

/// The EQ's VST3 class as a host names it: `mxm_vst3_host`'s prefix and its `moduleinfo.json` CID.
const VST3_ID: &str = "vst3:18E77858447698700CE2E56DD9FAC675";

/// This plugin's VST3 release bundle beside its CLAP, or `None` when it has not been built.
fn eq_n_vst3() -> Option<PathBuf> {
    let found = eq_n()
        .map(|clap| clap.with_extension("vst3"))
        .filter(|vst3| vst3.exists());
    if found.is_none() {
        eprintln!("skipping the VST3: run `cargo xtask bundle mxm-fx-eq-n --release`");
    }
    found
}

/// The plugin's state, nice-plug's JSON: under the CLAP its whole stream after a length, under the
/// VST3 `mxm_vst3_host`'s blob's component part (`MXV3`, a version, then each part's length).
fn state_of(instance: &mut PluginInstance<MxmHost>, vst3: bool) -> Value {
    if !vst3 {
        return save_state(instance);
    }
    let state: PluginState = instance
        .plugin_shared_handle()
        .get_extension()
        .expect("state extension");
    let mut blob = Vec::new();
    state
        .save(&mut instance.plugin_handle(), &mut blob)
        .expect("state saves");
    assert_eq!(&blob[..4], b"MXV3");
    let length = u64::from_le_bytes(blob[8..16].try_into().expect("a part's length")) as usize;
    serde_json::from_slice(&blob[16..16 + length]).expect("uncompressed nice-plug state")
}

/// `value` restored, as [`state_of`] frames it, the controller's part empty; the host's refusal
/// as an error.
fn restore(
    instance: &mut PluginInstance<MxmHost>,
    value: &Value,
    vst3: bool,
) -> Result<(), String> {
    let json = serde_json::to_vec(value).expect("state JSON");
    let mut bytes = Vec::new();
    if vst3 {
        bytes.extend_from_slice(b"MXV3");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&json);
        bytes.extend_from_slice(&0u64.to_le_bytes());
    } else {
        bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&json);
    }
    let state: PluginState = instance
        .plugin_shared_handle()
        .get_extension()
        .expect("state extension");
    state
        .load(&mut instance.plugin_handle(), &mut &bytes[..])
        .map_err(|error| format!("{error:?}"))
}

/// **A restore that reads back changed keeps its bands** (plan §7.8): nice-plug rolls back a whole
/// restore when a persisted field reads back other than it was saved without saying so. One EQ's
/// state into a second in the same process, as a duplicated track has it: the second takes a new
/// id and keeps the band. Then a state whose view settings and learn are from before fields they
/// now have: each band still comes back. **A state a field refuses is refused whole**, the bands as
/// they were. Through the CLAP and, as DaVinci Resolve loads it, the VST3.
#[test]
fn a_restore_that_reads_back_changed_keeps_its_bands() {
    let Some(effect) = eq_n() else {
        return;
    };
    restore_reads_back_changed(&effect, EFFECT_ID, false);
    if let Some(vst3) = eq_n_vst3() {
        restore_reads_back_changed(&vst3, VST3_ID, true);
    }
}

fn restore_reads_back_changed(effect: &Path, id: &str, vst3: bool) {
    let format = if vst3 { "VST3" } else { "CLAP" };
    let effects = [(effect, id), (effect, id)];
    let mut harness =
        Harness::with_fx(&source(), SOURCE_ID, 1, &effects).expect("the chain builds");
    let mut state = state_of(&mut harness.fx[0].instance, vst3);
    let params = state["params"].as_object_mut().expect("parameter map");
    params.insert("type_1".into(), json!({"i32": 1}));
    params.insert("freq_1".into(), json!({"f32": 262.0}));
    restore(&mut harness.fx[0].instance, &state, vst3).expect("the state loads");
    let held = state_of(&mut harness.fx[0].instance, vst3)["fields"]["instance"].clone();
    restore(&mut harness.fx[1].instance, &state, vst3)
        .unwrap_or_else(|error| panic!("{format}: a taken id refused the state: {error}"));
    assert_eq!(
        text(&mut harness.fx[1].instance, "Band 1 type"),
        "Bell",
        "{format}"
    );
    assert_ne!(
        state_of(&mut harness.fx[1].instance, vst3)["fields"]["instance"],
        held,
        "{format}: the id was taken"
    );

    let older = [
        ("view", json!(r#"{"analyser":false}"#), 2, "Low shelf"),
        (
            "match-learned",
            json!(
                r#"{"cells_per_octave":48.0,"lowest_hz":20.0,"input":[],"output":[],"seconds":1.0,"sample_rate":48000.0}"#
            ),
            3,
            "High shelf",
        ),
    ];
    for (key, saved, kind, shape) in older {
        let fields = state["fields"].as_object_mut().expect("persisted fields");
        assert!(fields.contains_key(key), "{key}: {fields:?}");
        fields.insert(key.into(), saved);
        let params = state["params"].as_object_mut().expect("parameter map");
        params.insert("type_1".into(), json!({"i32": kind}));
        restore(&mut harness.fx[0].instance, &state, vst3)
            .unwrap_or_else(|error| panic!("{format} {key}: refused: {error}"));
        assert_eq!(
            text(&mut harness.fx[0].instance, "Band 1 type"),
            shape,
            "{format} {key}"
        );
    }

    // A field that cannot be read: refused, and the high shelf stays.
    let fields = state["fields"].as_object_mut().expect("persisted fields");
    fields.insert("match-to".into(), json!("not a target"));
    let params = state["params"].as_object_mut().expect("parameter map");
    params.insert("type_1".into(), json!({"i32": 4}));
    assert!(
        restore(&mut harness.fx[0].instance, &state, vst3).is_err(),
        "{format}: a field refused the state"
    );
    assert_eq!(
        text(&mut harness.fx[0].instance, "Band 1 type"),
        "High shelf",
        "{format}: rolled back whole"
    );
    harness.shutdown();
}

/// **The instance id is the host's state, never a preset's** (plan §7.8): restored, it is saved
/// again as it came.
#[test]
fn the_instance_id_survives_the_hosts_state() {
    let Some(effect) = eq_n() else {
        return;
    };
    let source = source();
    let mut harness = Harness::with_fx(&source, SOURCE_ID, 1, &[(effect.as_path(), EFFECT_ID)])
        .expect("the chain builds");
    let instance = &mut harness.fx[0].instance;
    let mut state = save_state(instance);
    let fields = state["fields"].as_object_mut().expect("persisted fields");
    assert!(fields.contains_key("instance"), "{fields:?}");
    fields.insert("instance".into(), json!("1234567890123"));
    load_state(instance, &state);
    assert_eq!(
        save_state(instance)["fields"]["instance"],
        json!("1234567890123")
    );
    harness.shutdown();
}
