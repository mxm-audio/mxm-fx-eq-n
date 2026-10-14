# plan-mxm-fx-eq-n — a parametric EQ with only the bands you need

**Revision 24 — the external side chain, designed for the owner's approval** (2026-10-14; §7.2,
decisions 15–17; its design approved by the agent-loop in round 2, 2026-10-14: 2 findings, one
accepted, one premise rejected with the permanence it asked for stated). **Approved by the owner**
(2026-10-14, *"Approve and build"*, decisions 16 and 17 as proposed); SC.0 done; SC.1–SC.3 built (2026-10-14), their code review approved in round 2 (one finding,
accepted: only a dynamic shape's solo listens to the key). SC.4, listening in Bitwig at home, is next. **Revision 23 — F1, dynamic bands, designed for the owner's approval** (2026-10-14; §7.1, decisions
12–14; its design approved by the agent-loop in round 8, 2026-10-14: 17 findings over seven rounds,
all accepted). **Approved by the owner** (2026-10-14, *"Approve, values as proposed"*); F1.0 done, the brief's §C; F1.1, the DSP, built; F1.2–F1.4, the shell, the editor and the two presets, built; their code review approved in round 3 (2026-10-14: 6 findings over two rounds, all accepted). F1.5, the owner's listening and DAW checks, is next. **Revision 21 — E4 built** (2026-10-14): the factory set and the host tests, below. **Revision 20 — moved to its own repository** (2026-10-13). E1–E3 were built in the archived
monorepo after the split, by mistake, and are being ported here onto the live mxm-kit: the DSP
crate unchanged, the shell onto nice-plug 0.4.2, the editor onto the kit's keyboard language
(`mxm_ui::navigation`, `mxm-keys`) in place of the archive's `held` layer, and the analyser on the
kit's new `mxm-fft`. Paths below that name `crates/ui`, `crates/mxm-preset` and the like are the
kit's crates; the archive's revision history is kept as it was.

The plan's own agent-loop review was stopped by the owner at revision 16, unapproved (2026-10-12, *"yes stop the review now"*). Rounds 1–12 reviewed revisions up to 15, and round 13 was cancelled mid-round.
Round 13's six filed findings are answered here: the solo path, Space ownership, the end-to-end
budget, the column's vertical scroll, and two about undo provenance that plain undo makes moot. So
are the owner's last rulings: plain undo, and the plan cut back to requirements. **Those changes are
unreviewed.** Written from the
owner's rulings of 2026-10-12 and the evidence base `research:effects/parametric-equalizers.md`.
**Built so far:** E0, the brief, signed; E0.5, the hand-judged interaction probe, code-reviewed and
approved; **E1, the DSP crate** (2026-10-13), measured, and its code approved by the agent-loop in round 6; **E2, the plugin shell** (2026-10-13), bundled, `clap-validator` clean, and its code approved by the agent-loop in round 1; **E3, the editor** (2026-10-13), ported here and its code approved by the agent-loop in round 6; **E4** (2026-10-14), the factory set and the host tests, its code approved by the agent-loop in round 2. E5 is next.

| Rev | Change |
|---|---|
| 1 | First draft. The owner's rulings of 2026-10-12: bands made on demand by clicking the curve, FabFilter Pro-Q as the functional benchmark but kept simple, the selected band's details beside the graph rather than under it, an animated spectrum, newDAWn's held-key keyboard model, and *"as flexible and usefull as possible"* delivered in phases. Architecture proposed: a fixed pool of permanent band slots as host parameters, a series cascade of trapezoidal-SVF bands, a GUI-side analysis layer that writes ordinary bands, and a side column after `mxm-fx-curve`'s canvas exception. |
| 2 | **Round 1 (7).** Neutral line-click insert; no stacked layout; undo as per-parameter writes; drawn curve = shipped response; F1 band path gated on E1; fixed stepped domains. Owner: 24 bands, right column, K1, shared `mxm-fft`. |
| 3 | **Round 2 (6).** Physical keys via egui-baseview; undo required and action-based; structural changes wait as one pending target; fixed domains for every stepped parameter. |
| 4 | **Round 3 (6).** Analyser blocks through a bounded queue, decimated long windows under a budget, settings as view state; only Off slots skipped. |
| 5 | **Round 4 (5).** Completed-gesture observer in `mxm-preset`; keyboard creation from an empty Init; first slot-ownership mechanism (later removed, rev 8). |
| 6 | **Round 5 (5).** Undo provenance from host events, not values; one-card minimum with horizontal scroll; view settings mark dirty. |
| 7 | **Round 6 (5).** nice-plug host-event counter for undo; slope is structural; ownership mechanism collapsed (later removed, rev 8). |
| 8 | **Round 7 (5, all slot ownership), answered by owner decision 10, option A:** automation belongs to the slot number; ownership machinery removed. |
| 9 | **Round 8 (4).** Wheel edits Q only with focus or a modifier; the observer also reports every whole-patch replacement (presets, Init), so undo cannot cross one; a per-shape semantics table is an E0 deliverable; a cramping hybrid must stay continuous across its crossover. |
| 10 | **Round 9 (4 standing).** Decision rows 2–8, overwritten by a table edit in revision 9, restored with alternatives; host state restore starts the undo epoch through the persisted view-state field; undo history belongs to the plugin instance and survives the editor closing; identity bands park and re-enter from a cleared state through the creation fade, which meets the idle rule without a frozen-state click. |
| 11 | **Round 10 (3), unreviewed.** Telemetry keeps every channel and v1 shows their power average; undo is conservative on a host that echoes GUI writes, with no value- or revision-based fallback claimed; band solo enters v1 as an audition route, and a workflow phase F8 (multi-selection, copy and paste, global transforms) is added, with v1's selection and commands built as sets. The loop stopped at its round cap. |
| 12–13 | **Owner decisions** (2026-10-12): 2, a neutral Init; 5, the held-key layer in `crates/ui`; 8, a gain scale and no Mix; 9, phases in order. |
| 14 | **Round 11 (5 standing, 7 filed).** The gain scale's 0 % leaves gainless shapes as drawn; solo's meaning per shape joins the E0 table, and its lifetime is the editor's, never offline or saved; identity bands park only when their ring time is under the re-entry fade; text entry is a separate mode that makes the command layer inert; E0 declares a worst-case DSP budget; echoing hosts are handled by matching echoes to pending writes, with the identical-value coincidence as owner decision 11. |
| 15 | **Round 12 (3).** Solo crosses the transient-command path, so the DSP reads no editor state; every identity band parks, and re-entry is the same event as creation; decision 11 re-decided by the owner as option A (matches are echoes, inside an expiry window). §5.2–§5.4 consolidated. |
| 24 | **The external side chain designed** (2026-10-14), §7.2, for the owner's approval: next, before F2, the EQ only and heard at home (decision 15); per band, the key band-limited as the input is (decision 16); solo plays what the band listens to (decision 17); one port, keyed layouts first; 24 switches appended |
| 23 | **F1 designed** (2026-10-14), §7.1, for the owner's approval: Pro-Q's range-only controls, each band's own region as its detector, no external side chain yet (decisions 12–14); four parameters appended per slot; Wise's form (audio in series, detectors in parallel, a fixed detector filter); a dynamic band never parks; delivery F1.0–F1.5. |
| 22 | **E5, first findings** (the owner in MXM Player, 2026-10-14): solo is a toggle, not held; the gain scale fades while no band in the sound has a gain; the empty Band card's hover says how to get a band. A freeze found on the way is the baseview fork's (its frame clock starved input on a 59 Hz screen), fixed there. Then: Solo is the kit's toggle; a silence empties the spectrum, the long window's lows included. Code review approved in three rounds, with the baseview fix (2026-10-14). |
| 21 | **E4 built** (2026-10-14): thirteen factory presets generated from a design table in the editor's units, the brief's eight examples plus five so every shape is used, each in the lowest slots at Init's output; `host-tests/` through MXM Player: discovered as an effect, the editor offered, Init bit-exact dry in the chain, a band restored through the CLAP state heard and ringing down to exact silence, the player's bypass bit-exact; the lock on kit `f459221` and player `cd3eae3`. |
| 20 | **Moved to its own repository**, `mxm-audio/mxm-fx-eq-n` (2026-10-13): the archive was the wrong home. The DSP crate came across unchanged (50 tests, on 1.87 too); the FFT became mxm-kit's `mxm-fft`; the shell and the editor are ported onto the live kit, the editor's keys onto its keyboard language. |
| 19 | **E3 built** (2026-10-13): the graph, the column of tree cards, the Out meter, the held-key keyboard (`crates/ui`'s `held`), plain undo with epochs, held solo, the analyser on `crates/mxm-fft` (X0, X1) within a 2 ms budget, accessibility, the editor checks; the probe deleted. Undo records at the editor's own host context, so §5.4's binding observer and §6's `mxm-preset` change were not needed. |
| 18 | **E2 built** (2026-10-13): `plugins/mxm-fx-eq-n`, 146 permanent parameters with reserved stepped domains, the empty-EQ Init, the preset seam, solo as a transient command, the analyser ring and the Out meter, no editor yet. `clap-validator`'s parameter fuzz found a subnormal output, now flushed in the DSP. |
| 17 | **E1 built** (2026-10-13), after the owner moved past the open E0.5 items (*"no continue the plan"*). The cramping method is chosen: **matched designs** (Vicanek's impulse-invariant poles, numerators fitted to the analogue magnitude) realised in the SVF itself, so the modulation and precision case for the SVF stands. Precision: `f64` state. The measurements are in `crates/mxm-fx-eq-n-dsp/AGENTS.md`. |
| 16 | **Owner: "simplify undo", then "stop the review now".** Undo is plain: it restores the action's parameters, with no provenance tracking, so the nice-plug patch, echo matching and decision 11 are gone. Solo and the analyser handoff are stated as requirements, leaving mechanism to implementation. Round 13 (cancelled) answered: solo-off is always delivered and the shell ends solo itself; Space belongs to the field in text entry and to the host otherwise; one end-to-end audio budget includes the analyser handoff; the column scrolls vertically. Unreviewed. |

---

## Governing decisions

Everything an implementer must not settle alone, in one place; the sections below hold the reasoning.

| # | Decision | Status | Gates |
|---|---|---|---|
| — | Bands are made on demand: clicking the curve adds one; *"Only the ones you need, but all of the ones you need"* | Owner ruling, 2026-10-12 | — |
| — | FabFilter Pro-Q is the gold standard for function and interaction, *"But I want it simple"* | Owner ruling, 2026-10-12 | — |
| — | The selected band's parameters sit **to the left or right of the graph, never under it** | Owner ruling, 2026-10-12 | — |
| — | An **animated spectrum** of the sound is part of v1 | Owner ruling, 2026-10-12 | — |
| — | The keyboard follows **newDAWn's held-key model**: arrows alone navigate, a held key plus arrows edits | Owner ruling, 2026-10-12 | — |
| — | Scope is *"as flexible and usefull as possible"*: v1 is the simple core, the rest are later phases of this plan | Owner ruling, 2026-10-12 | — |
| 1 | Band cap: **24 slots**, Pro-Q's | **Settled** (owner, 2026-10-12) | — |
| 2 | Init is the neutral, empty EQ, an exception to "an effect opens engaged" (`plugins/AGENTS.md`), as `mxm-fx-curve` got; the alternative is opening with a default band | **Settled** (owner, 2026-10-12): neutral | — |
| 3 | The side column is on the **right** | **Settled** (owner, 2026-10-12) | — |
| 4 | The starting keymap is **K1**, newDAWn's meanings (§5.3) | **Settled** (owner, 2026-10-12) | — |
| 5 | The held-key command layer lives in **`crates/ui`**, the shared interface crate that already holds the keyboard standard (`src/navigation.rs`), as an opt-in module the later migration reuses (§5.3) | **Settled** (owner, 2026-10-12: *"it should follow the standard"*) | — |
| 6 | The analyser's FFT comes from a **shared `crates/mxm-fft`** built on `realfft`, and the collection's four in-house FFTs move to it, under its own plan (`plans/plan-mxm-fft.md`) | **Settled** (owner, 2026-10-12: *"make a shared fft crate. Havine 3 or for implementations is non DRY"*) | E3 |
| 7 | A plugin-owned undo/redo for band edits | **Required, not open.** The design system puts Undo and Redo in every app bar (§3.1) and makes a destructive action recoverable (§7.2); deleting a band is destructive | — |
| 8 | **No dry/wet Mix; a gain scale** that scales every band's gain together (§1.1) | **Settled** (owner, 2026-10-12) | — |
| 9 | The later phases run in order, F1 to F8 | **Settled** (owner, 2026-10-12) | — |
| 10 | **Automation belongs to the slot number** (option A): lowest free slot, presets in their saved slots, no ownership protection | **Settled** (owner, 2026-10-12) | — |
| 11 | ~~Undo on echoing hosts~~ **Moot**: undo is plain (owner, 2026-10-12, *"simplify undo"*), with no provenance tracking (§5.4) | **Settled** | — |
| 12 | F1's controls are **Pro-Q's, range only**: Threshold, Range, Attack, Release, a built-in soft knee, no ratio; the alternative was threshold, ratio and range (§7.1) | **Settled** (owner, 2026-10-14) | F1.0 |
| 13 | **A dynamic band listens to its own region**; the alternative was a free trigger range per band (§7.1) | **Settled** (owner, 2026-10-14) | F1.1 |
| 14 | **No external side chain in F1**: its own later step, once newDAWn routes a side chain (§7.1) | **Settled** (owner, 2026-10-14) | — |
| 15 | **The external side chain next**, before F2: the EQ only, proved by tests here and heard in Bitwig at home; the alternatives were a key source in MXM Player, or newDAWn's routing first (§7.2) | **Settled** (owner, 2026-10-14) | — |
| 16 | **Per band**: each dynamic band listens to its own input or the side chain, the key band-limited as the input is (Pro-Q 4's way); the alternative is one switch for every band (§7.2) | **Settled** (owner, 2026-10-14) | SC.1 |
| 17 | **Solo plays what the band listens to**, so a keyed band solos the band-limited key (Pro-Q's audition); the alternative keeps solo on the main input for every band (§7.2) | **Settled** (owner, 2026-10-14) | SC.1 |

Implementation choices with stated criteria — not owner decisions — are the cramping method (§3.3),
the state precision (§3.4) and the shape and slope set (§3.2).

## 1. What this is

A standalone CLAP effect: a parametric EQ whose graph starts as a flat line. Clicking the line adds a
band; dragging it sets frequency and gain; its details sit in a column to the right; an animated
spectrum runs behind the curve. There is no fixed row of eight or twenty bands.

**The benchmark is Pro-Q, functionally.** It is the most popular EQ plugin by every public measure, and
its feature set maps what "flexible and useful" means (`research:effects/parametric-equalizers.md`
§19–§20). It is a list of behaviours to match or deliberately leave out, never a visual target
(`plans/AGENTS.md` *Images*; `docs/MXM_DESIGN_SYSTEM.md` is normative).

**Product identity.** `mxm-fx-eq-n`, CLAP id `dk.mxm.mxm-fx-eq-n`, named by the owner under the
`mxm-fx-<effect>` rule (root `AGENTS.md` *Naming*). The `-n` is descriptive — *n* bands — not a box
number, so the `-<NN>` box-keying clause does not apply. `plugin_name!` is the one literal;
`bundler.toml` the one external copy.

### 1.1 What it is not

- **Not a copy of any product.** No hardware or software is reproduced; Pro-Q is a benchmark.
- **Not a graphic EQ** with fixed bands, and **not a multiband compressor** (the research's §22 draws
  both lines).
- **Not a resonance suppressor in v1.** Resonance finding arrives in F4 as a helper that writes
  ordinary, editable bands; continuous spectral suppression is a separate later decision (§7).
- **Not linear phase in v1.** Zero latency, minimum phase only; F6 adds phase modes.
- **No dry/wet Mix; a gain scale instead** (decision 8, owner, 2026-10-12). Mixing a minimum-phase EQ
  with its dry signal comb-filters wherever a band turns the phase (research §5.5), so what is
  heard leaves the drawn curve. The gain scale gives the "less of this EQ" a Mix is wanted for,
  without that. It is one automatable, smoothed control that scales every bell, shelf and tilt
  gain together, and nothing else. 100 % is as drawn; above 100 % exaggerates. **At 0 % every bell,
  shelf and tilt is flat, while cuts, notches and band passes stay as drawn**, because they have no
  gain. That is the meaning the owner approved, and Pro-Q's control behaves the same way. It never
  moves frequency or Q, and the graph draws the scaled curve. Pro-Q has the same control and no Mix.
- **No cross-instance features in v1** — collision display, an instance list.

## 2. The band model

### 2.1 Bands are a fixed pool of permanent slots

The host sees a fixed array of band slots, every slot's controls permanent automatable parameters.
The editor shows only the slots in use: the owner's *"only the ones you need"*, while a host still
sees a stable list. **No slot is ever renumbered**, so a band keeps its number and its automation for
as long as it exists (Pro-Q's behaviour; research §17.2).

**Automation belongs to the slot number** (decision 10, owner, 2026-10-12: option A, "simple"). This
is how a plain fixed-slot EQ behaves:

- **A new band takes the lowest slot whose type is Off.** Deleting a band sets its type Off, and the
  slot is free at once.
- **Presets and Init write their slots as saved.** A preset's band 3 is slot 3. Init sets every type
  Off. `crates/mxm-preset` needs no placement hook.
- **The trade-off is accepted, and the editor does not hide it.** The host keeps a deleted band's
  lanes, so a new band in that slot, or a preset band, is driven by them. The same goes for a slot a
  host lane has automated Off. Research §25.8 asks the opposite, and that item is set aside by
  owner ruling.
- **No other state exists.** There is no ownership record, no "ever owned" history and no placement
  protocol, so undo, presets, Init and restore are ordinary parameter writes.

This is within `plugins/AGENTS.md` *Editable models*, whose test is *who can add to the set*. The
release ships the pool, and the user turns slots on and off but cannot extend it, so its controls are
parameters. Precedents:
- `mxm-model-drums`' nested slot arrays with `foo_{n}` ids, and their id-completeness tests
  (`plugins/mxm-model-drums/src/params.rs`, drum-machine's `nested_slot_ids_are_complete_unique_and_stable`);
- nice-plug patch 1, which sizes the GUI event queue from the parameter count
  (`vendor/nice-plug/PATCHES.md`).

A slot's **type** parameter carries its use: one stepped parameter whose first value is *Off* and the
rest the shapes. Append-only is not enough on its own. Projects and presets store the normalized
value (`crates/mxm-preset/AGENTS.md`), so growing the range would remap every saved shape. The type
therefore **reserves a fixed domain from its first release**: unassigned values read as unavailable,
and a new shape takes the next reserved value. That is `mxm-model-drums`' `model` selector
(`plugins/mxm-model-drums/src/params.rs`). **The rule covers every stepped per-band parameter**, not
just the type. The slope will gain Brickwall in F6, and the placement arrives in F2. A test pins
every shipped stepped value's normalized representation. A separate bypass toggle lets a band be
heard on and off without losing its shape.

Host parameter names are fixed at load — the wrapper rescans values, never names — so a slot is
named by its number ("Band 7 frequency"), never by its shape.

**Rejected: bands as persisted state**, as `mxm-fx-curve` keeps its points. It removes the parameter
clutter but loses automation of frequency, gain and Q, which every surveyed product offers.
**Rejected: CLAP parameter rescan** to add parameters at runtime. Hosts handle a changing parameter
list poorly, and an id must be permanent from the release that declares it.

**The cap is 24 slots**, Pro-Q's (decision 1, owner, 2026-10-12; research §17.1).

### 2.2 Signal path

```
in ──► band 1 ─► band 2 ─► … ─► band N ─► output gain ──► out
        (each band's gain × the gain scale)
         (series cascade of the slots in use, in slot order; dB responses add)
```

- **Series, in dB.** Bands compose by adding their dB responses, so the drawn curve is the sum of the
  drawn bands (research §3.6). Slot order is irrelevant to the linear result.
- **Layouts**: mono→mono and stereo→stereo, the two channels processed identically in v1. Per-band
  stereo placement (L, R, M, S) is F2.
- **An empty EQ is bit-exact dry**: no slot in use and output gain at 0 dB means output equals input,
  whatever the gain scale.
  Decision 2 makes that the Init.
- **Q follows the half-gain-in-dB convention**, the cookbook's: under it a cut exactly cancels the
  equal boost (research §3.2–§3.3). Products define Q differently, so a value is never borrowed from
  another product without converting.

## 3. The DSP — `crates/mxm-fx-eq-n-dsp`

Framework-free, MSRV 1.87, zero runtime dependencies, as every effect DSP crate.

### 3.1 One band is a trapezoidal SVF

Each band is Simper's trapezoidal (TPT) state-variable filter, with the output mix choosing the shape.
The research measured it against the direct forms (§4.4–§4.5 there):
- **It is proven stable** under per-sample changes of cutoff and damping (Wishnick 2014; McClellan
  2026). On a square-wave frequency sweep it peaked at 1.3× the input, against 2.2× for DF1 and 5.2×
  for TDF2.
- **It keeps a 20 Hz, Q 10 bell at 192 kHz within 0.002 dB in `f32`.** Direct-form coefficients in
  `f32` gave +13.6 dB instead of +12.
- **Its bell and shelf table is the collection's own**, `docs/filters/02-topologies.md` §2.2. The
  research checked it against Simper's paper and found it correct, which answers that file's "verify
  before shipping shelves".

Its weak spot is a sudden gain or Q jump, hence smoothing (§3.4). In this bell the damping follows
the gain, so a dynamic band (F1) moves its poles. Stability holds, but square-wave gain modulation
left broadband energy at −15.1 dB, against −23.6 dB for DF1. **Measured at E1:** under
envelope-rate gain modulation the shipped bell puts −89.7 dB above 5 kHz, and Wise's fixed-core
form, where only a gain moves, −92.6 dB. So F1 inherits the band.

**The drawn curve is the shipped response.** The editor evaluates each band from the DSP crate's own
design at the current sample rate: the coefficients audio runs, cramping and slopes included, never
an analogue formula. So the graph cannot show a curve the audio does not have.

### 3.2 Shapes and slopes

v1 offers Pro-Q's core set: **bell, low shelf, high shelf, low cut, high cut, notch, band pass, tilt
shelf**. Cuts and shelves have a slope; cuts are cascades of second-order sections, with Pro-Q's
6–96 dB/octave span as the reference (research §3.5). The final set, the slope ladder and the ranges
are settled at E0 with the owner and written in the brief, not here. Brickwall cuts need linear phase
and belong to F6.

**Each shape's meanings are part of the public contract.** Saved projects depend on them, so the brief
fixes them before E2 releases the parameters. It gives a table per shape:
- what frequency means (a shelf alone has three competing definitions, research §3.4);
- what Q means (width, resonance or slope);
- what gain means (gain or depth);
- what slope means;
- what solo plays (§5.2);
- what a shape change keeps.

The bell's half-gain Q (§2.2) is the first row. Inverse curves and matching (F4, F5) rely on it.

### 3.3 Cramping near Nyquist — measured, then chosen

A bilinear bell at +12 dB, Q 1 is 3.3–7.5 dB off the analogue curve at 10–18 kHz at 44.1 and 48 kHz
(research §4.2). The SVF is a bilinear design, so it cramps the same way. The research measured the
candidates (§4.3 there):

| Approach | Error near the top | Cost |
|---|---|---|
| Matched second-order designs (Vicanek; Orfanidis) | 0.4–1.75 dB | Direct-form coefficients: they bring back the modulation and `f32` problems §3.1 avoids |
| SVF at 2× oversampling | about 1 dB | Twice the band cost, plus the resampling filters |
| SVF at 4× oversampling | about 0.25 dB | Four times |
| SVF plus the cookbook's bandwidth correction | right to about 10 kHz, over-wide skirt above | Free |

E1 chooses against stated criteria, recorded in the DSP crate's `AGENTS.md`: the magnitude error to
20 kHz at 44.1 and 48 kHz, stability under the modulation test, precision at the bottom of the range,
cost at the cap, zero added latency, and **continuity under frequency automation across any internal
regime boundary**. A hybrid, such as matched `f64` direct-form bands above a crossover frequency, is
allowed only if a sweep across its crossover stays bounded and click-free, for example by
crossfading the two regimes with hysteresis. Otherwise it is rejected. The fast-automation test in
§8 sweeps across it.

**Chosen at E1: matched designs in the SVF.** Every section keeps the SVF's structure, with its
poles placed by impulse invariance and its numerator fitted to the analogue magnitude. The matched
bell is 0.74–0.84 dB off near the top at 44.1 kHz where the bilinear one is 3.8–7.5 dB, at no
per-sample cost, with zero latency and no regime boundary: every matched pole is at or below its
band's frequency. Oversampling was not built, because it fails the zero-latency criterion and the
steady budget. The figures, and the two shapes the fit cannot place exactly, are in
`crates/mxm-fx-eq-n-dsp/AGENTS.md`.

### 3.4 Smoothing, transitions and precision

- **Smooth the user's values, recompute the coefficients.** Log-frequency, dB gain and log-Q are smoothed
  over the collection's 5–20 ms window (`plugins/AGENTS.md`), and the coefficients are recomputed
  from them. Interpolating coefficients bends the band's shape (research §4.4). A coefficient
  set costs about 6.6 ns (§4.6 there), so recomputing per sample while a value moves is affordable.
- **Creating, deleting, bypassing and changing a band's type are click-free.** A bell or shelf ramps
  its gain to or from 0 dB, where it is the identity. A cut or a type change crossfades the band's
  old and new outputs. A short linear transition like `mxm-fx-curve`'s 20 ms is the precedent; the
  length is measured.
- **A structural change during a transition waits; the newest one wins.** Each band holds at most
  two filter states, the current one and the one it is fading to. **Structural** means any change
  that alters a band's topology or section count: type, bypass, slope (and placement, in F2),
  create and delete. One arriving mid-fade becomes the band's single pending target, and a later one
  replaces it.
  It starts when the fade ends. So the worst case is two states per band, and a change is delayed
  by at most one transition, after `mxm-fx-curve`'s latest-state publication. Frequency, gain and Q
  are not structural: they smooth continuously and never fade.
- **Precision.** Coefficients are designed in `f64`, and **the state is `f64`** (chosen at E1).
  `f32` state was within 0.003 dB on the research's low-frequency cases, and `f64` costs nothing
  more in a serial cascade, so the repository's default stands.
- **Denormals are flushed on every state.** Non-finite input becomes zero.

### 3.5 Cost and idle

The SVF costs about 3.5–4 ns per band per sample (research §4.6), so 24 single-section stereo bands at
48 kHz come to about 1 % of one core (derived). **That is not the worst case.** The worst v1
configuration is:
- 24 bands, each a cut at the steepest admitted slope (several sections);
- every band mid-transition (two states, §3.4);
- stereo, at 192 kHz;
- with the chosen cramping method, whose oversampling multiplies the rest;
- **with an editor open**, so the analyser's audio-side block handoff (§4) runs as well.

**E0's brief declares one end-to-end audio-thread budget for that configuration, before E1
measures.** E1's cramping and slope choices are accepted only if that whole case fits; later audio-side
analyser work spends from the same budget. The analyser's FFT runs on the editor thread, so it has its
own frame budget (§4).

**Doing nothing costs nothing** (`plugins/AGENTS.md`'s idle rule). Three kinds of band run none of
their processing:
- an Off slot;
- a bypassed band, once its fade has finished;
- a band whose shape is exactly the identity: a bell, shelf or tilt settled at 0 dB.

**Leaving either state never resumes a frozen one**, because a stale state would click as the gain
moves. The band starts from a cleared state and re-enters through the creation fade (§3.4).

**Every identity band parks, and re-entry is exactly creation.** A band's ring time is about
2.2·Q/f₀ (research §8.3), seconds for a sharp band at 20 Hz. So a band starting from a cleared state,
whether newly created or leaving identity, reaches its drawn response only over that time. The plan
accepts that for creation, and re-entry is the same event:
- **Click-free.** It comes in through the creation fade.
- **Settled within the band's ring time.** It is not instantly at the response a never-parked band
  would have had.

This keeps the idle rule whole, for every identity configuration. Keeping long-ringing bands running
instead would be an exception to the owner's rule, and only the owner could grant it. Cuts, notches
and band passes are never the identity, so they always run.

**Tail.** A high-Q band rings for about 2.2·Q/f₀ (research §8.3): seconds for Q 40 at 20 Hz. The
plugin keeps processing while any band's state is above a measured floor, then zeroes the state and
reports Normal, so a host can sleep it on real silence.

### 3.6 The realtime contract

`plugins/AGENTS.md`'s `process()` rules, unchanged: no allocation, locks or I/O; everything
preallocated at `activate()`; `reset()` leaves no tail; bounded internal blocks. Sample-accurate
automation is enabled, as the three original effects do, and v1 has zero latency (§6).

## 4. The analyser — the animated spectrum

- **The audio-to-editor handoff, as requirements.** How it is built is the implementation's choice.
  - **Coherent, ordered analysis windows.** A torn or spliced window smears energy into every bin,
    which a scope can tolerate but an FFT cannot (`plugins/mxm-mono-02/src/telemetry.rs` is the
    scope precedent). A gap restarts the window and is never spliced over.
  - **Audio never blocks, allocates or waits on the editor.**
  - **Every input channel is kept separate,** so F2–F5's L/R/M/S, grab, resonance and matching
    analysis can be built on the same boundary.
  - **No editor, no work** (`mxm-fx-curve`'s `connect()` gating).
  - **v1 displays the power average of the channels,** so anti-phase or side-only energy still shows.
- **GUI side.** The editor windows its history, runs a real FFT and converts to dB on a log-frequency
  axis. It smooths across frequency (fractional-octave) and time (ballistics) and draws behind the
  curve. Pre-EQ and post-EQ are told apart by fill against outline, never by hue alone (`AGENTS.md`
  *User Preferences*; research §18).
- **Resolution is a real trade, and it has a budget.** A low-frequency peak needs a long window:
  0.3–1.4 s to measure a room-mode width to within 20 % (research §8.6). At 192 kHz that is hundreds
  of thousands of samples.
  - **Display and analysis rates are separate.** The display animates at its own rate while a long
    window is recomputed at a hop proportional to its length.
  - **The long windows run on a decimated signal.** Low-frequency resolution analyses the
    low-passed, decimated signal rather than the full rate.
  - **The cost is measured.** Every resolution × pre/post/both × sample rate fits a budget stated at
    E3 (§8): **2 ms of a 60 Hz frame on the GUI thread**, measured at 0.35 ms at worst (192 kHz,
    High, both), in the plugin's `AGENTS.md`.
- **Settings are view state** — on/off, pre, post or both, resolution, speed and tilt (exact set at
  E3). They are not parameters, so they have no ids and no automation. They are saved with the
  project, and a change marks it dirty through `mxm-fx-curve`'s dirty-only state transaction
  (`plugins/mxm-fx-curve/AGENTS.md` *Editor transactions and history*). Presets, Init and undo leave
  them alone. A tilt makes typical music read level; the corpus average slope is about
  −4.5 dB/octave (research §10.6).
- **Motion rules.** `docs/MXM_DESIGN_SYSTEM.md` §9: a measurement display is appropriate motion. It stops
  when the editor closes, yields before audio is at risk, and honours the operating system's
  reduced-motion setting, or offers its own option where there is none.
- **The FFT is the shared `crates/mxm-fft`** on `realfft` (decision 6), owned by
  `plans/plan-mxm-fft.md` with its migration of the collection's in-house FFTs. It is used GUI-side
  only, so this plugin's DSP crate takes no dependency on it. E3 needs the crate (step X0 there), not
  the migration.
- **The analysis layer writes ordinary bands.** Spectrum grab (F3), the resonance finder (F4) and
  matching (F5) all run here, on the GUI side. Their output is band-parameter edits, made like any
  other edit, so the DSP stays a plain band cascade.

## 5. The editor

### 5.1 Layout — graph and side column

This follows `mxm-fx-curve`'s named exception to the cards contract (`plugins/mxm-fx-curve/AGENTS.md`
*Identity and host surface*; `plans/plan-editor-standard.md`):

- **Wide.** The graph takes every spare pixel horizontally and vertically, with an Out meter rail beside
  it (the shared `shell::vertical_level_meter`). A fixed-width column of natural height sits to the
  **right** (decision 3, owner, 2026-10-12).
- **What the column holds.** A **Band** card shows the selected band's controls: number, bypass, shape,
  frequency, gain, Q, slope and delete. Below it, **Output** (output gain and gain scale) and **Analyser** cards.
- **There is no stacked layout, and the window still narrows to one card.** The column stays to the
  graph's right at every size; it never moves under the graph. Narrowing first shrinks the graph to
  its floor. Below graph-plus-column, the whole surface scrolls horizontally, with the column still
  beside the graph, as `mxm-fx-curve`'s narrow flow already scrolls horizontally.
  **Vertically, the column scrolls on its own** when its cards are taller than the space, at any zoom
  up to 200 %. The graph keeps the height, and the column stays to its right.
  The minimum is one card wide, never narrower than the app bar's last compact step (`plugins/AGENTS.md`;
  design system §4.3), so a narrow host tile keeps every control reachable; a narrow-window
  reachability test holds it.
- **Why a column, not a bottom panel:** the owner's reason. The details panel never moves with the
  band count, so a variable number of bands cannot reflow anything (*"This will be too difficult to
  design with a variable amount of bands"*). Pro-Q floats its band panel under the graph; this
  is a deliberate difference from the benchmark.
- **The standard checks still apply:** opening size inside the quarter-4K budget, the minimum at 200 %
  zoom, tree checks on every card body, and the shared `opening_size` check `mxm-fx-curve` lacks.
- **With nothing selected,** the Band card has no subject. How it shows that is settled at E0.5 under
  the no-help-text rule (`plugins/AGENTS.md`): no sentence on the panel saying "select a band".

### 5.2 The graph — mouse

From Pro-Q and the collection's own canvas (research §17.2; `plugins/mxm-fx-curve/src/canvas.rs`):

- **Clicking the line adds a band there:** a bell at that frequency at **0 dB**, so the sound does
  not change until it is dragged. Bands add in dB (§2.2), so a bell started at the curve's height
  would count that height twice. This is the intent of `mxm-fx-curve`'s line-click insertion,
  which the owner accepted.
- **Where a point is drawn is decided at E0.5.** One option is at its own gain, Pro-Q's convention,
  with the composite curve drawn separately. The other is on the composite curve, where a drag
  changes the band's gain by the drag's distance. Either keeps insertion neutral.
- **Dragging a point** sets frequency (x) and gain (y). **The wheel changes Q only with focus or a
  modifier** (design system §7.1): over the selected point while the graph has focus, or with the
  modifier settled at E0.5. Otherwise the wheel scrolls the view, as the narrow layout needs.
- **Clicking a point selects it,** and the column follows. A right-click opens shape and delete.
- **Delete removes the selected band.**
- **Band solo is v1, the owner's sweep made into a feature** (research §17.3). Solo, switched on,
  plays what the band is about, so sweeping it hunts a resonance by ear.
  - **What it plays per shape** is a column of the E0 semantics table (§3.2). The proposal:
    - a bell, notch or band pass, a 0 dB bell included, plays a band-pass at its frequency and Q;
    - a cut plays what it removes;
    - a shelf plays the region it changes.
  - **While soloing,** frequency, gain and Q stay editable. **Solo latches** (the owner, 2026-10-14,
    in MXM Player: held, the band could not be edited while it played); Ctrl/Cmd held on a point
    still auditions it.
  - **It is an audition route, not a band edit.** It touches no parameter and no history, and it
    crosses the collection's transient-command path (`plugins/AGENTS.md`), so the DSP reads no editor
    state. The plugin's `AGENTS.md` says why saving or automating an audition would be false.
  - **Requirements on that path:**
    - turning solo off is always delivered, and no full path can reject it;
    - the shell ends solo on editor close, reset, a state restore or deactivation, without depending
      on the editor;
    - solo never runs in an offline render and is never saved;
    - the races are defined and tested.

    How the path is built is the implementation's choice.
- **v1 must not preclude the workflow phase (F8).** Selection is a set of bands, even though v1
  selects one at a time. Commands take their target set, and an undo action may span several bands.

Modifier gestures (fine drag, axis lock) are settled at E0.5 by hand, as `mxm-fx-curve`'s C0.5
probe settled its own.

**Band identity without hue.** Every point carries its slot number. The selected point is marked by
shape (a ring) as well as colour. Hue may add to that but never carries it alone. The owner is
red-green colour-blind; the research found one maker-documented colour-blind scheme (Eventide's)
and notes that Pro-Q draws right in red and mid in green (§18 there).

### 5.3 The keyboard — the collection's keyboard language

The owner's ruling (2026-10-12): *"follow newDAWn's held-key model"*. In the live collection that
model is the law for every editor already: **the keyboard language** (mxm-kit's design system §11,
engine `mxm-keys`, the keymap in use in the folder newDAWn and every editor share), read through
`mxm_ui::navigation`'s cursor. The archive's K1 table and its `held` layer are replaced by it (revision
20). Nothing here is the EQ's own but what the bands' verbs do.

- **The column and the bars are the cursor's**, as in every editor: the arrows walk the controls,
  VIEW + arrows go between the cards and up to the bars, a step key (or VALUE) + arrows edit a
  value as one gesture that OUT keeps and BACK cancels, F1 shows the keys.
- **The graph is a surface** (`navigation::surface`, added to mxm-kit for this editor): with the
  cursor on it, the language's outputs come to the editor, and they act on the bands.
  - ← → select the previous or next band by frequency, and step along the line where there is none;
    COARSE + ← → leaves the bands and walks the line an octave a press.
  - **MOVE** + ← → frequency (a semitone; COARSE an octave, MICRO a cent), ↑ ↓ gain (0.5 dB; 3, 0.1).
  - **VALUE** + ↑ ↓ gain. **EXTENT** + ← → narrower or wider (Q).
  - **MICRO** + ← → takes the band's start or end edge, which MOVE then moves alone, the other edge
    staying put: E0.5's "which edge a Q key keeps", answered by the language.
  - **DUPLICATE** + arrows carries a copy where they go. **ADD** makes a 0 dB bell at the keyboard's
    place, so a keyboard user starts from an empty Init; **REMOVE** and **DELETE** take it away.
  - BACK with nothing to cancel lets go of the edge, then of the band.
- **A gesture is one host gesture and one undo step**; BACK puts it back. BACK also cancels a mouse
  drag on the graph, as on every shared control.
- **Typing is the cursor's**: a focused text field or an open menu makes it inert.
- **Space** is not the language's, and the live window adapter has no key capture of the archive's
  kind; whether a host's transport hears it is E5's check.

**Proved by** the plugin's own language tests (every verb on the graph, gesture balance, BACK) and
the shared `keyboard_checks::the_cursor_reaches_and_operates_from` over the column in a bell's state
and a cut's.

### 5.4 Undo, presets, accessibility

- **Undo is required, and plain** (owner, 2026-10-12: *"simplify undo"*). The app bar has Undo and Redo
  (`docs/MXM_DESIGN_SYSTEM.md` §3.1), and so do Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z. They make deleting a
  band recoverable without a dialog (§7.2).
  - **What the history is.** A bounded list of editor actions, each the parameters one gesture changed,
    before and after. It belongs to the plugin instance, so it survives the editor closing
    (`mxm-fx-curve`'s precedent).
  - **What undo does.** It writes the action's "before" values back, as Pro-Q's undo does. If the host
    automates one of those parameters, the host's next automation point puts its own value back. A
    host writing automation records the undo like any other edit. No provenance tracking is
    attempted.
  - **Every editor write is seen at the editor's own host context**, a recorder that passes each
    write to the host (E3). It sees the graph's and the keyboard's writes as well as the column's,
    which an observer at the shared binding would not, and needs no shared change. Whole-patch
    replacements are what the preset bar writes, and a host restore is counted by the persisted
    view field.
  - **Epochs.** A host restore, a preset load or Init starts a new epoch, so an undo never reaches back
    across a replaced patch.
- **Presets** use `crates/mxm-preset` with parameters only. The slots are parameters, so a preset
  has no custom state, no fingerprint and no dirty-only transaction; its bands land in their saved
  slots (§2.1). Init is generated from the defaults, with every slot Off. The factory set is whatever
  the brief justifies; the fifty-preset floor is for instruments.
- **Accessibility**: every point and control has a name and a text value (`canvas.rs`'s `widget_info`
  is the precedent). Hover text follows the collection's rules for the player.

## 6. Contracts this touches, by owner

| Owner | What changes |
|---|---|
| Root `AGENTS.md` | *Naming* (`mxm-fx-eq-n` added to the effect names), DOX count and list, crate and plugin counts, the MSRV table, the Child DOX Index |
| `plugins/AGENTS.md` | The plugin table and child index. The side-column exception is recorded beside `mxm-fx-curve`'s, and the keyboard exception |
| mxm-kit `docs/MXM_DESIGN_SYSTEM.md` §11, `docs/plugin-conventions.md` | An editor's own view of items joins the cursor as a surface; the EQ's row, its Init exception and its layout exception |
| `docs/briefs/mxm-fx-eq-n.md` | New: the E0 brief |
| mxm-kit `crates/ui` | `navigation::surface` and `surface_outputs`, the sheet naming a surface's jobs (on main since `69083b4`) |
| mxm-kit `crates/mxm-fft` | New and shared (on main since `69083b4`); this plugin's editor is its first consumer |
| `crates/mxm-preset` | Nothing: undo records at the editor's own host context (§5.4, E3) |
| `vendor/nice-plug` | Nothing in v1. F6 makes the collection's first latency report: `set_latency_samples` and `request_restart` exist, but the player's headless restart path is untested (`plugins/mxm-mono-00/AGENTS.md`) |
| The workspace | `repos.txt` gains the repository once the owner creates it on GitHub |
| This repository | `bundler.toml`, `README.md`, `Cargo.toml`: its own, as every product's |
| Control map | None in v1. No role fits on-demand bands, and a map is optional |

## 7. Later phases — the rest of "flexible and useful"

Each later phase is a new revision of this plan, ordered by the owner (decision 9). Each builds on
the band pool and the analysis layer, not beside them.

| Phase | What | Evidence | What it needs decided first |
|---|---|---|---|
| F1 | **Dynamic bands**: threshold, range, attack, release; the band's own filter as side chain. Designed in §7.1 | Research §7 (Wise 2009: side chains parallel, audio in series, one control across channels); `research:effects/dynamics-processing.md`. The band stays the v1 SVF: E1's gain-modulation measurement passed (§3.1) | Decided (12–14). The external side-chain input is a later step of its own |
| F2 | **Per-band stereo placement**: stereo, L, R, mid, side | Research §6: a minimum-phase band on one channel moves the image | How dynamic bands link across channels |
| F3 | **Spectrum grab and analyser extras**: freeze, peak labels, drag a peak to create a bell; a reference overlay | Research §11: the gesture is documented, the peak picker is not | Which spectrum is grabbed; how peaks are chosen |
| F4 | **Resonance finder**, writing ordinary cut bands | Research §8–§9: prominence, persistence and decay tests; professionals removed about a fifth of the excess; a cut does not shorten a ring | Detector and amount. Continuous spectral suppression must first clear oeksound's active patent, US 10,587,238 (research §9.6): a legal and product decision |
| F5 | **EQ match to a reference**: gated, level-aligned long-term ratio, smoothed, applied as an amount, fitted to bands | Research §10: 15–20 fitted bands were indistinguishable from a 2,048-tap FIR; makers advise under 50 % | The reference's source; whether the fit uses free slots |
| F6 | **Phase modes**: linear, perhaps analogue-matched | Research §5: 100–400 ms pre-ring on narrow low cuts | The collection's first latency report (§6) |
| F7 | **Candidates**: character/saturation, auto-gain, band modulation, cross-instance collision | Research §13–§15, §12 | Each one is the owner's call |
| F8 | **Workflow**: multi-band selection with relative editing, copy and paste of bands, global transforms (scale all gains, shift all frequencies) | Research §17.2–§17.4: established across the surveyed products | Selection gestures; what a transform keeps |

**Kit follow-up, not a phase**: mxm-kit's shared binding (`mxm_params::binding::Bound`) reads a
parameter itself, so in any editor a control pressed within one host buffer of another write to
the same parameter starts from the value before it. A pending-aware read there, like this editor's
`normalised_now`, would close it for every editor (round 4 of the E3 code review, 2026-10-13).

### 7.1 F1 — dynamic bands (revision 23, for the owner's approval)

The owner's rulings of 2026-10-14 (decisions 12–14): **Pro-Q's controls** — Threshold, Range,
Attack, Release, no ratio; **each band listens to its own region**; **no external side chain in F1**.
That is a later step of its own (§7.2, now designed), once newDAWn routes a side chain to a plugin's second input, so it
can be tested on the work machine (MXM Player's chain has no side chains; newDAWn's extra inputs hear
silence). mono-08's input was removed because a synth did not need it (the owner), not as a rule.

**What a dynamic band is.** A **bell or shelf** whose gain moves with its own level: at or below its
threshold it is the band as drawn; above it the gain moves toward the drawn gain plus Range. Range is
signed: below 0 it cuts what gets loud (a resonance, sibilance), above 0 it lifts it. **A band is
static while its Range is 0**, so every v1 patch, preset and project means what it meant. Cuts,
notches and band passes have no gain; **the tilt is not dynamic either**: it lifts one side of its
pivot and cuts the other, so "cut what gets loud" has no meaning for it (Pro-Q's flat tilt is not
v1's tilt). None of them shows dynamics controls.

**The parameters, appended, four per slot** (96, to 242): `range_<n>`, `threshold_<n>`, `attack_<n>`,
`release_<n>`, permanent from their first release, after the 146, so no v1 id moves (§2.1).
**Migration, both ways in**: a preset without them loads them at their defaults
(`default_missing_legacy_parameter`); so does a host state, through the shell's `filter_state`, which
writes each missing one at its default before nice-plug restores, since nice-plug touches only the
ids a state holds (`kit/nice-plug/src/wrapper/state.rs`), and extends a loaded preset's persisted
baseline with each at its default, as `mxm_preset::add_switches_off` does for switches, so an old
project's preset keeps its clean or Modified meaning. Either restore into an instance already
dynamic leaves every slot static.

**F1.0, the brief's addendum, signed by the owner before any code:**
- **Ranges**: Range −30 to +30 dB, default 0; Threshold −60 to 0 dB; Attack about 0.5–200 ms;
  Release about 10–2,000 ms; the defaults.
- **The law**, proposed: above Threshold, through a 6 dB soft knee, the band's gain moves at a fixed
  slope — **3:1 for a cut, 1:1.5 for a lift** — and eases into Range rather than stopping at it: with
  `a` the movement that slope asks for, the moving part is `Range · (1 − e^(−a/|Range|))`. So the
  band never passes Range; a louder band never comes out quieter (its slope stays between 3:1 and
  1:1); and at any level more Range never means less action, while Range toward 0 takes the band
  smoothly back to static. The owner signs the slopes and the knee.
- **The bound**: the heard gain — the gliding drawn gain plus the moving part — is held inside the
  band's ±30 dB at every sample, as a static gain is, in the DSP and in the gain telemetry hands the
  graph, so the two never differ; the moving part's own state is not clamped, so its ballistics run
  on. Then the gain scale applies, and the DSP's ±60 dB domain (`MAX_EFFECTIVE_GAIN_DB`) holds
  every case.
- The names of two or three dynamic factory presets (a de-esser, a resonance tamer).

**The signal path keeps Wise's form** (research §7):
- **Audio in series, detectors in parallel**: every detector reads the EQ's input, never the bands
  before it, so **the gain each band takes never depends on slot order**. The sound can, slightly:
  filters whose gains move do not commute exactly. F1.1 measures two reacting bands swapped, in the
  DSP's `AGENTS.md`; the owner is asked if it is audible.
- **The detector's filter is the band's region, fixed**: the design solo plays (`design_solo` — a band
  pass at the band's frequency and Q for a bell, the shelf's region for a shelf), its own state,
  never moved by the dynamic gain.
- **Linked peak level**, one gain for both channels, and **the ballistics smooth after the law**, as
  `mxm-fx-curve`'s detector does (it measured −49.6 against −35.1 dB of gain-signal energy above 10 kHz
  at its fastest when the level was smoothed first). **What they smooth is the moving part itself, signed, in
  dB**: Attack while it moves away from rest (growing, in the law's sign), Release while it moves back
  toward rest or across it, so a lift is timed as a cut is (`mxm-fx-curve` picks by the gain's
  numeric direction, which would swap them for a lift). One pole per direction, so the state only
  ever lies between where it was and where the law now asks: a Range edit, a sign change included,
  never overshoots either. **Range glides** as the drawn gain does (§3.4).
- **Zero latency**, no look-ahead (that needs F6's latency report); **the band is the v1 SVF** (§3.1:
  envelope-rate gain modulation −89.7 dB above 5 kHz).

**Idle.** A band whose Range is not 0 can be heard to move, so it runs, detector included, whatever its
drawn gain — parking it between hits would bring every hit in through §3.5's slow re-entry fade. It
parks, and re-enters as v1 bands do, whenever nothing it does can be heard: Range 0 with an identity
drawn gain, **or the gain scale at 0 %**, which makes every bell and shelf flat (brief A5). Off and
bypassed bands cost nothing, as before. **The host keeps calling until every moving part is at an
exact rest**: a release ends, at an inaudible floor stated in the DSP's `AGENTS.md`, at exactly 0, and
a detector's state is zeroed on silence as the filters' are; only then does the shell report
`Normal`. So a host's sleep starts from rest, and waking is a silence that ran its course.

**Cost.** A band that keeps reacting is a steady load, so **the steady budget applies: 35 %** (brief
A6), with the worst case 24 dynamic bells moving, stereo, 192 kHz, editor open; 70 % is only for
structural transitions on top. F1.1 measures it first. Over budget, the coefficients are recomputed
at a decimated rate under the gain's smoothing, measured against per-sample; the owner is asked
before any budget moves.

**The editor.** The Band card gains a Dynamics group for bells and shelves — Range, Threshold,
Attack, Release on the shared binding, and a meter of the band's moving gain; reserved to the tallest
shape, so the opening size and minimum are re-measured (§8). The graph draws a dynamic band's range
as a ring around its point, Pro-Q's way, and the curve at each band's gain now, from telemetry, the
drawn curve faint beneath; both are the DSP's own design (§3.1). Telemetry is two gains per band — the
heard (bounded) and the drawn as the DSP glides it — atomics, only with an editor connected (§4): the
curve draws the heard one, and the meter their difference, both under the gain scale as the audio applies it: the action heard. The controls are the column's, so the cursor reaches
them; no new job in the language.

**Tests**, research §25.4 first: the law against a steady tone's level across Threshold and Range,
cut and lift, and the bound at every extreme of drawn gain, Range and gain scale; attack and release
against their values, low and high, a lift timed as a cut is; the detector's band against the audio band, and a narrow cut's
group-delay mismatch measured (Wise's "conundrum"); broadband energy at the fastest attack; sample-accurate Range automation through
both signs inside the transition bound (§8); the law monotonic in level at every Range, and in Range at every level; the gains independent
of slot order, and the swapped-order difference measured; the idle rule (Range 0, gain scale 0 %, a dynamic band never fading in on a
hit); `Normal` reached after bounded silence, and no stale gain on the first audio after a host's sleep;
a Range edit mid-action never past either law's response; a drawn-gain edit against an active
moving part inside the bound, the graph's gain equal to the heard one; Init bit-exact dry; no allocation with every band dynamic; an old preset and an old host state
leaving an already-dynamic instance static, and an old project's loaded preset still clean; 242 ids complete, unique and permanent; the Dynamics
group's tree in every Band state; the ring, the moving curve and the meter only with an editor.

**Rejected**, with reasons:
- **Side chains in series** (MDynamicEq's default): slot order would change the sound, Wise's race.
- **The band's own SVF as its detector**: its damping follows the gain (§3.1), so the detector's
  width would move with the gain it decides.
- **Smoothing the level before the law**: more gain-signal energy, as `mxm-fx-curve` measured.

**Delivery.** F1.1 was reviewed on its own; from F1.2 the code of F1.2–F1.4 is written first, its tests written and run at the end, and one agent-loop review covers it (the owner, 2026-10-14: *"write down what to test and then do the tests in the end"*):

| Step | Delivers | Gate |
|---|---|---|
| **F1.0** | The brief's addendum: ranges, defaults, the law's slopes and knee, the bound, the presets' names. **Signed** (2026-10-14): the brief's §C | Owner |
| **F1.1** | DSP: detector, law, bound, ballistics, the idle rule; the cost measured. **Built** (2026-10-14): `dynamics.rs`, the engine's dynamic bands, 15 new tests; 24 dynamic bells all moving cost 9.8 % at 192 kHz against the 35 % steady budget; the detector's delay, the fastest attack's broadband energy and slot order measured (`eq_n_dynamics`). **Code review approved** (agent-loop, four rounds, 2026-10-14) at `d436275`: a reset during a cut, a band back from Off and `is_settled`'s waiting moves fixed | Measurements in the DSP's `AGENTS.md` |
| **F1.2** | Shell: the 96 parameters, both migrations, telemetry | Unit tests; bundle; `clap-validator` |
| **F1.3** | Editor: the Dynamics group, the ring, the moving curve, the meter | Editor tests; owner by eye |
| **F1.4** | The dynamic presets beside the thirteen static ones, the factory tests learning the new fields; host tests through MXM Player | Merge gate |
| **F1.5** | Listening, and a DAW check at home (Bitwig) | Owner sign-off |

### 7.2 The external side chain (revision 24, for the owner's approval)

The owner put it next, before F2 (2026-10-14, *"make the external side chain next"*), and chose
**the EQ only, heard at home** (decision 15): neither host on the work machine routes a side chain
(MXM Player's chain is one line; newDAWn feeds extra inputs silence until its after-v1 routing), so
the key path is proved by tests here and heard in Bitwig with F1.5.

**What it is, Pro-Q's way** (decision 16; `research:effects/parametric-equalizers.md` §7, Pro-Q 4's
manual: a per-band button "between triggering on the plug-in input signal, or the external side
chain input"): **each dynamic band chooses what it listens to** — its own input, as F1, or the
side-chain input — and **the key is band-limited exactly as the input is**, through the band's own
detector filter (§7.1). Only the detector's source changes: the law, the bound, the ballistics and
the audio path are F1's. Wise's form holds — the detectors stay in parallel, now on two inputs.

**The port.** One auxiliary input, **"Side chain"**, with the main input's channel count. The
layouts become *stereo + stereo key*, *mono + mono key*, *stereo*, *mono*, in that order:
- the keyed layouts first: nice-plug starts on the first layout, so a host that never selects a
  configuration offers the input (whether Bitwig selects one is SC.4's check);
- **the configuration ids are set here, once, and permanent from SC.2 on**, as parameter ids are:
  nice-plug's id is the layout's index, and the EQ has never been released (private, pre-alpha, no
  tags) and its one host so far, MXM Player, negotiates on every load and saves no id, so no project
  holds the old ones. The layouts test pins the four, ports and names;
- MXM Player picks the first configuration with one input port (`envelope.rs`,
  `select_effect_configuration`), the plain stereo one, so it keeps loading the EQ; newDAWn accepts
  extra ports and feeds them silence.
Brief A1 gains the port (SC.0). Still zero latency, no look-ahead.

**A keyed band with no key hears silence.** Unrouted, a layout without the port, or a host feeding
silence: the band releases to rest and is the band as drawn. It never falls back to its own input —
a dropped route must not silently change what triggers the band. The hover says so.

**Parameters.** One per slot, appended after the dynamics so nothing moves (266 in all): `key_<n>`,
a switch named "Band n side chain", Off by default. An older preset (`default_missing_legacy_parameter`)
or host state (`filter_state`, mxm-kit's `add_switches_off`, made for exactly this) restores every
band to its own input, a loaded preset still clean. It matters only for a dynamic shape; the
engine ignores it elsewhere, as it does Range. **It is a slot field like the other ten**: a new band
writes it Off, so a slot never inherits a deleted band's source, and a copy carries the original's
(the editor's `slot_writes`, eleven fields).

**The DSP.** The engine's block calls take an optional key (`process_stereo_keyed`,
`process_mono_keyed`, the old calls passing none): per sample, a keyed band's detector reads the
key's sample, linked across its channels, in place of the dry input's. Switching a band's source
starts its detector fresh, as a shape change does; the moving part carries on through the ballistics,
so a switch never steps the gain. With no band keyed the output is bit-exact what F1's is, key or no
key. **Activity:** a keyed band that is moving keeps the host calling (`is_ringing`, F1's rule), so a
key that plays while the main input rests leaves the band where the key put it when the main input
returns; with every band at rest the idle rule is F1's.

**Solo plays what the band listens to** (decision 17; Pro-Q's audition "listen to the current
triggering signal"): for a keyed band, the key through the band's detector filter. For every other
band solo is unchanged — it already is the band's own detector signal (§7.1). A keyed band with no
key solos silence, as it hears silence.

**The editor.** The Dynamics group gains a **Side chain** toggle (the kit's `toggle_sized`, as Solo
and Analyser), its hover saying what it listens to and that with nothing routed the band stays as
drawn. A keyed band's ring is dashed on the graph. The column is re-measured: one row more, and the
opening still inside 1920 × 1080 (§8). No side-chain spectrum: that is F3's analyser work.

**No factory preset.** A keyed preset does nothing until a route exists, so it would read as broken
in MXM Player; F1's two cover the dynamic starting points.

**Tests** (written and run at the end, the owner's rule): the key, not the input, triggers a keyed
band (a loud key cuts a quiet input; a loud input with a silent key does not), linked across the key's
channels; a source switch mid-stream steps nothing and starts the detector fresh; no band keyed is
bit-exact F1, key present or absent; no allocation with a key; a keyed band unrouted is the band as
drawn; solo of a keyed band is the band-limited key; the layouts' order and port names; the 266 ids;
both migrations; a new band in a keyed slot is unkeyed and a copy of a keyed band is keyed; the
toggle's group, keyboard reach and hover; the opening size. Through MXM Player:
the EQ still loads, on the plain stereo configuration, and every existing host test passes. The key
path through a real host is the owner's check at home.

**Delivery** (one review over SC.1–SC.3, tests at the end):

| Step | Delivers | Gate |
|---|---|---|
| **SC.0** | Decisions 15–17 and brief A1's port, signed. **Done** (2026-10-14) | Owner |
| **SC.1** | DSP: the keyed block calls, the detector's source per band, solo's source. **Built** (2026-10-14): 8 side-chain tests, the realtime test keyed | DSP tests at the end |
| **SC.2** | Shell: the port and layouts, 24 switches, both migrations. **Built**: MXM Player selects configuration 2 | Unit and host tests at the end; `clap-validator` |
| **SC.3** | Editor: the toggle, the dashed ring, the re-measured column (opening 1204 × 1048). **Built**; review approved in round 2 | Editor tests at the end; one agent-loop review of SC.1–SC.3 |
| **SC.4** | Listening in Bitwig at home, a kick keying a bass's low bell, with F1.5 | Owner sign-off |

## 8. What must not break

| Property | Proved by |
|---|---|
| An empty EQ at 0 dB output is bit-exact dry | A DSP test on noise, silence and NaN-laced input |
| Band ids are complete, unique and permanent; a slot is never renumbered; a new band takes the lowest Off slot; presets land in their saved slots | A nested-ids test after `mxm-drum-machine`'s, plus create/delete/preset/Init tests on slot numbers |
| No allocation, locking or I/O in `process()` | The collection's allocation-guard tests, with every slot in use and every type changing |
| Creating, deleting, bypassing and changing a band's type are click-free, including when changes arrive mid-transition | A transition test that bounds the per-sample step under repeated sample-accurate type, bypass and slope automation, in the style of `mxm-fx-curve`'s transition probe |
| Fast automation stays bounded | The research's square-wave frequency-modulation and DC-step tests, run against the shipped band |
| Low-frequency, high-Q precision | The research's 20 Hz / Q 10 / 192 kHz case, as a test |
| The drawn curve is the audio's response | Drawn curve against the cascade's rendered response, across sample rates, shapes and slopes. Its tolerance is below the audibility thresholds in research §8.8, stated in the DSP `AGENTS.md` |
| The worst-case configuration (§3.5) fits the budget E0 declared; identity bands cost nothing and re-enter click-free | A release benchmark of that configuration at 44.1–192 kHz, figures recorded in the DSP `AGENTS.md`; tests that park 0 dB bells across the frequency and Q range, confirm no core work, then move their gain: the step is bounded (click-free), and the response matches a newly created band's, settling within the ring time |
| The analyser does no work with the editor closed; its windows are coherent; its cost is bounded | A telemetry gating test; a sequence-gap test (dropped blocks restart the window, never splice); a cost measurement at every resolution × pre/post/both × sample rate, within the budget stated at E3 |
| Undo and redo reverse whole editor actions, in order, and never cross a preset load, Init or host restore | Undo tests against a setter that records gestures without applying them (after `crates/mxm-preset/src/binding.rs`'s regression), plus epoch tests |
| Every parameter is reachable and operable by the held-key commands, starting from an empty Init; a held edit is one gesture; focus loss releases | The plugin's own keyboard coverage test, which starts from Init with no pointer and creates, edits and deletes a band and reaches the column and analyser, plus gesture-balance tests after `plugins/mxm-mono-01/tests/keyboard_editing.rs` |
| The host keeps its keys | E0.5 and E5: MXM Player plus at least one DAW on Windows. Letters act only while the editor is focused; Space reaches the host outside text entry and goes to the field inside it |
| Band identity and pre/post spectra readable without hue | Painted checks on the graph |
| The editor fits the quarter-4K budget at 75–200 % zoom; the graph takes the slack | The shared opening-size check plus `mxm-fx-curve`-style growth tests |
| Presets and Init round-trip | Preset tests; Init equals the empty EQ |
| A loadable, valid plugin | The release bundle, `clap-validator`, `host-tests/` through MXM Player (`Harness::with_fx`) |

## 9. Delivery

| Phase | Delivers | Gate |
|---|---|---|
| **E0** | `docs/briefs/mxm-fx-eq-n.md`: name, v1 scope, shape and slope set, ranges, the per-shape semantics table (§3.2), the gain scale's range, the worst-case DSP budget (§3.5) | Owner |
| **E0.5** | A disposable interaction probe in MXM Player: graph and right column, click-to-add, drag, where points are drawn (§5.2), the K1 held-key layer, an analyser stub. Also the key-delivery spike: physical-key down/up in MXM Player and one DAW, under a QWERTY and a non-QWERTY layout, no host double action, Space passing through. **Built** as `apps/mxm-eq-n-probe`, with the egui-baseview physical-key patch, and **deleted at E3** (2026-10-13). Its code was reviewed by the agent-loop and **approved in round 11** (2026-10-12; 30 findings fixed, two rejected with evidence). **Owner so far:** per-band curves are "mostly visual noise", now a switch, off by default; a narrow bell's solo ringing is "fine" (solo stays a band-pass at the band's Q); momentary solo reads *Hold* and holds for as long as it is pressed. **The owner moved on to E1 on 2026-10-13** (*"no continue the plan"*), leaving the rest of the probe unjudged. Those items move to the phases that can judge them. Where points are drawn, latching or momentary solo, which edge a Q key keeps and the K1 keys go to E3's by-eye check. Key delivery in a DAW and on a non-QWERTY layout goes to E5, including whether Space reaches the host on Windows (baseview there ignores the plugin's handled report) | Owner by hand |
| **E1** | `crates/mxm-fx-eq-n-dsp`: bands, cascade, smoothing, transitions, the cramping choice, precision, cost at the cap, tail and activity, silence/NaN/reset tests; MSRV 1.87 verified. **Built** (2026-10-13), 49 tests; the worst case measures about 20 % steady and up to 57 % with every band crossfading and gliding at 192 kHz. Its code was reviewed by the agent-loop and **approved in round 6** (2026-10-13; 16 findings fixed, none rejected), inside A6's 35 % and 70 % | Measurements in its `AGENTS.md` |
| **E2** | `plugins/mxm-fx-eq-n` shell: the slot pool, layouts, Init, presets seam, telemetry rings, meters, permanent-id test. **Built** (2026-10-13): 30 tests; the release bundle passes `clap-validator` (33 passed, 0 failed, in debug and release); code **approved by the agent-loop in round 1** | Unit tests; bundle; `clap-validator` |
| **E3** | The editor: graph, column, analyser, held-key commands with the 3×3 map, undo, band identity, accessibility, every editor check. **Built** (2026-10-13): 25 tests in the editor and its modules, 59 in the plugin, and a release cost measurement; the analyser within its budget; the probe deleted. For the owner's eye: points at their own gain, solo held, an edge key keeps the other edge, K1 as newDAWn has it **Ported** to this repository (2026-10-13) onto the live kit: the keys are the language, the graph a surface; 64 tests, the column under the shared keyboard check. **Code review approved** (agent-loop, six rounds, 2026-10-13) at `6a45037`; the kit side (mxm-fft, `navigation::surface`) in three rounds, on kit main since `69083b4` | Editor tests; owner by eye |
| **E4** | Factory presets, `host-tests/`, inventories, DOX closeout (§10). **Built** (2026-10-14): thirteen presets from `preset.rs`'s design table, generated and tested in each preset's own state; five host tests through MXM Player (`Harness::with_fx`, the CLAP state path); the DOX closeout below done, the durable rules in the plugin's and the DSP's `AGENTS.md`. **Code review approved** (agent-loop, two rounds, 2026-10-14) at `5287a9a`: round 1's two findings fixed (a missing fetched source now fails the host tests; `plugins/AGENTS.md` owns `host-tests/`) | Merge gate |
| **E5** | Listening, by-eye and DAW checks | Owner sign-off |

v1 is E0–E5. F1–F8 follow in the owner's order.

## 10. DOX closeout

On delivery: new `plugins/mxm-fx-eq-n/AGENTS.md` and `crates/mxm-fx-eq-n-dsp/AGENTS.md`, with their
contracts and measured figures; the §6 owners updated; `crates/ui/AGENTS.md` for the held-key
layer. The durable rules then leave this plan for those files: the slot lifecycle,
the keyboard model, the cramping choice and the side-column exception.

**Done at E4** (2026-10-14). The slot lifecycle, the keyboard model and the side-column exception
are in `plugins/mxm-fx-eq-n/AGENTS.md`, the cramping choice in `crates/mxm-fx-eq-n-dsp/AGENTS.md`;
the surface is in mxm-kit's `crates/ui/AGENTS.md` and `docs/plugin-conventions.md`, which also
carries this plugin's row; the workspace lists the repository (`repos.txt`). Left for the owner:
two inventories in public repositories still count ten effects — the workspace's `AGENTS.md`
(changed locally, not pushed) and mxm-kit's `docs/plugin-conventions.md`, whose table also lacks
`mxm-fx-oldify`'s row.

**Evidence:** `research:effects/parametric-equalizers.md`, a comparative survey dated 2026-10-12. Its
§25 validation programme is what E1–E5 draw on.
