# AGENTS.md — plugins/mxm-fx-eq-n

Parent: [`../AGENTS.md`](../AGENTS.md). The detail behind the rules: [NOTES.md](NOTES.md).

# Purpose

`mxm-fx-eq-n`, an original parametric EQ with only the bands you need: up to 24, made on demand,
over [`crates/mxm-fx-eq-n-dsp`](../../crates/mxm-fx-eq-n-dsp/AGENTS.md). The product contract is
[`plans/plan-mxm-fx-eq-n.md`](../../plans/plan-mxm-fx-eq-n.md) and the signed brief
[`docs/briefs/mxm-fx-eq-n.md`](../../docs/briefs/mxm-fx-eq-n.md). **Built through E4**: the host
surface, the preset seam, solo's command path, the analyser and the meter (E2), the editor (E3),
and the factory set and the host tests (E4), and F1's dynamic bands through the shell, the editor
and two presets (F1.1–F1.4). DAW and listening checks are E5's and F1.5's.

# Ownership

- `src/lib.rs`: identity, layouts, the lifecycle and `process`.
- `src/params.rs`: the permanent parameters, their domains and readings.
- `src/preset.rs`: the `mxm_preset::Instrument` seam and the factory set's design.
- `presets/`: the factory set, generated from that design, never hand-edited.
- `src/audition.rs`: band solo, the transient command.
- `src/telemetry.rs`: the editor link and the Out meter, two of mxm-kit's `PeakMeter`s.
- `src/analyser.rs`: the analyser's audio half.
- `src/view.rs`: the analyser's settings, persisted view state.
- `src/editor/`: the editor. `mod.rs` its state, writes and keyboard commands; `panel.rs` the frame;
  `graph.rs` the graph and its pointer; `column.rs` the cards; `history.rs` undo; `spectrum.rs` the
  analyser's editor half; `tests.rs` its tests.
- `src/tests.rs`: the shell's tests.
- `host-tests/`: the slow tier, the release bundle through MXM Player.

# Local Contracts

## Identity and host surface

- **`mxm-fx-eq-n`, CLAP id `dk.mxm.mxm-fx-eq-n`**, from `plugin_name!` and `bundler.toml`. The
  `-n` is descriptive (*n* bands), not a box number (root `AGENTS.md`, *Naming*).
- **Layouts:** stereo→stereo and mono→mono, the two channels processed identically (brief A1).
  No note port, so no developer channel (`plugins/AGENTS.md`, *A developer channel*).
- **242 permanent parameters** (`params.rs`):
  - 24 slots, nested, each with `type`, `bypass`, `freq`, `gain`, `q` and `slope`; their ids are
    `<field>_<slot>`, `type_1` … `slope_24`, and their host names carry the slot number ("Band 7
    frequency"), never the shape;
  - `output`, the output gain, as linear gain shown in dB;
  - `gainscale`, 0–200 %;
  - F1's dynamics, four per slot after the globals (plan §7.1): `range_<n>`, `threshold_<n>`,
    `attack_<n>` and `release_<n>`, named "Band 7 range" and grouped with their slot; their place
    in the editor's indices starts at `params::DYNAMICS_BASE`, so no v1 index moved.

  `the_ids_are_complete_unique_permanent_and_242` holds them.
- **A state or preset from before F1 leaves every band static**: a preset through
  `default_missing_legacy_parameter`, a host state through `filter_state`
  (`preset::add_dynamics_at_defaults`), which also extends a loaded preset's baseline so it stays
  clean.
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

## Deviations from the shared parameter rules — [NOTES.md § Deviations](NOTES.md#deviations-from-the-shared-parameter-rules-and-why)

- **A band's gain is stored in dB**, against *gain parameters store linear gain*: no nice-plug
  smoother runs here, and **nothing is smoothed by nice-plug**; the DSP glides and fades.
- **Frequency and Q are `Skewed`**, the middle of the travel near 1 kHz and Q 1.
- **Readings choose their unit and precision from the rounded value**, and are the EQ's own.
  `every_reading_survives_the_hosts_text_round_trip` runs `mxm_plugin_test::text_checks` with Q's
  9.995 switch as the one extra boundary; the preset tests assert
  `mxm_preset::stale_factory_texts` is empty.

## Init is the empty EQ, and inserting the plugin changes nothing

The owner's decision 2 (2026-10-12): an exception to *an effect starts engaged*, as `mxm-fx-curve`
is. Init and every default leave every slot Off, the output at 0 dB and the gain scale at 100 %.
The engine then leaves the buffer untouched, NaN and subnormals included
(`init_is_bit_exact_dry_in_both_layouts`). Init is generated from the parameter defaults
(`the_init_preset_is_the_parameter_defaults_and_the_empty_eq`).

**Presets are parameters only** (plan §5.4): no custom state, no fingerprint, no dirty-only
transaction.

## The factory set is generated — [NOTES.md § The factory set](NOTES.md#the-factory-set)

- **`FACTORY_DESIGN`** (`preset.rs`'s tests) states each preset in the editor's units, a band at
  a time; `write_the_factory_presets` (ignored) writes `presets/`, texts read in each preset's own
  state, and `shipped_files_are_exactly_the_readable_design` fails when a file differs.
- **A preset's bands fill slots 1…n**, none bypassed, the rest Off, and the output and gain scale
  stay at Init's, so a band the player adds takes the next slot and a preset changes the sound
  only through its bands (`every_factory_preset_is_heard_and_fills_the_lowest_slots`, which also
  asks the shipped cascade for at least 1 dB of change).
- **About a dozen, named for the job, every shape used** (brief B), the brief's examples by name
  (`the_set_is_the_briefs_and_uses_every_shape`), and F1's two dynamic ones, *De-esser* and
  *Resonance tamer* (brief §C); `Designed::dynamic` gives a band its dynamics.

## Band solo is a transient command — [NOTES.md § Why](NOTES.md#band-solo-is-a-transient-command-never-a-parameter)

- **No id, state or preset value**: `plugins/AGENTS.md`'s transient-command exception, for one act.
- `audition.rs` is one atomic word, the band asked for and a count of the shell's endings; a request
  made before an ending cannot undo it.
- **The shell ends solo** in `reset` and `deactivate` (a restore and an activation both reset);
  **each block passes it to `set_solo`** only with an editor connected and never offline; it never
  waits, locks or allocates. The editor ends it on close.
- `solo_needs_an_editor_never_plays_offline_and_ends_with_the_shell` and `audition.rs`'s tests.

## The analyser's and the meter's handoffs — [NOTES.md § The handoffs](NOTES.md#the-handoffs-in-full)

- **Analyser** (`analyser.rs`): preallocated pre/post blocks through a bounded `rtrb` ring; every
  block spends a sequence number, a break starts a generation, a block carries its rate; nothing is
  copied with the editor closed or the analyser off; `process` works in pieces of one block
  (`a_hosts_block_sizes_change_nothing`). The consumer goes to the editor once (`MxmFxEqN::editor`).
- **Out meter** (`telemetry.rs`): mxm-kit's `PeakMeter` per channel, through the field; the shell
  publishes nothing with no editor connected, and opening or closing the editor clears it; a
  non-finite sample reaches it and latches the clip.

## Activity and rates

- **`KeepAlive` while any filter rings**, `Normal` once every state has settled to exact silence, so
  a host can sleep the plugin on silent input (`plugins/AGENTS.md`'s idle rule).
- **Activation below the DSP's `MIN_SAMPLE_RATE`**, or at a rate that is not finite, is refused.
- **Activation and `reset` seed the engine from the parameters at once** (`Engine::reset`), so a
  saved EQ is heard from its first sample rather than fading in from dry.

## The editor (plan §5) — [NOTES.md § The editor in full](NOTES.md#the-editor-in-full)

- **Layout**, the named exception to paged cards: the graph takes every spare point, the Out meter
  rail beside it, a fixed-width column of Band, Output and Analyser cards to its right, never under
  it. The window opens at `REFERENCE` and narrows to `MINIMUM`
  (`the_opening_size_is_inside_the_budget_and_holds_the_column_without_scrolling`,
  `the_app_bar_holds_in_the_minimum_window`). Each card body is a tree and passes the tree checks
  in every Band state; the Band card is reserved to its tallest shape.
- **The graph**: clicking the line adds a 0 dB bell in the lowest Off slot, every field written; a
  point is drawn at its own gain; a drag is one gesture, BACK puts it back; the wheel turns Q only
  over a point with focus or Alt; identity never rests on hue; every point has a name and value.
- **The keyboard is the collection's keyboard language**, through the cursor
  (`navigation::paged_with_bar`, every card a `bar_card`); **the graph is a surface**
  (`navigation::surface`, `on_surface`), where MOVE, VALUE, EXTENT, MICRO's edges, DUPLICATE, ADD,
  REMOVE and DELETE act on the bands, OUT keeps a gesture as one undo step and BACK reverts it.
- **Solo is a toggle** (the owner, 2026-10-14): the Solo button latches it on the selected band
  until it is pressed again, another band is selected, the band leaves the sound, the patch is
  replaced or the editor closes; Ctrl/Cmd held on a point solos it while held.
- **The gain scale fades** while no band in the sound has a gain (`gain_heard`), and its hover says
  so; it stays operable. With no band selected the Band card's hover says how to get one.
- **Undo is plain** (`history.rs`): the host context records every write; epochs at whatever the
  preset bar writes and at a host restore, **each ending every open gesture, a column control's
  too, where the host has it, writing nothing**; the history is the instance's.
- **The editor reads what it sent** (`normalised_now`, `band_now`): nice-plug changes a parameter
  only when its queued event is written, so the editor's own reads — the graph, the keyboard's
  edits and the column's own decisions (its menus, Solo) — take the history's pending values until
  the host applies them. The column's shared controls read the parameter itself, mxm-kit's binding
  as everywhere ([NOTES.md](NOTES.md#the-editor-in-full), *a known limit*; plan §7).
- **Dynamic bands** (plan §7.1): a bell or shelf's Band card has Range, Threshold, Attack and
  Release on the shared binding and a meter of how far it moves now; the graph rings a dynamic
  band's point, draws a line to where its Range would take it, and, while a band moves, the curve
  as heard over the drawn one, faint. Both read `Telemetry::gains`, published only with an editor
  connected. A new band writes static dynamics into its slot; a copy takes the original's.
- **The analyser** (`spectrum.rs`, mxm-kit's `mxm-fft`): band power per twelfth of an octave, a
  decimated long window for the lows; with no audio for `spectrum::STALE_SECONDS` the display
  falls to the floor and the next audio restarts both windows; settings are view state (`view.rs`),
  persisted, dirty-only;
  within `spectrum::BUDGET_MS` of a frame (`analyser_cost`).

## The host tests (`host-tests/`)

- **Product code is not linked into the host**: a band is set as a host restores a saved EQ,
  through the CLAP state, and read back by the host under its slot's name.
- They hold the bundle's boundary (plan §8): discovered as an effect with no map, the editor
  offered on the player's effect path, Init bit-exact dry in the chain, a restored band heard and
  ringing down to exact silence, a dynamic band cutting the loud note, a state from before F1
  restored static through the wrapper's own `filter_state`, and the player's bypass bit-exact dry.
- **They read `target/bundled/`, this repository's own**: with a shared `CARGO_TARGET_DIR` the
  bundle lands in that target's `bundled/` and must be copied here first.

## Not here, by plan

- **No control map** (plan §6): no collection role fits on-demand bands, and a map is optional.
- **DAW and listening checks** are E5's.

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
cargo xtask fetch
cargo test -p mxm-fx-eq-n-host-tests
```

The validator's random parameter fuzz once found a subnormal output, now flushed in the DSP. The
analyser's cost: `cargo test -p mxm-fx-eq-n --release --lib analyser_cost -- --ignored --nocapture`.
The pictures: `cargo test -p mxm-fx-eq-n --lib pictures`, into `target/layout-tree/mxm-fx-eq-n/`.
The factory set: `cargo test -p mxm-fx-eq-n --lib write_the_factory_presets -- --ignored`.

# Child DOX Index

No child AGENTS.md files.
