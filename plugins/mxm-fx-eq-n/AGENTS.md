# AGENTS.md — plugins/mxm-fx-eq-n

Parent: [`../AGENTS.md`](../AGENTS.md). The detail behind the rules: [NOTES.md](NOTES.md).

# Purpose

`mxm-fx-eq-n`, an original parametric EQ with only the bands you need: up to 24, made on demand,
over [`crates/mxm-fx-eq-n-dsp`](../../crates/mxm-fx-eq-n-dsp/AGENTS.md). The product contract is
[`plans/plan-mxm-fx-eq-n.md`](../../plans/plan-mxm-fx-eq-n.md) and the signed brief
[`docs/briefs/mxm-fx-eq-n.md`](../../docs/briefs/mxm-fx-eq-n.md). **Built through E4 and F1–F8**
([NOTES.md § What is built](NOTES.md#what-is-built)); the DAW and listening checks are the owner's.
No control map (plan §6): no collection role fits on-demand bands.

# Ownership

- `src/lib.rs`: identity, layouts, the lifecycle and `process`.
- `src/params.rs`: the parameters, their domains and readings.
- `src/preset.rs`: the `mxm_preset::Instrument` seam and the factory set's design.
- `presets/`: the factory set, generated from that design, never hand-edited.
- `src/audition.rs`: band solo, a transient command.
- `src/preview.rs`: Match's Preview, the other transient command (plan §7.6).
- `src/learned.rs`: what Match learned of the track and what it is matched to, host state.
- `src/telemetry.rs`: the editor link and the Out meter, two of mxm-kit's `PeakMeter`s.
- `src/analyser.rs`: the analyser's audio half, and the share tap's (`Tap::capture_post`).
- `src/instances.rs`: the registry other tracks' editors read this instance through, its id and
  names (plan §7.8).
- `src/view.rs`: the analyser's settings, persisted view state. The External line's, Find's and
  Match's too.
- `src/editor/`: the editor. `mod.rs` its state, writes and keyboard commands; `panel.rs` the frame;
  `graph.rs`, `column.rs` (the cards), `history.rs` (undo), `spectrum.rs` (the analyser), then a
  module per feature, each named in [NOTES.md § What is built](NOTES.md#what-is-built); `tests.rs`.
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
- **Not permanent during pre-alpha** (the owner, 2026-10-16: "relax it for pre-alpha"): until the
  first release a parameter's id, its order, its stepped domain and the places of its values may
  change when the design calls for it, the tests that pin them and the factory set changing with
  them, and a state or preset from before the change need not load as it was. From the first
  release they are permanent, as the collection's convention has it (`plugins/AGENTS.md`).
- **293 parameters** (`params.rs`; in full: [NOTES.md § The parameters](NOTES.md#the-parameters-in-full)),
  each block after the last, so no earlier index moved: 24 nested slots of `type`, `bypass`,
  `freq`, `gain`, `q` and `slope` (`type_1` … `slope_24`, the host's names carrying the slot
  number, "Band 7 frequency", never the shape); `output` and `gainscale`; per slot F1's `range`,
  `threshold`, `attack` and `release`, the side-chain `key` and the `place`; F6's `phase` and
  `resolution`, not automatable; F7's `autogain`. `the_ids_are_complete_unique_and_293` holds them.
- **A state or preset from before F1, §7.2 or §7.3 leaves every band static, on its own input, in
  Stereo** (one from before F6 or F7: Zero latency, Medium, auto gain off): a preset through
  `default_missing_legacy_parameter`, a host state through `filter_state`
  (`preset::add_missing_at_defaults`), which also extends a loaded preset's baseline so it stays
  clean.
- **A persisted field whose restore may install other than what was saved says so**, with
  `nice_plug::wrapper::accept_canonicalized_persistent_field` and its key (`params::VIEW_KEY`,
  `IDENTITY_KEY`, `LEARNED_KEY`); otherwise the wrapper rolls back the whole restore, bands and
  all ([NOTES.md § Restores](NOTES.md#a-restore-that-reads-back-changed)).
  `a_restore_that_reads_back_changed_keeps_its_bands` holds it.
- **Automation belongs to the slot number** (decision 10). A new band takes the lowest Off slot,
  and a preset's band 3 is slot 3. No ownership state exists.

## The stepped domains are reserved

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
  **each block passes it to `set_solo`** only with an editor connected, whatever mode the host
  says it runs in (Resolve says offline as it plays); it never waits, locks or allocates. The
  editor ends it on close.
- `solo_needs_an_editor_whatever_the_mode_and_ends_with_the_shell` and `audition.rs`'s tests.

## Match's Preview is a transient command too — [NOTES.md § Why](NOTES.md#matchs-preview-is-a-transient-command-too)

- **No id, state or preset value**, as solo (plan §7.6, decision 50); `preview.rs`, the bands by
  the slot Apply writes, one writer, latest wins. **The shell plays it** each block only with an
  editor connected, whatever the host's mode, only in an Off slot; **the editor ends it** on close.
- `preview_plays_in_an_off_slot_with_an_editor_whatever_the_mode` and
  `preview_hands_the_proposals_to_the_free_slots`.

## The analyser's and the meter's handoffs — [NOTES.md § The handoffs](NOTES.md#the-handoffs-in-full)

- **Analyser** (`analyser.rs`): preallocated pre/post blocks through a bounded `rtrb` ring; every
  block spends a sequence number, a break starts a generation, a block carries its rate; nothing is
  copied with the editor closed or the analyser off; `process` works in pieces of one block
  (`a_hosts_block_sizes_change_nothing`). The consumer goes to the editor once (`MxmFxEqN::editor`).
  Other tracks' tap (`instances.rs`, plan §7.8) copies only while another track's editor watches;
  the instance id is host state, never a preset's.
- **Out meter** (`telemetry.rs`): mxm-kit's `PeakMeter` per channel, through the field; the shell
  publishes nothing with no editor connected, and opening or closing the editor clears it; a
  non-finite sample reaches it and latches the clip.

## Activity and rates

- **`KeepAlive` while any filter rings**, `Normal` once every state has settled to exact silence, so
  a host can sleep the plugin on silent input (`plugins/AGENTS.md`'s idle rule).
- **Activation below the DSP's `MIN_SAMPLE_RATE`**, or at a rate that is not finite, is refused.
- **Activation and `reset` seed the engine from the parameters at once** (`Engine::reset`), so a
  saved EQ is heard from its first sample rather than fading in from dry.
- **One latency per activation** (plan §7.7; the kit's *Latency is fixed*): reported at
  activation; a change while active is heard after the host's restart ([NOTES.md § One
  latency](NOTES.md#one-latency-per-activation)).

## The editor (plan §5) — [NOTES.md § The editor in full](NOTES.md#the-editor-in-full)

- **Layout**, the named exception to paged cards: the graph takes every spare point, the Out meter
  beside it, then **two fixed-width columns** of cards to its right, never under it (decision 30):
  the Band card and Match, then Output, Analyser and Resonances, scrolling together; it opens at
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
- **Every slot field is written** by a new band (static, unkeyed, Stereo) and carried by a copy
  (`slot_writes`, twelve fields); Split is one undo step.
- **The features**, a bullet each in [NOTES.md § The editor's features in short](NOTES.md#the-editors-features-in-short).
  What binds them:
  - **Every command is one undo step**: a proposal written (Find, Match's Apply), Split, a Phase
    or Resolution change, an edit of a selection's primary, which moves the group (`group_edit`).
  - **Nothing they hold is saved but view state** (`view.rs`, persisted, dirty-only): the
    analyser's settings, the External line, Collisions, Find's Amount, Match's Amount and Detail;
    and Match's, what this EQ learned and what it is matched to (`EqParams::learned`, `match_to`,
    host state beside it, every other EQ reading the first through the registry). A saved
    reference is a file, never a preset (`references.rs`). A change to them marks the project
    dirty through `GuiContext::set_state`, nice-plug's dirty-only transaction, which neither
    wrapper may answer by waiting on the audio thread ([NOTES.md § Marking the project dirty](NOTES.md#marking-the-project-dirty)).
  - **The GUI thread's budget**: the analyser and Find's and Match's listening inside
    `spectrum::BUDGET_MS` of a frame (`analyser_cost`, `finder_cost`, `match_cost`), their
    searches and fits on `finder::Worker`.
  - **The drawn curve is the one heard**: a moving band's, a placed band's per channel, the phase
    in force's (`Telemetry::gains`, published only with an editor connected; `Telemetry::phase`).

# Work Guidance

- During pre-alpha an id, the order, a stepped domain or a value's place may change (*Not
  permanent during pre-alpha*); change the pinning tests and regenerate the factory set with it.
  From the first release: rename a field freely, never an id.
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

The host tests: **Product code is not linked into the host**: a band is set as a host restores a
saved EQ, through the CLAP state, and read back by the host under its slot's name. They read this
repository's own `target/bundled/` (with a shared `CARGO_TARGET_DIR`, copy the bundle there
first); what each holds: [NOTES.md § The host tests](NOTES.md#the-host-tests-host-tests). The
analyser's cost, the pictures and the factory set's generator: [NOTES.md § The slower
checks](NOTES.md#the-slower-checks).

# Child DOX Index

No child AGENTS.md files.
