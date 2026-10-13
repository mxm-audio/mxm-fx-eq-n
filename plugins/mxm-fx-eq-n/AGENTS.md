# AGENTS.md — plugins/mxm-fx-eq-n

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

`mxm-fx-eq-n`, an original parametric EQ with only the bands you need: up to 24, made on demand,
over [`crates/mxm-fx-eq-n-dsp`](../../crates/mxm-fx-eq-n-dsp/AGENTS.md). The product contract is
[`plans/plan-mxm-fx-eq-n.md`](../../plans/plan-mxm-fx-eq-n.md) and the signed brief
[`docs/briefs/mxm-fx-eq-n.md`](../../docs/briefs/mxm-fx-eq-n.md). **Built through E3**: the host
surface, the preset seam, solo's command path, the analyser and the meter (E2), and the editor (E3).
Factory presets and `host-tests/` are E4's.

# Ownership

- `src/lib.rs`: identity, layouts, the lifecycle and `process`.
- `src/params.rs`: the permanent parameters, their domains and readings.
- `src/preset.rs`: the `mxm_preset::Instrument` seam.
- `src/audition.rs`: band solo, the transient command.
- `src/telemetry.rs`: the editor link and the Out meter, two of mxm-kit's `PeakMeter`s.
- `src/analyser.rs`: the analyser's audio half.
- `src/view.rs`: the analyser's settings, persisted view state.
- `src/editor/`: the editor. `mod.rs` its state, writes and keyboard commands; `panel.rs` the frame;
  `graph.rs` the graph and its pointer; `column.rs` the cards; `history.rs` undo; `spectrum.rs` the
  analyser's editor half; `tests.rs` its tests.
- `src/tests.rs`: the shell's tests.

# Local Contracts

## Identity and host surface

- **`mxm-fx-eq-n`, CLAP id `dk.mxm.mxm-fx-eq-n`**, from `plugin_name!` and `bundler.toml`. The
  `-n` is descriptive (*n* bands), not a box number (root `AGENTS.md`, *Naming*).
- **Layouts:** stereo→stereo and mono→mono, the two channels processed identically (brief A1).
  No note port, so no developer channel (`plugins/AGENTS.md`, *A developer channel*).
- **146 permanent parameters** (`params.rs`):
  - 24 slots, nested, each with `type`, `bypass`, `freq`, `gain`, `q` and `slope`; their ids are
    `<field>_<slot>`, `type_1` … `slope_24`, and their host names carry the slot number ("Band 7
    frequency"), never the shape;
  - `output`, the output gain, as linear gain shown in dB;
  - `gainscale`, 0–200 %.

  `the_ids_are_complete_unique_permanent_and_146` holds them.
- **Automation belongs to the slot number** (decision 10). A new band takes the lowest Off slot,
  and a preset's band 3 is slot 3. No ownership state exists.

## The stepped domains are reserved from this first release

Saved projects and presets hold normalised values (mxm-kit's `crates/mxm-preset/AGENTS.md`), so a range that
grew would remap them.
- `type` is 0–31: 0 Off, 1–8 bell, low shelf, high shelf, low cut, high cut, notch, band pass and
  tilt shelf, and 9–31 unassigned.
- `slope` is 0–15: 0–7 are 6, 12, 18, 24, 36, 48, 72 and 96 dB/octave, and 8–15 unassigned.

An unassigned value reads *Unavailable n* to the host. The DSP gets Off for a type and 12 dB/octave
for a slope. A new shape or slope takes the next unassigned value.
`the_stepped_values_keep_their_normalised_places` pins every value's place.

## Deviations from the shared parameter rules, and why

- **A band's gain is stored in dB**, against *gain parameters store linear gain*. That rule exists
  because nice-plug's logarithmic smoother is invalid across a dB range spanning zero. No nice-plug
  smoother runs here. An EQ band's gain is its curve's height, which the DSP glides and scales in dB.
- **Nothing is smoothed by nice-plug.** The DSP glides frequency, gain and Q in their own domains
  and fades every structural change. A host smoother on top would make the heard curve lag the drawn
  one.
- **Frequency and Q are `Skewed`, not logarithmic**: nice-plug has no logarithmic range. The skews
  put the middle of the travel near 1 kHz and Q 1, where a log axis would. The editor works in the
  natural units and steps in octaves, semitones and cents (brief A4).
- **Readings choose their unit and precision from the rounded value** (`plugins/AGENTS.md`,
  *Parameters*). Frequency switches from Hz to kHz, and Q from three to two to one decimal places.
  These readings are the EQ's own, so they stay here rather than in `mxm_params::text`.
  `every_reading_survives_the_hosts_text_round_trip` runs the shared walk
  (`mxm_plugin_test::text_checks`) with Q's 9.995 switch as this plugin's one extra boundary, and
  the preset tests assert `mxm_preset::stale_factory_texts` is empty.

## Init is the empty EQ, and inserting the plugin changes nothing

The owner's decision 2 (2026-10-12): an exception to *an effect starts engaged*, as `mxm-fx-curve`
is. Init and every default leave every slot Off, the output at 0 dB and the gain scale at 100 %.
The engine then leaves the buffer untouched, NaN and subnormals included
(`init_is_bit_exact_dry_in_both_layouts`). Init is generated from the parameter defaults
(`the_init_preset_is_the_parameter_defaults_and_the_empty_eq`).

**Presets are parameters only** (plan §5.4): no custom state, no fingerprint, no dirty-only
transaction. The factory set is E4's.

## Band solo is a transient command, never a parameter

`plugins/AGENTS.md`'s transient-command exception, for one act. An audition is what somebody
listens to while holding a key or a button. Saved, a project would reopen soloed. Automated, a
render would play one band's band pass instead of the EQ. Recalled by a preset, it would change what
the EQ does without changing the EQ. So solo has no id, state or preset value.

`audition.rs` holds it: one atomic word with the band asked for and a count of the times the shell
ended a solo. The editor asks with the count it read, so a request made before an ending cannot
undo it, and a request to turn solo off fails only when the shell already turned it off.
- **The shell ends solo** in `reset` and `deactivate`. A state restore and an activation both run
  `reset` (the wrapper's state transaction reactivates and resets).
- **Each block it passes the request to the DSP's `set_solo`** only with an editor connected and
  never in an offline render.
- **Never waits, locks or allocates.**

The editor ends it on close (E3). `solo_needs_an_editor_never_plays_offline_and_ends_with_the_shell`
and `audition.rs`'s tests hold the lifecycle.

## The analyser's and the meter's handoffs

- **Analyser** (`analyser.rs`, plan §4): preallocated blocks of pre- and post-EQ audio, every channel
  kept, through a bounded `rtrb` ring. Every block made spends a sequence number, so a block dropped
  on a full ring is a visible gap. Activation, deactivation and reset start a new generation, and a
  block carries its sample rate. The audio thread copies nothing with the editor closed or the
  analyser off. `process` works in pieces of one analyser block, so the pre-EQ copy needs no buffer.
  The engine's control clock runs on across calls, so the pieces change no sample
  (`a_hosts_block_sizes_change_nothing`).
- **The ring's consumer goes to the editor once** (`MxmFxEqN::editor`). Its half is
  `editor/spectrum.rs`, below.
- **Out meter** (`telemetry.rs`): mxm-kit's `PeakMeter` per channel, the collection's one policy: a
  peak max-combined and reset on read, a clip at 0 dBFS latched until acknowledged, a non-finite
  peak a clip and never a level. The shell publishes nothing with no editor connected.

## Activity and rates

- **`KeepAlive` while any filter rings**, `Normal` once every state has settled to exact silence, so
  a host can sleep the plugin on silent input (`plugins/AGENTS.md`'s idle rule).
- **Activation below the DSP's `MIN_SAMPLE_RATE`**, or at a rate that is not finite, is refused.
- **Activation and `reset` seed the engine from the parameters at once** (`Engine::reset`), so a
  saved EQ is heard from its first sample rather than fading in from dry.

## The editor (plan §5)

- **Layout**, the named exception to paged cards (`plugins/AGENTS.md`): the graph takes every spare
  point, an Out meter rail (`shell::vertical_level_meter`, post output gain) stands beside it, and a
  fixed-width column of three cards, Band, Output and Analyser, sits to its right and never under
  it. Narrower than graph and column, the whole surface scrolls sideways with the column still
  beside; taller than the space, the column scrolls on its own. The window opens at `REFERENCE`,
  inside the quarter-4K budget and as tall as the column's tallest state
  (`the_opening_size_is_inside_the_budget_and_holds_the_column_without_scrolling`), and narrows to
  `MINIMUM` (`the_app_bar_holds_in_the_minimum_window`). Every card is one of the cursor's
  (`navigation::bar_card`), the graph first.
  Each card body is a `mxm_ui::tree` and passes the tree checks in every Band state; the Band card
  is reserved to its tallest shape, so nothing below it moves. With no band selected its title
  alone says so ("Band —").
- **The graph**: clicking the composite line adds a 0 dB bell in the lowest Off slot, writing every
  field of the slot, so nothing a deleted band left behind returns. **A point is drawn at its own
  gain** (Pro-Q's convention, the composite drawn separately); a gainless shape sits on 0 dB.
  Dragging sets frequency and gain as one gesture, Shift at a tenth of the speed. The wheel turns Q
  only over the selected point with the graph focused, or over any point with Alt, one gesture per
  turn. A right-click opens the shapes and Delete. Identity never rests on hue: every point carries
  its number, the selected one a ring, a bypassed one is hollow; pre is an outline and post a fill.
  Every point has an accessible name and text value.
- **The keyboard is the collection's keyboard language** (mxm-kit's design system §11, engine
  `mxm-keys`, the keymap in use in the folder newDAWn and every editor share), through the cursor
  (`navigation::paged_with_bar`): the arrows walk the column's controls and the bars, VIEW + arrows
  go between the graph and the cards, and F1 shows the keys. **The graph is a surface**
  (`navigation::surface`): with the cursor on it, the language's outputs come to `on_surface`.
  ← → select the previous or next band by frequency, and step along the line (a semitone, an
  octave with COARSE) where there is none; MOVE + ← → moves the band's frequency (a semitone,
  COARSE an octave, MICRO a cent) and ↑ ↓ its gain (0.5, 3, 0.1 dB); VALUE + ↑ ↓ its gain;
  EXTENT + ← → narrows or widens it (a share of Q's logarithmic travel); MICRO + ← → takes its start
  or end edge, which **MOVE then moves alone, the other edge staying put**, so a bell, notch or band
  pass moves its centre too (a shelf's or a cut's edge changes Q alone); DUPLICATE + arrows carries
  a copy in the lowest Off slot where they go; ADD makes a 0 dB bell at the keyboard's place;
  REMOVE, DELETE and RIPPLE remove the band; BACK with nothing to cancel lets go of the edge, then
  of the band. A gesture is one undo step when OUT or the release keeps it, and BACK puts every
  parameter it opened back. The keyboard's place is painted only once the keyboard is used
  (`navigation::shown`).
- **Solo is held, never latched**: from the press to the release of the Band card's Hold button, a
  Ctrl/Cmd press on a point, or OPEN (Enter) held with the cursor on Hold. A bypassed band, the tilt and an Off slot are
  refused, as the DSP refuses them; the editor ends solo when it closes.
- **Undo is plain** (plan §5.4; `editor/history.rs`). The editor's host context is a recorder, so
  every write is seen, from the column's controls, the graph and the keyboard alike, and the shared
  binding needed no observer. BACK during a drag puts the band back (`mxm_ui::drag::cancelled`),
  as the shared controls do. An action is the parameters one gesture changed; undo writes their
  "before" values as one gesture, redo their "after" values: Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z or
  Ctrl+Y, and the app bar's buttons. **Epochs**: whatever the preset bar writes (a load, Init) is a
  whole-patch replacement, and a host restore is counted by the view field's `set`; either clears
  the history. The history is the instance's and survives the editor closing; it keeps 256 actions.
- **The analyser** (`editor/spectrum.rs`, on mxm-kit's `mxm-fft`, plan §4): band power over a twelfth
  of an octave per drawn point, averaged over the channels, so a full-scale sine reads 0 dB and
  noise reads alike whatever the window. A short window of about 85 ms at the full rate, and below
  300 Hz a long window on a decimated stream (an eighth-order low pass to about 3 kHz), 0.34 s at
  Medium and 1.4 s at High. A gap or a new generation restarts both windows; a block at another
  rate is ignored. Rises show at once, falls at the chosen speed; the tilt pivots at 1 kHz.
  **Settings are view state** (`view.rs`): on/off, Show (pre, post, both), Resolution, Speed and
  Tilt, defaulting to 1.5 dB/octave, the research's corpus slope on a band-power display. They
  persist with the project, mark it dirty through a parameter-free GUI transaction, and are no
  preset's or undo's. With the analyser off nothing is copied or transformed.
- **The analyser has a budget on the GUI thread**, `spectrum::BUDGET_MS` of a 60 Hz frame.
  Measured on the Threadripper 3970X in release (`analyser_cost`, 450 points): 0.05–0.11 ms at
  44.1 kHz, 0.07–0.16 ms at 48, 0.08–0.19 ms at 96 and 0.13–0.35 ms at 192, the long window adding
  about 0.1 ms. Both FFT crates are optimised in dev builds, so a debug bundle animates too.

**Choices made for the owner's by-eye check** (plan E0.5, moved to E3): points at their own gain;
solo held, not latched; the bands' keys are the language's, MOVE on an edge keeping the other one.
The probe's band curves (owner: "mostly visual noise") are not drawn.

## Not here, by plan

- **No control map** (plan §6): no collection role fits on-demand bands, and a map is optional.
- **Factory presets and `host-tests/`** are E4's; DAW and listening checks are E5's.

# Work Guidance

- A parameter's id, its stepped domain and the place of each stepped value are permanent. Rename a
  field freely, never an id.
- A new shape or slope takes the next unassigned value, gets a reading, and joins the place test.

# Verification

```bash
cargo test -p mxm-fx-eq-n
cargo clippy -p mxm-fx-eq-n --all-targets
cargo xtask bundle mxm-fx-eq-n --release
clap-validator validate target/bundled/mxm-fx-eq-n.clap
```

The validator's random parameter fuzz once found a subnormal output, now flushed in the DSP. The
analyser's cost: `cargo test -p mxm-fx-eq-n --release --lib analyser_cost -- --ignored --nocapture`.
The pictures: `cargo test -p mxm-fx-eq-n --lib pictures`, into `target/layout-tree/mxm-fx-eq-n/`.
There are no `host-tests/` until E4.

# Child DOX Index

No child AGENTS.md files.
