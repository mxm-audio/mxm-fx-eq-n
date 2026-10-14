//! `mxm-fx-eq-n` through MXM Player's ordinary discovery, its state path and its serial effect
//! chain, against the release bundle (plan §8, *A loadable, valid plugin*).
//!
//! Product code is not linked into the host: a band is set the way a host restores a saved EQ,
//! through the CLAP state. These hold the shipping boundary: the bundle is classified as an effect
//! and offers its editor; inserted at Init it changes nothing, to the bit; a restored band is read
//! back under its slot's own name, is heard, and rings down to exact silence; and the player's
//! bypass restores the dry render to the bit.

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
}

/// The restored band, written into the instance's own saved state, values plain as nice-plug
/// stores them. The default slot is checked first, so a changed state format fails here by name.
fn restore_high_cut(instance: &mut PluginInstance<MxmHost>) {
    let mut state = save_state(instance);
    let params = state["params"].as_object_mut().expect("parameter map");
    assert_eq!(params.len(), 146);
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
