# AGENTS.md — plugins/mxm-fx-eq-n

Parent: [`../AGENTS.md`](../AGENTS.md). The detail behind the rules: [NOTES.md](NOTES.md).

# Purpose

`mxm-fx-eq-n`, an original parametric EQ with only the bands you need: up to 24, made on demand,
over [`crates/mxm-fx-eq-n-dsp`](../../crates/mxm-fx-eq-n-dsp/AGENTS.md). The product contract is
[`plans/plan-mxm-fx-eq-n.md`](../../plans/plan-mxm-fx-eq-n.md) and the signed brief
[`docs/briefs/mxm-fx-eq-n.md`](../../docs/briefs/mxm-fx-eq-n.md). **Built through E4**: the host
surface, the preset seam, solo's command path, the analyser and the meter (E2), the editor (E3),
and the factory set and the host tests (E4), and F1's dynamic bands through the shell, the editor
and two presets (F1.1–F1.4), their side chain (plan §7.2), F2's per-band stereo placement
(plan §7.3), F3's freeze and grab (plan §7.4), F4's resonance finder (plan §7.5) and F5's EQ
match (plan §7.6). DAW and listening checks are E5's, F1.5's, SC.4's, F2.4's, F3.4's, F4.3's and
F5.3's. No control map (plan §6): no collection role fits on-demand bands.

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
  analyser's editor half; `grab.rs` F3's hold, peaks and freeze; `finder.rs` F4's resonance
  finder, the rungs and the worker, its tests in `finder/tests.rs`; `matcher.rs` F5's EQ match,
  its tests in `matcher/tests.rs`; `tests.rs` the editor's tests.
- `src/tests.rs`: the shell's tests.
- `host-tests/`: the slow tier, the release bundle through MXM Player.

# Local Contracts

## Identity and host surface

- **`mxm-fx-eq-n`, CLAP id `dk.mxm.mxm-fx-eq-n`**, from `plugin_name!` and `bundler.toml`. The
  `-n` is descriptive (*n* bands), not a box number (root `AGENTS.md`, *Naming*).
- **Layouts** (brief A1; plan §7.2), four and their order permanent, a configuration's id being
  its index: stereo and mono each with a **"Side chain"** input of the main's channel count, then
  plain stereo and mono, which MXM Player selects. A Stereo band treats both channels alike; a
  placed band works on its own channel (plan §7.3). No note port, so no developer channel
  (`plugins/AGENTS.md`, *A developer channel*).
- **290 permanent parameters** (`params.rs`):
  - 24 slots, nested, each with `type`, `bypass`, `freq`, `gain`, `q` and `slope`; their ids are
    `<field>_<slot>`, `type_1` … `slope_24`, and their host names carry the slot number ("Band 7
    frequency"), never the shape;
  - `output`, the output gain, as linear gain shown in dB;
  - `gainscale`, 0–200 %;
  - F1's dynamics, four per slot after the globals (plan §7.1): `range_<n>`, `threshold_<n>`,
    `attack_<n>` and `release_<n>`, named "Band 7 range" and grouped with their slot; their place
    in the editor's indices starts at `params::DYNAMICS_BASE`, so no v1 index moved;
  - the side-chain switches after them (plan §7.2), `key_1` … `key_24`, "Band 7 side chain", from
    `params::KEY_BASE`;
  - the placements last (plan §7.3), `place_1` … `place_24`, "Band 7 placement", stepped 0–7
    (0 Stereo, 1 Left, 2 Right, 3 Mid, 4 Side, 5–7 unassigned, read *Unavailable n*, heard as
    Stereo), from `params::PLACE_BASE`.

  `the_ids_are_complete_unique_permanent_and_290` holds them.
- **A state or preset from before F1, §7.2 or §7.3 leaves every band static, on its own input, in
  Stereo**: a preset through `default_missing_legacy_parameter`, a host state through `filter_state`
  (`preset::add_missing_at_defaults`), which also extends a loaded preset's baseline so it stays
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
  beside it, then **two fixed-width columns** of cards to its right, never under it (decision 30):
  the Band card alone, then Output, Analyser, Resonances and Match, scrolling together; it opens at
  `REFERENCE`, hugged to the taller column, narrows to `MINIMUM`, and every card is a tree passing
  the tree checks in every Band state, the Band card reserved to its tallest shape (the
  opening-size tests).
- **The graph**: clicking the line adds a 0 dB bell in the lowest Off slot, every field written; a
  point is drawn at its own gain; a drag is one gesture, BACK puts it back; the wheel turns Q only
  over a point with focus or Alt; identity never rests on hue; every point has a name and value.
- **The keyboard is the collection's keyboard language**, through the cursor
  (`navigation::paged_with_bar`, every card a `bar_card`); **the graph is a surface**
  (`navigation::surface`, `on_surface`), where MOVE, VALUE, EXTENT, MICRO's edges, DUPLICATE, ADD,
  REMOVE and DELETE act on the bands, OUT keeps a gesture as one undo step and BACK reverts it.
- **Solo is a toggle** (the owner, 2026-10-14), latched on the selected band until pressed again,
  another selection, the band leaving the sound, a patch replaced or the editor closed; Ctrl/Cmd on
  a point solos while held. **The gain scale fades** while no band in the sound has a gain.
- **Undo is plain** (`history.rs`): the host context records every write; epochs at whatever the
  preset bar writes and at a host restore, **each ending every open gesture, a column control's
  too, where the host has it, writing nothing**; the history is the instance's.
- **The editor reads what it sent** (`normalised_now`, `band_now`, `dynamics_now`, `key_now`): its
  own reads take the history's pending values until the host applies them; the column's shared
  controls read the parameter itself ([NOTES.md](NOTES.md#the-editor-in-full), *a known limit*).
- **Dynamic bands** (plan §7.1, §7.2): Range, Threshold, Attack, Release, the Side chain toggle and
  a meter of the movement in a bell or shelf's card; the graph rings the point (dashed when keyed),
  draws where Range would take it and, while a band moves, the curve as heard over the drawn one,
  both from `Telemetry::gains`, published only with an editor connected.
- **Stereo placement** (plan §7.3): a **Placement** selector beside Bypass, a **Split** button (a
  Stereo band into Left/Right, Alt into Mid/Side) beside Delete band. With placed bands the graph
  draws each channel's response to itself (L, R; M, S dashed), lettered, as is a placed point. In
  mono (`Telemetry::mono`) a placed band is faint, refuses solo, and offers only Stereo.
- **Every slot field is written** by a new band (static, unkeyed, Stereo) and carried by a copy
  (`slot_writes`, twelve fields); Split is one undo step.
- **The analyser** (`spectrum.rs`, mxm-kit's `mxm-fft`): band power per twelfth of an octave, a
  decimated long window for the lows; with no audio for `spectrum::STALE_SECONDS` the display
  falls to the floor and the next audio restarts both windows; settings are view state (`view.rs`),
  persisted, dirty-only; within `spectrum::BUDGET_MS` of a frame (`analyser_cost`). The **Side
  chain** view (plan §7.4), offered only on a layout with the port (`Telemetry::side_chain_port`),
  draws the key's spectrum dashed and lettered "SC"; the audio thread copies the key only then.
  **Freeze** holds two seconds' highest levels and marks peaks a drag grabs (NOTES, *Freeze*).
- **Find** (plan §7.5, decisions 27–30; NOTES, *Find*): the Resonances card's toggle, offered only
  with the analyser on; Find and Freeze each end the other, the analyser off or the editor closed
  ends it, and it is never saved. It listens to the pre-EQ input (`finder.rs`) and proposes static
  cut bells, never a continuous curve: dashed curve and labelled downward triangles on the graph;
  a click, a drag (gain following, a pair together), ADD on the keyboard's proposal or **Apply
  all** writes them as ordinary bands, every slot field (`slot_writes`), one undo step each.
  **Amount** (whole percent, 20 % by default) is view state, saved, a change marking the project
  dirty and ending no freeze (`ViewSettings::display`). The search runs on `finder::Worker`, so
  the GUI thread pays only for listening and a copy of the evidence, inside
  `spectrum::BUDGET_MS` (`finder_cost`).
- **Match** (plan §7.6, decisions 31–34; NOTES, *Match*): the Match card's toggle, under
  Resonances, offered only with the analyser on and the side-chain port; Find, Match and Freeze
  each end the others; it is never saved. It listens to the side chain and the pre-EQ input
  (`matcher.rs`, the key copied while it listens) and proposes bells and shelves for the free
  slots, the EQ as heard counted; dashed curve and labelled hollow diamonds on the graph; **Apply**
  writes them all, every field, one undo step, only on the basis they were fitted on. **Amount**
  (40 %) and **Detail** (8 bands) are view state, saved. Its fits run on the same worker, one job
  at a time (`editor::Task`); listening stays inside `spectrum::BUDGET_MS` (`match_cost`).

## The host tests (`host-tests/`)

- **Product code is not linked into the host**: a band is set as a host restores a saved EQ,
  through the CLAP state, and read back by the host under its slot's name.
- They hold the bundle's boundary (plan §8): discovered as an effect with no map, the editor
  offered on the player's effect path, Init bit-exact dry in the chain, a restored band heard and
  ringing down to exact silence, a dynamic band cutting the loud note, a state from before F1
  restored static through the wrapper's own `filter_state`, the EQ on the plain stereo
  configuration, and the player's bypass bit-exact dry. They read this repository's own
  `target/bundled/` (with a shared `CARGO_TARGET_DIR`, copy the bundle there first).

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

The analyser's cost, the pictures and the factory set's generator: [NOTES.md § The slower
checks](NOTES.md#the-slower-checks).

# Child DOX Index

No child AGENTS.md files.
