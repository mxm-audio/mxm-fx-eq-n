# plan-mxm-fx-eq-n — a parametric EQ with only the bands you need

**Revision 36 — An unsure difference is shrunk, never left out** (2026-10-28; §7.6, decision 49).
The owner, matching a voice-over in Resolve, found one +3 dB bell proposed, nothing at 40 %, and the
lines' shape still while only their drawing changed: *"that seems like too little"*. A voice's
halves differ by a dB or more almost everywhere, so decision 49 had left nearly every frequency
out. Now each difference's shape is shrunk by its spread, the lines are drawn solid, the card reads
how many bands are proposed and Apply's hover why it waits. **Built** (2026-10-28, F5.11).
**Revision 35 — Match settles by frequency, and is heard before it is applied** (2026-10-28; §7.6,
decisions 49 and 50). The owner asked why proposals waited a fixed 10 s, and whether the match
could be heard while Amount and Detail move: *"Yes drop the 10 secs"*, of a learn read in halves
that counts each frequency once it has settled, and of a Preview played as solo is. **Built**
(2026-10-28, F5.10); F5.7, Resolve, is the owner's. **Revision 34 — One learn per EQ, matched to another's** (2026-10-28; §7.6, decisions 31 and 48).
The owner, after learning from another track in Resolve: *"This shit is too convoluted. How does
other plugins do it?"*, then, of the design each other maker has, every instance learning its own
track and one chosen to match to, *"yes, rebuild it that way"*. **Built** (2026-10-28, F5.9); F5.7,
Resolve, is the owner's. **Revision 33 — Match under the Band card, a lamp for each learn, what was learned drawn**
(2026-10-28; §7.6, decision 34). The owner, trying the learns in Resolve with nothing reaching the
reference: *"make something go green or something when audio is comming in"*, *"could the match
card not be under the band card?"*, and with a learn green, *"I still cannot see the eq curve of
what has been learned"*. **Built** (2026-10-28, F5.8); F5.7, Resolve, is the owner's. **Revision 32 — F5's Match redesigned for the owner's approval** (2026-10-28; §7.6, decisions 31
and 48). Having tried Match in Resolve, the owner asked for it to learn as Find does (*"yes,
redesign it that way"*): the reference and the track each learned while on and held while off, the
reference from this track, the side chain or another track, and saved by name so any track can be
matched to it (decision 48, the owner's choice). **Signed by the owner** (2026-10-28, *"yes, go
ahead"*); **F5.5–F5.6 built** (2026-10-28); F5.7, Resolve, is the owner's. **Revision 31 — F8, working with several bands, designed for the owner's approval** (2026-10-16;
§7.9, decisions 44–47). **Its design approved by the agent-loop in round 3** (2026-10-16): round 1's
three blockers — the selection's primary and order, the shortcuts against the shared keymap, F8.3's
gate — and round 2's, a point's menu against the selection, with their other findings, all
accepted and applied here. **Signed by the owner** (2026-10-16, *"Approve all and build"*, decisions
44–47 as proposed); F8.0 done. **F8.1–F8.3 built and reviewed** (2026-10-16; the agent-loop's
code review READY in round 4: the history held by a counter, not a parameter; a group's limits read
frequencies as they run; the selection sorted before it is deduplicated; the title's hover naming
the group; a follower's gesture abandoned with the primary's; a card slider's group stopping at a
limit inside its own gesture); F8.4, Bitwig, is the owner's. **Revision 30 — F7, auto gain and other tracks' spectra, designed for the owner's approval**
(2026-10-16; §7.8, decisions 38–43): of F7's four candidates the owner chose these two
(2026-10-16). **Its design approved by the agent-loop in round 2** (2026-10-16): round 1's three
blockers — collisions marked on silence, the make-up's composition with Output gain, F7.4's gate —
and its other findings, all accepted and applied here; round 2's two MINORs applied after it. **Signed by the owner** (2026-10-16, *"Approve all and build"*, decisions 38–43 as proposed, with 42 the push to mxm-kit's `main`); F7.0 done. **F7.1–F7.4 built
and reviewed** (2026-10-16; F7.2's `mxm-editor` addition is mxm-kit `5f34311`; the agent-loop's code
review READY in round 4: a restore's id check and store under one lock, a watched track whose id
changes let go, shown names never alike, the make-up documented as the output's target, watches
begun and ended under the feed's lock, no collision marks with the analyser off); F7.5, Bitwig, is
the owner's. **Revision 29 — F6, phase modes, designed for the owner's approval** (2026-10-15; §7.7, decisions
35–37). **Its design approved by the agent-loop in round 2** (2026-10-15): round 1's four blockers
— the kernel designed off the audio thread into preallocated banks, old and new banks on one
delay line, the editor following the mode in force, the window and centring exact — and its
other findings, all accepted and applied here. **Signed by the owner** (2026-10-15, decisions
35–37 as proposed); F6.0 done. **F6.1–F6.3 built and reviewed** (2026-10-15; the agent-loop's
code review READY in round 4: the convolution made `f64` for the whole gain range, the rate's
sizes capped, a failed design thread no restart loop, the hovers' delay the one in force, a reset
asking for its kernel at once); F6.4, Bitwig, is the owner's.

**Revision 28 — F5, EQ match to a reference, designed for the owner's approval** (2026-10-15;
§7.6, decisions 31–34). **Its design approved by the agent-loop in round 3** (2026-10-15): rounds
1–2 filed five blockers — the level taken out after the Amount, the EQ counted through the source's
covariance, shelves reachable at the eased ends, the tests aligned with the stopping rule and the
incremental fit — and nine further findings, all accepted and applied here. **Signed by the owner**
(2026-10-15): decisions 31 and 32 as proposed; 33 and 34 their alternatives — a Detail control, and
the Match card under Resonances, approved by the agent-loop in round 4; F5.0 done. F5.1–F5.2 built (2026-10-15) and their code approved by the agent-loop in round 2 (one blocker: quiet frames now ask a new fit); F5.3, the owner's listening in Bitwig, is next. **Revision 27 — F4, the resonance finder, designed for the owner's approval** (2026-10-15; §7.5,
decisions 27–30). **Its design approved by the agent-loop in round 5** (2026-10-15): rounds 1–2
filed seven MAJOR findings and rounds 3–4 seven blockers, all accepted and applied here. **Approved
by the owner** (2026-10-15, *"Approve all and build"*, decisions 27–30 as proposed); F4.0 done.
The owner also set the rest of the plan's course: F5 to F8 phase by phase, each designed, reviewed
and signed before it is built. F4.1–F4.2 built (2026-10-15) and their code approved by the agent-loop in round 6 (2026-10-15: the worker's lifetime, the gain scale's reach, listens kept apart, every sample rate read to as high as a band runs, excitations as clusters); F4.3, the owner's listening, is next. **Revision 26 — F3, spectrum grab and analyser extras, designed for the owner's approval**
(2026-10-15; §7.4, decisions 23–26; its design approved by the agent-loop in round 3: 5 findings over two rounds,
all accepted). **Approved by the owner** (2026-10-15, *"Approve and build"*, decisions 23–26 as
proposed); F3.0–F3.3 done, their code review approved in round 7; F3.4, the owner by eye and ear, next. **Revision 25 — F2, per-band stereo placement, designed for the owner's approval** (2026-10-14;
§7.3, decisions 18–22; its design approved by the agent-loop in round 3, 2026-10-14: 5 findings over
two rounds, all accepted). **Approved by the owner** (2026-10-15, *"Approve and build"*: decisions 19,
20 and 22 as proposed; 18 and 21 their alternatives — Split now, no presets); Split's design, added
after that, approved by the agent-loop in round 6 (3 findings over rounds 4–5, all accepted). F2.1–F2.3
built (2026-10-15), their code review approved in round 3 (11 findings over two rounds, all accepted; round 3's one
MINOR fixed after it). F2.4, the owner's listening, is next. **Revision 24 — the external side chain, designed for the owner's approval** (2026-10-14; §7.2,
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
| 29 | **F6 designed** (2026-10-15), §7.7, for the owner's approval: Zero latency and Linear phase, no Natural phase (decision 35); three resolutions, Medium by default (decision 36); dynamic bands held at their drawn gain in Linear phase (decision 37); a zero-phase kernel per channel or a 2×2 matrix for placed bands, uniformly partitioned, its latency `N/2 + B` reported at activation and a change restarting the plugin; `phase` and `resolution` appended, not automatable; the DSP crate's first dependency, `mxm-fft` |
| 30 | **F7 designed** (2026-10-16), §7.8, for the owner's approval: of F7's four candidates the owner chose auto gain and other tracks' spectra; auto gain estimated from the curve, K-weighted over the corpus mean mix (decisions 38, 39), a parameter off by default (40); instances meeting in one process (41), named by the host's track names through an mxm-kit addition (42), one external spectrum with collisions (43) |
| 36 | **An unsure difference is shrunk, never left out** (2026-10-28), §7.6, from the owner's trial in Resolve: decision 49 rewritten, each difference's shape shrunk by its spread, `s·s²/(s² + u²)`; the learned lines solid; the card's reading the bands proposed; Apply's hover why it is disabled |
| 35 | **Match settles by frequency, and is heard before it is applied** (2026-10-28), §7.6, the owner's asking: no fixed 10 s, each frequency counting once the halves of both learns agree there (decision 49); Preview, the proposals played in the free slots as a transient command (decision 50); the card five rows |
| 34 | **One learn per EQ, matched to another's** (2026-10-28), §7.6, the owner's asking: Learn learns this track, its input and its output, kept with the project; Match to chooses another EQ that has learned, or a saved reference (decision 31, rewritten; 48); the From list, the two learns and learning another track while it plays gone; the card four rows |
| 33 | **Match under the Band card, a lamp for each learn, what was learned drawn** (2026-10-28), §7.6, the owner's asking: each learn's lamp green while it hears sound; the card in the first column (decision 34, rewritten), the source on a row of its own and the two learns side by side; the window opening hugged to the first column, shorter; the reference and the track learned drawn as lines at the analyser's level, a learn's level read while it sounds |
| 32 | **F5's Match redesigned** (2026-10-28), §7.6, for the owner's approval: a reference and the track each learned and held, as Find is (decision 31, rewritten); the reference from this track's input, the side chain or another MXM EQ's output; references saved by name in a folder beside the user presets, any of them chosen for any track (decision 48); Match offered on every layout |
| 31 | **F8 designed** (2026-10-16), §7.9, for the owner's approval: a selection of bands and its primary, by Shift-click, a rectangle on the background, SELECT and Select all (decision 44); a group moving by one ratio, offset and factor, stopping at its first member's limit (45); copy, cut and paste through a clipboard in the process (46); global transforms from the selection and Invert gain, no Shift parameter (47) |
| 28 | **F5 designed** (2026-10-15), §7.6, for the owner's approval: the side-chain input is the reference (decision 31); the fit writes only into free slots, the EQ as heard counted (decision 32), through the source's covariance; one Amount, 40 % by default (decision 33); the Match card (decision 34), signed with 33 and 34's alternatives, a Detail control and the card under Resonances; power-mean long-term spectra of both inputs before the EQ, smoothed by the wider of a third of an octave and an ERB, the level taken out after the Amount so the loudness is kept, Ramos & López's greedy fit refined by Levenberg–Marquardt on the actual responses |
| 27 | **F4 designed** (2026-10-15), §7.5, for the owner's approval: the finder proposes static cut bells the player applies, never continuous suppression (decision 27); a long listen to the input that keeps peaks fixed while the music moves (decision 28); one Amount, 20 % by default (decision 29); a second column of cards (decision 30); its design approved by the agent-loop in round 5: the rungs, the five spectra and the placement order made exact, the depths solved by projected Gauss–Seidel on the actual responses, a pair counted as one candidate, Amount's migration, the DOX closeout |
| 26 | **F3 designed** (2026-10-15), §7.4, for the owner's approval: an explicit Freeze holding the last two seconds' maximum, no hover entry (decision 23); peaks by prominence, spacing and count (decision 24); a grab makes a static bell, Q from the peak's width (decision 25); the side-chain spectrum as the reference overlay (decision 26) |
| 25 | **F2 designed** (2026-10-14), §7.3: Pro-Q's five placements and, by the owner's choice, its Split on a Stereo band (L/R, or M/S with Alt) (decision 18); placed bands disabled in mono (decision 19); a placed dynamic band detects its own channel (decision 20); no new presets (decision 21); channel bands before mid/side bands (decision 22); 24 placements appended; curves each a channel's self-response, labelled. Approved by the owner 2026-10-15 |
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
| 18 | **F2's placements are Pro-Q's five**: Stereo, Left, Right, Mid, Side, **and Pro-Q's Split button now**; the proposal had left Split to F8 (§7.3) | **Settled** (owner, 2026-10-15: the alternative) | F2.3 |
| 19 | **In mono, a placed band is disabled** and keeps its placement (Pro-Q's way); the alternative plays it as Stereo (§7.3) | **Settled** (owner, 2026-10-15) | F2.1 |
| 20 | **A placed dynamic band detects the channel it works on**, a Stereo band F1's linked peak; a Left/Right pair then moves independently, Wise's phase intermodulation; the alternative links every band to the stereo peak, so a Left band also reacts to the right (§7.3) | **Settled** (owner, 2026-10-15) | F2.1 |
| 21 | **No new presets for F2**; the proposal was two Side presets, *Mono bass* and *Wider air* (§7.3) | **Settled** (owner, 2026-10-15: the alternative) | — |
| 23 | **Freeze is an explicit toggle**, holding each band's highest level of the last two seconds; no hover entry; the alternative is Pro-Q's hover-and-wait entry (§7.4) | **Settled** (owner, 2026-10-15) | F3.2 |
| 24 | **Peaks**: at least 6 dB of prominence within a third of an octave, within 40 dB of the loudest band, a sixth of an octave apart, at most eight; the alternative is a looser or tighter rule (§7.4) | **Settled** (owner, 2026-10-15) | F3.1 |
| 25 | **A grab makes a static bell**, Q from the peak's 3 dB width; the alternative makes a dynamic band directly (§7.4) | **Settled** (owner, 2026-10-15) | F3.2 |
| 26 | **The reference overlay is the side-chain input's spectrum**; the alternative is a captured snapshot of the EQ's own (§7.4) | **Settled** (owner, 2026-10-15) | F3.3 |
| 27 | **F4 proposes static cut bells and the player applies them**: the EQ's own bands, never continuous per-bin suppression; the alternative proposes dynamic bands (§7.1) directly (§7.5) | **Settled** (owner, 2026-10-15) | F4.2 |
| 28 | **The finder listens for peaks that stay put**: a long, high-resolution listen to the input that keeps a peak only if it holds its frequency through most of what was heard; the alternative takes F3's frozen peaks as the candidates (§7.5) | **Settled** (owner, 2026-10-15) | F4.1 |
| 29 | **One Amount, 20 % by default**: each cut takes that share of its peak's prominence, at most 12 dB; the alternative starts at 50 % (§7.5) | **Settled** (owner, 2026-10-15) | F4.2 |
| 31 | **Each EQ learns its own track, and is matched to another's**, as every maker's match works (Pro-Q's instances, Ozone's snapshots, §10.1): one toggle, **Learn**, learning this track's input before the EQ and its output after it anew while on, and off keeping them with the project; **Match to** chooses another EQ that has learned, by its track's name (§7.8), or a saved reference; the alternative learned the reference and the track in one window, from this track, the side chain or another track while that one played (§7.6) | **Settled** (owner, 2026-10-28: *"yes, rebuild it that way"*) | F5.9 |
| 32 | **The fit writes only into free slots, the EQ as heard counted**, so the player's bands never change; the alternative replaces the EQ, every band refitted (§7.6) | **Settled** (owner, 2026-10-15) | F5.1 |
| 33 | **One Amount, 40 % by default, and Pro-Q's detail control, a band count the player sets**; the proposal had no detail control, the fit stopping only where what is left is just audible or the slots run out (§7.6) | **Settled** (owner, 2026-10-15: the alternative) | F5.2 |
| 34 | **The Match card goes under the Band card** in the first column, which is then the taller, the window opening hugged to it (§7.6) | **Settled** (owner, 2026-10-28: *"could the match card not be under the band card?"*) | F5.8 |
| 35 | **Two phase modes, Zero latency and Linear phase**; the alternative adds Pro-Q's Natural phase, the analogue phase at a small latency, which Zero latency already follows below the top octave (§7.7) | **Settled** (owner, 2026-10-15) | F6.1 |
| 36 | **Three resolutions for Linear phase, Low, Medium and High, Medium by default** (171 ms kernel, 91 ms latency at 48 kHz); the alternative is one fixed resolution, Medium (§7.7) | **Settled** (owner, 2026-10-15) | F6.1 |
| 37 | **Dynamic bands are held at their drawn gain in Linear phase**, coming back in Zero latency; the alternative lets them move through the kernel, as Pro-Q allows up to High: a new kernel every 21 ms, their detector on the input delayed by the latency so it hears what the output plays, the gain then acting at least one update interval later still (a bank designed, taken at a boundary and faded in) and an Attack under 21 ms acting as 21 ms, at the cost of a kernel design every interval while any band moves (§7.7) | **Settled** (owner, 2026-10-15) | F6.1 |
| 38 | **Auto gain estimates the loudness change from the curve**, as Pro-Q does: no latency, no pumping, a bounce the same each time; the alternative measures the input's and output's loudness (BS.1770 over a few seconds) and matches them, following the music but moving with it, and wrong for seconds after a change (§7.8) | **Settled** (owner, 2026-10-16) | F7.1 |
| 39 | **The estimate weighs the curve by the corpus mean mix** (research §10.6) under K-weighting, 5 % of the weight above 4 kHz; the alternative weighs it by pink noise, the usual test signal, 43 % above 4 kHz, so treble boosts are made up about five times as much (§7.8) | **Settled** (owner, 2026-10-16) | F7.1 |
| 40 | **Auto gain is a parameter, off by default**, so Init, the factory set and older states keep their level; the alternative is on by default for a new instance (§7.8) | **Settled** (owner, 2026-10-16) | F7.1 |
| 41 | **Instances meet in one process**, through a registry; under Bitwig's *Individually* hosting none are listed; the alternative shares spectra across processes through shared memory, a platform layer per system (§7.8) | **Settled** (owner, 2026-10-16) | F7.3 |
| 42 | **Tracks are named by the host's track names, through a small addition to mxm-kit's `mxm-editor` pushed to its public `main`** (signing approves the push); the alternative leaves the kit alone and numbers the instances, "Track 1", "Track 2" (§7.8) | **Settled** (owner, 2026-10-16) | F7.2 |
| 43 | **One external spectrum, the side chain or one other track, and a Collisions switch** marking, hatched, where the two may collide; the alternative is Pro-Q 4's Instance List, every instance's spectrum and curve in one view, each editable from any (§7.8) | **Settled** (owner, 2026-10-16) | F7.3 |
| 44 | **The selection by Shift-click (adding or taking out one band) and a rectangle dragged on the background**, SELECT with the arrows and Ctrl/Cmd+A, the primary band the one the card shows and solo plays; the alternative is Pro-Q's own: Ctrl/Cmd-click adds or takes out one band and Shift-click selects a consecutive range, which takes the modifier this EQ holds for solo on a point (§5.2), so solo would move to another (§7.9) | **Settled** (owner, 2026-10-16) | F8.1 |
| 45 | **A group moves by one ratio in frequency, one offset in gain, one factor in Q, and stops at its first member's limit**, keeping the curve's shape; the alternative is Pro-Q's gain scaling, each gain multiplied by the dragged band's ratio, so 0 dB bands never move and cuts and boosts spread apart (§7.9) | **Settled** (owner, 2026-10-16) | F8.1 |
| 46 | **Copy and paste through a clipboard in the process**, shared by every instance as the registry is, pasting into free slots and never over a band; the alternative uses the system clipboard as text, which also reaches other processes and programs, through each system's clipboard in the window library (§7.9) | **Settled** (owner, 2026-10-16) | F8.2 |
| 48 | **What an EQ learned is saved by name as a reference**, a file per reference in a folder beside the EQ's user presets, listed in Match to and chosen for any track in any project, the choice saved with the project; the alternative keeps a reference only while the editor is open and keeps a match by saving the applied bands as an ordinary preset, which matches only material like the track it was fitted on (§7.6) | **Settled** (owner, 2026-10-28: the reference saved) | F5.6 |
| 49 | **No fixed time; an unsure difference is shrunk by how unsure it is**: a learn read in two halves, a second of sound each in turn, whose agreement after smoothing is its spread; the difference's shape about its mean shrunk by the track's and the reference's spreads together, `s·s²/(s² + u²)`, so a difference well over its spread is kept and one within it mostly taken back, and nothing is asked where nothing is known yet; the alternatives waited 10 s of each for every frequency, or left out every frequency less sure than 0.5 to 1 dB, which left a voice's almost all out (§7.6) | **Settled** (owner, 2026-10-28: *"Yes drop the 10 secs"*; *"that seems like too little"*) | F5.10, F5.11 |
| 50 | **Preview plays the proposals before they are applied**, following Amount and Detail, as a transient command as band solo is (§5.2): no parameter written, never in a render, only with an editor; the alternative applied, listened and undid (§7.6) | **Settled** (owner, 2026-10-28: *"Yes"*) | F5.10 |
| 47 | **Global transforms come from the selection**: Select all, then drag or the arrows, and Invert gain on the menu; no Shift parameter; the alternative adds Bitwig's and DMG's **Shift**, an automatable parameter moving every band by one ratio in the DSP, a host able to ride it (§7.9) | **Settled** (owner, 2026-10-16) | F8.1 |
| 30 | **A second column of cards**: the Band card alone in the first, Output, Analyser and a new Resonances card in the second, the window opening wider and hugged to the taller; the alternative keeps one column, which then scrolls at the opening size (§7.5) | **Settled** (owner, 2026-10-15) | F4.2 |
| 22 | **Channel bands before mid/side bands**, so slot order never matters; the alternative allows one family per instance, Mid and Side unavailable while a Left or Right band exists (§7.3) | **Settled** (owner, 2026-10-15) | F2.1 |
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
list poorly, and an id must be permanent from the release that declares it. **Until the first
release the parameters are not permanent** (the owner, 2026-10-16: "relax it for pre-alpha"): an
id, the order, a stepped domain or a value's place may change when the design calls for it, and an
older state or preset need not load as it was.

**The cap is 24 slots**, Pro-Q's (decision 1, owner, 2026-10-12; research §17.1).

### 2.2 Signal path

```
in ──► band 1 ─► band 2 ─► … ─► band N ─► output gain ──► out
        (each band's gain × the gain scale)
         (series cascade of the slots in use, in slot order; dB responses add)
```

- **Series, in dB.** Bands compose by adding their dB responses, so the drawn curve is the sum of the
  drawn bands (research §3.6). Slot order is irrelevant to the linear result (with F2's placements,
  through §7.3's fixed family order).
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
| `vendor/nice-plug` | Nothing. F6 reports a latency at activation, as mxm-fx-oldify does, and a change from `process`, which nice-plug turns into the host's restart; MXM Player's headless path does not restart, so there a change waits for the next activation (§7.7) |
| Root `AGENTS.md` | F6: the DSP crate's MSRV row loses *zero runtime dependencies*: its one is mxm-kit's `mxm-fft` (decision 6) |
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
| F4 | **Resonance finder**, writing ordinary cut bands. Designed in §7.5 | Research §8–§9: prominence, persistence and decay tests; professionals removed about a fifth of the excess; a cut does not shorten a ring | Decisions 27–30. Continuous spectral suppression must first clear oeksound's active patent, US 10,587,238 (research §9.6): a legal and product decision, outside F4 |
| F5 | **EQ match to a reference**: gated, level-aligned long-term ratio, smoothed, applied as an amount, fitted to bands. Designed in §7.6 | Research §10: 15–20 fitted bands were indistinguishable from a 2,048-tap FIR; makers advise under 50 % | Decisions 31–34 |
| F6 | **Phase modes**: linear, perhaps analogue-matched. Designed in §7.7 | Research §5: 100–400 ms pre-ring on narrow low cuts | Decisions 35–37 |
| F7 | **Auto gain and other tracks' spectra**, the owner's two of four candidates (character or saturation and band modulation set aside, 2026-10-16). Designed in §7.8 | Research §12, §19; FabFilter's Pro-Q 4 help; Bitwig's user guide | Decisions 38–43 |
| F8 | **Workflow**: multi-band selection with relative editing, copy and paste of bands, global transforms (scale all gains, shift all frequencies). Designed in §7.9 | Research §17.2–§17.4: established across the surveyed products; FabFilter's Pro-Q 4 help | Decisions 44–47 |

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
  nice-plug's id is the layout's index, and the EQ has never been released (pre-alpha, no
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

### 7.3 F2 — per-band stereo placement (revision 25, for the owner's approval)

The next phase in the owner's order (decision 9), after the side chain. **Pro-Q's five placements**
(research §6 — standard per band in Pro-Q, Kirchhoff-EQ, F6, Frequency 2 and DMG):
**Stereo**, the default and every band today, **Left**, **Right**, **Mid** and **Side**.

**What a placed band does.** With `M = (L + R)/2` and `S = (L − R)/2` (research §6): a Left band
filters the left channel only, a Right band the right; a Mid band filters `M` and a Side band `S`,
each then decoded, `L′ = M′ + S′`, `R′ = M′ − S′`. A placed band runs one channel of its filter, so it
costs no more than a stereo one. **With every band on Stereo the output is bit-exact what it is
today.**

**Slot order still never matters** (§2.2), because the order is fixed by family (decision 22): audio
runs in series through **every Stereo, Left and Right band, then every Mid and Side band**. Within a
family the bands commute — a Stereo band is one filter on both channels and commutes with any band,
Left and Right bands each touch one channel, Mid and Side bands one of `M` and `S` — so only a
channel band and a mid/side band fail to, and their order is the fixed one: fix a channel, then
shape the image. Moving gains commute only as closely as F1 measured (§7.1).

**The image moves, as in Pro-Q** (research §6, derived): any phase in a channel-only minimum-phase
filter puts sound where the other channel had none — a 24 dB/oct high-pass at 100 Hz on Mid sends a
hard-left 140 Hz source to the right 4.4 dB louder than the left. Pro-Q allows it and advises linear
phase, which is F6 here. F2 allows it too; the placement's hover says a Mid or Side cut moves the
image near it. No warning is drawn on the panel (the workspace rule).

**A placement change is click-free**: placement joins the band's structure with its shape and slope.
**Within a family** (Left to Right, Mid to Side, Stereo to Left or Right) the change crossfades the old
band into the new, as a shape change does (§3.3). **Across the family boundary** (Left to Mid, Side to
Stereo) the old and new bands belong at different places in the order, so the change is a delete and
a create at once: the old band fades out to identity at its old family's place while the new one
fades in from identity at its own, each over the fade a band's removal and creation already take.
**Each arm runs in its own family's stage, not where the band was**: the cascade is two stages, the
channel stage (every Stereo, Left and Right arm) then the mid/side stage (every Mid and Side arm). A
fading arm is still a band of its family — its filter mixed with identity on the same channels — so
it commutes within its stage, and the order holds at every sample, however many bands cross at once
and in either direction (two bands swapping families put both old arms and both new arms each in
their own stage). Each band keeps the two filter states it has today, and the cost is the two
filters a crossfade already runs.

**Dynamic bands** (decision 20, the F2 row's open question, settled by the owner): **a placed band
detects the channel it works on** — a Left band its left, a Mid band `M` — so a band placed on one
channel answers that channel. A Stereo band keeps F1's linked peak; detectors stay in parallel on the
input; a keyed band reads the same channel of the key (§7.2), so a Side band on a mono key hears
silence. **The consequence, for the owner:** a Left/Right pair of dynamic bands then moves
independently, which is the per-channel dynamic filtering Wise found causes "phase intermodulation"
(research §6–§7), a moving, "space-y" image; a pair that must move together is a Stereo band. The
alternative links every band to the stereo peak: a pair moves as one, but a Left band then also
reacts to the right channel.

**Solo plays what the band works on, where it sits**: a Left band's region in the left, a Mid band's
in the centre, a Side band's as side (`L = S`, `R = −S`). Every other rule of solo is §5's and §7.2's.

**Mono** (decision 19, Pro-Q's way): in a mono layout a Stereo band shows no Placement control, and a
band placed Left, Right, Mid or Side — from a preset or a stereo instance's state — is shown disabled
and does nothing, its placement kept. **Its Band card shows the Placement control with one choice
enabled, Stereo**, the way back (Pro-Q resets it from the band's menu); the others are shown, not
offered, and the hover says why. Choosing Stereo is an ordinary edit, undoable.

**Parameters.** One per slot, appended after the side-chain switches (290 in all): `place_<n>`,
"Band n placement", stepped **0–7**, reserved now as `type` and `slope` are: 0 Stereo, 1 Left,
2 Right, 3 Mid, 4 Side, 5–7 unassigned (read *Unavailable n*, treated as Stereo). An older preset
or host state restores every band to Stereo (`default_missing_legacy_parameter`; `filter_state`
writing the default for a missing id), a loaded preset still clean. **A slot field**: a new band is
Stereo, a copy keeps its original's placement (`slot_writes`, twelve fields).

**The editor.**
- A **Placement** selector in the Band card for every shape, its hover saying what each choice does.
  The opening is at 1204 × 1048 now; a row more would pass 1080, so the selector shares a row, with
  Bypass (two selectors do not fit Shape's). The tree checks and the opening size are re-measured
  (§8).
- **The graph groups the curves by the channels they act on**, Pro-Q's way: with every band on
  Stereo it is today's one curve. With placed bands, the EQ is a 2 × 2 response: **each curve is one
  channel's response to itself** — Left and Right in the left/right basis when a Left or Right band
  exists, Mid and Side in the mid/side basis when a Mid or Side band does — the diagonal of the
  product of the DSP's own complex responses, in the order above (§3.1's rule, with placements). With
  one family in use the off-diagonal is zero, so each curve is exactly what its channel hears; with
  both, what moves between channels is not drawn. **Identity never rests on hue** (§5): each curve
  is labelled with its channel's letter at its end, and a placed band's point carries the same
  letter beside its number.
- **Split** (decision 18, Pro-Q's button): in the Band card's action row beside Solo and Delete
  band, in a stereo layout only, and **on a Stereo band only** — the one placement whose filter on
  both channels is the same filter on `M` and on `S`, so either pair keeps its sound; on a placed
  band it is shown disabled, its hover saying the band is already on one channel. A click turns the
  band into a Left/Right pair, an Alt-click into a Mid/Side pair (the hover says so): the band
  becomes Left (or Mid) and its copy Right (or Side). The copy takes
  the lowest Off slot and every other field of the original — shape, frequency, gain, Q, slope,
  bypass, dynamics and side-chain switch (`slot_writes`) — and the pair is one undo step, the
  original still selected. With every slot in use Split does nothing and its hover says why. The
  original's change is a placement change (click-free, above), the copy fades in as a new band does.
  **A static band splits into the sound it made**; a dynamic one splits into two bands that each
  listen to their own channel (decision 20) — that is what splitting a dynamic band is for. A
  Left/Right pair moves as one on equal channels and apart on unequal ones; a Mid/Side pair's Mid
  copy follows the mid and its Side copy the side, so on equal channels (no side) only the Mid copy
  moves.
  It is a button, not a new job in the keyboard language: the cursor reaches it as a column control.

**No new factory presets** (decision 21): the thirteen and F1's two stay Stereo.

**Tests** (written at the end): every band on Stereo is bit-exact today's output, NaN and silence
included; each placement filters only its channels (a Left band leaves the right bit-exact; a Mid
band leaves a side-only input bit-exact, and a Side band a mid-only one); a placement change within
a family and one across it step nothing, two bands crossing in opposite directions at once included; a placed dynamic band detects its channel (a loud left with a quiet right moves a Left band,
not a Right one); a Side band keyed from a mono key hears silence; solo of each placement; mono
disables placed bands and keeps their values, and its Stereo choice restores one; the family order
(a Left band and a Mid band give the same output whichever slot holds which); the 290 ids, the
reserved places, the migrations, the slot lifecycle; the selector's tree, keyboard and hover; Split — a static Stereo band split into a Left/Right pair renders
what it did, bit for bit, once both have settled, and into a Mid/Side pair within rounding (the
reference is the Stereo band before the split); Split disabled on a placed band; a dynamic Left/Right
pair on equal channels as one, on a louder left with the left band moving further; a dynamic
Mid/Side pair on equal channels with only the Mid copy moving; every other field
copied, one undo step, nothing done with every slot in use, absent in mono; each
curve against its channel's rendered self-response, one family and both; no allocation with every placement changing; the cost with 24 Mid bells. Through
MXM Player: every host test, and a placed band restored through the state heard on its channel.

**Delivery** (tests at the end, one review over F2.1–F2.3):

| Step | Delivers | Gate |
|---|---|---|
| **F2.0** | Decisions 18–22, signed. **Done** (2026-10-15) | Owner |
| **F2.1** | DSP: placement in the band's structure, each channel's path, detection, solo, mono. **Built** (2026-10-15): 14 placement tests; steady 19.2 → 20.5 % at 192 kHz, 24 mid bells ≈ 10 % | DSP tests at the end |
| **F2.2** | Shell: 24 placements, both migrations, host tests. **Built** | Unit and host tests at the end; `clap-validator` |
| **F2.3** | Editor: the selector, Split, the grouped curves and their letters, the point's letter. **Built**; review approved in round 3 | Editor tests at the end; one agent-loop review of F2.1–F2.3 |
| **F2.4** | Listening, and the image checked by ear on speakers and headphones | Owner sign-off |

### 7.4 F3 — spectrum grab and analyser extras (revision 26, for the owner's approval)

The next phase in the owner's order (decision 9; the owner, 2026-10-15, *"yes to all"*). Pro-Q's
Spectrum Grab is the reference (research §11, its manuals): the spectrum freezes, "the most
important peaks" are labelled with their frequency, and dragging one makes a bell whose Q "is
determined automatically", the drag setting its gain. FabFilter does not disclose what counts as
a peak, how they are ranked, whether the frozen spectrum is an instant or a hold, or how Q is set;
F3 states its own, below, each one a test.

**Freeze, explicit** (decision 23). A **Freeze** toggle in the Analyser card holds the display.
There is no hover entry: Pro-Q's freezes after "a few seconds" over the spectrum, a timer the
player cannot see and the keyboard cannot reach. Freezing holds **each band's highest level of the
last two seconds** — Kirchhoff-EQ accumulates frames before it picks — so a peak that comes and goes
with the notes is there to grab; an instant would catch whatever the last frame held. The hold is
the display's own smoothed levels (1/12 octave, §4), so what is labelled is what is drawn.
**A freeze keeps one source and one representation**: the spectrum grabbed is fixed when Freeze is
turned on — the post-EQ one when it is shown, else the pre-EQ one (Pro-Q's rule: post "works best"
because it matches what is heard) — and the hold, its labels and every grab use it alone. **Any
change to Show, Resolution, Speed, Tilt or the Side chain view ends the freeze** (the toggle turns
off and the display runs again), as do a reset (seen with the first audio after it) and a change of
rate, so held levels are never shown as another source or under another transform; the two-second
hold starts again on the same changes, so it never mixes them either. The side-chain line holds
with the rest. A width change ends it too, since it moves the graph's frequency grid and the held
lines were read on the old one; the peaks (frequency, level, Q) stay true, so a grab or ADD in that
frame makes the same bell, and a height change, which moves no frequency, leaves it standing. **Freeze is the editor's moment, not a setting**: unlike the analyser's
view state (§4) it is not saved with the project; turning the analyser off, closing the editor or a
restored state or preset ends it, and its hold goes with it, so a reopened editor always shows the
live spectrum.

**Peaks** (decision 24), from the frozen spectrum alone:
- a **local maximum standing at least 6 dB above the higher of the two lowest points between it and
  the next higher level within a third of an octave each side** (its prominence, the usual
  topographic one), so the spectrum falls at least 6 dB on both sides and both 3 dB points exist; a
  maximum whose third-octave reaches the display's edge without that fall is not a peak;
- **within 40 dB of the frozen spectrum's loudest band, and inside the range the graph draws**, so
  structure near the analyser's floor (−120 dB) never displaces audible peaks;
- **at least a sixth of an octave from a stronger one**, the stronger kept;
- **at most eight**, ranked by prominence, each labelled with its frequency in the graph's own
  reading (`1.20 kHz`), a label that would overlap a stronger one's not drawn.
**What the rule sees**: a peak must fall 6 dB within a third of an octave each side, so the labels
go to resonances narrower than about 0.4 octave at their 3 dB points (Q above about 3.7: a
resonance falls 6 dB at a detune of √3);
a broad bump is shaped with a band by hand. The numbers are this plan's: Sir Audio StandardEQ is the one product that exposes them (a maximum
count, a minimum spacing, a range), and F4's resonance finder will share this list (research §11:
"the grab gesture and the resonance detector can share one candidate list").

**What a grab makes** (decision 25): dragging a labelled peak makes **a static bell** — Pro-Q's
rule; it can be made dynamic afterwards (§7.1) — at the peak's frequency, with **Q from the peak's
width**: `Q = f0 / (f_high − f_low)` between the first points 3 dB below it on each side of the
frozen spectrum (they exist by the peak rule), held inside 0.5–12, and its gain following the vertical drag from 0 dB (down cuts, up lifts) as any band's does,
through the gain scale (at 0 %, where every bell is drawn and heard flat, the drag sets none). It is one
gesture and one undo step, BACK takes it away, and it takes the lowest Off slot like any new band;
with every slot in use no peak is grabbable and the hover says why. A click on a label without a
drag makes nothing. A band's point under the pointer comes first, before a peak's marker or label:
a freeze never hides a band from the pointer. **The keyboard** reaches it in the graph's own language: while frozen, with no
band selected, ← → walk the marked peaks instead of the line (the focused one's label always
drawn), and ADD on one makes its bell, then
VALUE sets its gain as for any band.

**The reference overlay** (decision 26): **the side-chain input's spectrum** (§7.2), a third line
drawn dashed and labelled "SC", never by hue alone, while the Analyser card's **Side chain** view is
on. The plugin can observe whether its layout has the port — the shell publishes it with the channel
count at activation — but not whether a host has routed anything to it: an unconnected port and a
silent source are the same silence. So **the toggle is offered only on a layout with the port**, and
the line shows what the port carries, at the display's floor when that is nothing; its hover says
that with nothing routed there the line stays flat. It is analysed as the input is — the audio thread copies the
key with the pre and post blocks, preallocated, nothing with no editor — so a kick's spectrum can be
seen against the bass it keys. Another instance's spectrum is F7's.

**Not in F3**: peak grabbing from the side chain (not documented anywhere; research §11), note
names (no piano display here), a grab that makes a dynamic band directly.

**Tests** (written at the end): freezing holds the two-second maximum and nothing moves while
frozen, and a change of any analyser setting ends it, as do the analyser off, the editor closed and a restore, nothing of it saved; the peak rules on synthetic spectra — prominence on an asymmetric peak and a shoulder (not a
peak), a maximum at the display's edge, a prominent bump 50 dB under the loudest band (not a peak), spacing, the count, the ranking, a label dropped for overlap; Q from width on a known peak, and its bounds; a grab makes the bell at the
peak's frequency and Q, its gain the drag's, one undo step, BACK leaving nothing, the lowest Off slot,
nothing with every slot in use; the keyboard walk and ADD; the Side chain toggle only on a layout with the port, its
line at the floor on a silent key, the copy realtime-safe and absent with no editor; the analyser's cost with the side-chain line,
inside its 2 ms budget (§4); the opening size with the new toggles.

**Delivery** (tests at the end, one review over F3.1–F3.3):

| Step | Delivers | Gate |
|---|---|---|
| **F3.0** | Decisions 23–26, signed. **Done** (2026-10-15) | Owner |
| **F3.1** | The hold, the peak picker and Q from width, in the analyser's editor half. **Built** (2026-10-15): `editor/grab.rs`, 7 tests | Unit tests at the end |
| **F3.2** | Grab, mouse and keyboard; Freeze and Side chain in the Analyser card. **Built** (2026-10-15) | Editor tests at the end |
| **F3.3** | The side-chain line: the audio thread's copy and its analysis. **Built** (2026-10-15) | Realtime and cost tests at the end; one agent-loop review of F3.1–F3.3: **approved** in round 7 (2026-10-15) |
| **F3.4** | The owner by eye and ear | Owner sign-off |

### 7.5 F4 — the resonance finder (revision 27, for the owner's approval)

The next phase in the owner's order (decision 9). Cutting a peak is solved: the bell's half-gain Q
makes an exact inverse (research §9.2). Deciding **what is a resonance** and **how much to take**
is not. A resonance is a peak at a **fixed frequency** weighting every note that reaches it, where a
note's harmonics move with the note (§8.2); the published mix detectors are magnitude-only and
short-window (§8.5); in mix EQ the expensive error is a false alarm, a wanted harmonic cut, so the
criteria are conjunctive and the cuts shallow (§8.3); and professionals removed 6–47 % of each
peak's excess, median 19 % (Grachten 2019, §8.5).

**What F4 writes** (decision 27): the finder **proposes cut bells and the player applies them**;
applied, they are ordinary static bands, editable, deletable and able to be made dynamic (§7.1). It
never applies a curve itself and never moves while the music plays: the documented route of static
notches at measured peaks (Corbach 2009; feedback suppressors, §9.1), clear of what oeksound's
patents claim, a per-bin curve over a detrended spectrum made minimum phase (US 10,587,238 and two
later families, §9.6). That is a product reading of the claims as the research records them, not
legal advice; continuous suppression stays out of this plan unless the owner has it cleared.

**Find** (decision 28), a toggle that **listens to the input before the EQ**, so the player's own
boosts are never taken for resonances and editing bands does not disturb it. Like Freeze it is
offered only with the analyser on; turning one of Find and Freeze on turns the other off; the
analyser off turns Find off, and back on leaves it off. The listen runs from when Find is turned on.
**Turned off, Find holds what it learned**, as a noise reduction holds the profile it learned: it
stops listening and keeps its proposals, which still answer the EQ and Amount, until it is turned
on again, which learns anew, or Freeze, Match, the analyser off or the editor closed ends it. What
it learns from is just the audio played to it (the owner, 2026-10-27: "Just the audio you play to
it"), so what it proposes is what that audio showed, however the session goes on. A reset breaks its frames, as a dropped block does, and keeps what it heard, since a host resets
whenever it resumes processing: Resolve, played on through a voice-over of 22 clips, found
resonances and lost them again as it went (the owner, 2026-10-27). A reset into other channels
starts the listen again.
- **Two kinds of evidence** (§8.6), on three rungs, each a frequency's only home, so no peak is found
  twice: **width** from 8,192-point frames (about 4/B seconds for a bandwidth B: Q 32 from about
  50 Hz up) and **movement** from frames about the shortest that tells notes a semitone apart in
  each octave (67/f seconds): 2,048 points, and 1,024 in the lowest rung's top octave. Hann windows,
  a hop of half a frame. At 48 kHz:

  | Rung | Rate | Its frequencies | Width frames | Movement frames |
  |---|---|---|---|---|
  | 0 | the input, decimated first by the smallest power of two that brings it to at most 64 kHz (1 at 44.1–48 kHz, 2 at 88.2–96, 4 at 176.4–192) | 1.6 kHz and up | 0.17 s | 43 ms |
  | 1 | a quarter of rung 0's | 400 Hz up to 1.6 kHz | 0.68 s | 0.17 s |
  | 2 | a sixteenth | below 400 Hz | 2.7 s | 0.68 s below 200 Hz, 0.34 s above |

  The ranges are half-open and continuous to as high as a band runs. What a rung reads and what it
  owns differ: an undecimated rung reads to Nyquist, a decimated one to its anti-alias corner, and
  a rung owns no frequency without a third of an octave read above it (a key point's reach), so a
  slower rung hands the third below its top to the faster one; the edges above hold wherever
  every rung reaches them (at 44.1 kHz and up). Where rung 0 is decimated (above 64 kHz), a rung
  at the full rate goes above it and owns the top octaves. Each decimation is the display's
  anti-alias low pass (§4), run on left and right, mid and side formed after. A rung's spectrum runs past its own frequencies, so a peak near a boundary is
  measured whole. Below about 200 Hz a semitone takes longer than a short note, so a fast bass line
  can look fixed there; Find's hover says so.
- **Only heard frames count**: a frame whose input, both channels' power averaged, is at least
  −60 dBFS. Quiet frames enter nothing, long-term spectrum included. Proposals appear after four
  seconds of heard input and update about twice a second.
- **Five spectra**: left, right, mid (`(L + R)/2`), side (`(L − R)/2`) and the channels' power
  average; a mono input has the one. **Candidates** are the peaks of each spectrum's **long-term
  spectrum** by F3's rule (decision 24) with the leakage bins either side skipped (§8.3); the five
  lists are merged within half a semitone, the most prominent standing for the others. The
  long-term spectrum is every heard width frame's level **averaged in dB, each bin clipped 3 dB
  over its running mean**, so a note's harmonic, there for less than half the frames, stands within
  a dB or two of the floor while a resonance, which weights every note, keeps its height; so a
  resonance is measured where broadband sound reaches it (a mean of power lets the lines tower over
  the bump). Each candidate is **measured by the cookbook bell fitted to it**, least squares on a
  sloping floor over a third of an octave either side, its residuals Huber-weighted: its centre,
  its height P and its Q, which is the width at P/2 (one-point measures follow whichever line rides
  highest on the bump). A candidate needs three heard width frames in its rung, so the clip is in
  force (the lowest rung's first candidates come about 5.5 s into a listen), and a fitted width of
  at least four of their bins: anything narrower reports the frame's window, not the peak (§8.6),
  and is a line or noise. **Nor a fit wider at half its gain than the third either side it reads**
  (Q under about 2.1): there the height is guessed, not measured, traded for width, and on a
  voice's formants and sibilance under the ripple of its harmonics a 6 dB bump fitted at +36 dB
  and Q 0.2, proposing a cut of 12 dB over 1–8 kHz (the owner's voice-over in Resolve,
  2026-10-27). Such a fit does not hold and the candidate keeps its first measure; on a swept
  test of broad and narrow bumps under ±0.5–3 dB of ripple, it left no fit over twice the true
  height (101 before) and kept 97.5 % of the narrow ones (Q 3 and up), within 0.5 dB.
- **Placed where the excess is**, with F2's placements (§7.3), the first rule that holds: the peak
  in one of left and right only, **Left** or **Right**; in both, their P more than 3 dB apart, a
  **Left and a Right** band, each its own depth; in the side and not the mid, **Side**; in the mid
  and not the side, where the side's level there is within 20 dB of the mid's, **Mid**; otherwise
  **Stereo**, on the power average. (A one-channel resonance shows in mid and side alike, and a side
  one in left and right alike, so the order separates them.) What follows is judged on the
  placement's own spectrum, each channel's for a pair; only a peak that persists there counts as
  that spectrum's.
- **Persistence decides**, on the movement frames. A candidate is present, within half a semitone,
  in at least half of the frames that could show it — those whose third of an octave around it is
  within 40 dB of their loudest third, at least sixteen of them, so a passage that never reaches
  the region does not vote it away and noise does not pass by chance. Present means F3's 6 dB of prominence taken over the **median** of
  its surroundings, a twelfth to a third of an octave away, on 1/48-octave cells: over their lowest
  point, one frame's noise has a 6 dB dip beside anything. Near the edge of what a rung reads (the
  top, an anti-alias corner, 20 Hz) the surroundings are what is read, so long as one whole side
  is, so the top octave and both sides of a rung's edge are read too. **And those frames show at least two
  excitations**: what plays elsewhere in the rung, its loudest outside that third, the commonest —
  every one less than a semitone from it counted with it — at most two thirds of them (the strongest other peak inside the third is none wherever
  broadband sound excites a resonance). A region only one note reaches gives no verdict and nothing
  is proposed there. Moving harmonics fail; a room, body or cabinet
  resonance passes; a drone, a held note and a reflection's comb persist too (§8.2), so the player
  decides each proposal.
- **Ranked by audibility** (§8.8): how far P exceeds the deviation just audible on music, 1.5·Q^0.3
  dB (Toole & Olive: about 1.5 dB at Q 1, 3 at Q 10, 5 at Q 50). **At most eight candidates**, a
  sixth of an octave apart; a pair is one candidate with two bands at one centre. In rank order a
  candidate is proposed only if its bands fit the free slots still left; one that does not is passed
  over.
- **What was found stays a while.** A candidate a search no longer finds is kept, as last measured,
  until 10 s of heard input pass without it; one found within a sixth of an octave of it is it,
  measured again. Each rule above has an edge a resonance can stand at — present in about half its
  frames, a prominence about 6 dB, between two placements or two neighbouring peaks — and such a
  one came and went with the updates as the listen went on: over the owner's voice-over
  (2026-10-28: "It gets and looses ressonance frequencies over time") 1.5 kHz was lost four times
  in a minute, the last at its end. Held, that minute lost two proposals, each ten seconds after it
  was last found, and ended with five.

**How much** (decision 29). One **Amount**, 0–100 %, **20 % by default** (Grachten's median): each
proposal's target is **−Amount × P**, never deeper than **−12 dB** (Ramos & López, §8.4), and its
**Q is the width at P/2**, held inside **0.5–20** (past Corbach's Q 20 a cut's own ringing and
mistuning cost more than the resonance, Fazenda, §8.4). For one isolated peak, inside those bounds,
where the EQ does nothing, that bell at 100 % is the peak's exact inverse in the band's half-gain
convention (§3.2). **Amount is view state**, in whole percent, saved with the analyser's settings
(§4): a project from before loads it at 20 % (the settings' own default), a change marks the project
dirty, and it is not part of a freeze's basis (§7.4). Find, what it heard and its proposals are the
editor's moment, never saved.
- **What is left decides.** What the EQ as heard already takes off a candidate is the bell fitted
  to its spectrum less the bell fitted, from the same start, to that spectrum plus the EQ there —
  the graph's self-response for that channel (§7.3), gain scale, bypass and placement included, a
  dynamic band at its drawn gain. Where the EQ already takes Amount × P off to within 0.5 dB, the
  candidate is **treated** and not proposed; otherwise the remainder is proposed as
  a further band. A shown proposal goes only once under 0.25 dB, so updates do not flicker. After an
  Apply, a bypass or a larger Amount, Find shows what is left.
- **Depths are solved together**, since a neighbour's skirt cuts into a centre. **What is solved**:
  at each candidate's centre, its spectrum's response — the self-response the graph draws for that
  channel (§7.3), and for Stereo the power average of the left's and the right's — of the EQ as heard
  plus every proposal, of every placement, should stand at the EQ's own plus the target. What crosses
  between channels is not modelled, as the graph does not draw it. **How**: each depth starts at its
  target, then 32 sweeps of projected Gauss–Seidel in rank order: each depth moves by its centre's
  remaining error, on the actual responses, and is held inside −12…0 dB (cut only). It converges
  wherever each bell is deepest at its own centre, as a cut bell is, and it is bounded and
  deterministic everywhere: where the cap or a Q bound puts targets out of reach together, the
  sweeps end where they end and those centres stay short (Ramos & López also refine iteratively).
  Each proposal shows the depth Apply all gives it.

A proposal never cuts deeper than a band can be heard to under the gain scale (its ±30 dB times
the scale), so the depth shown is the depth applied. A Left and Right pair stands at one centre,
midway between its two fits.

**On the graph**, while Find has proposals: **the curve they would make**, dashed over the EQ's, and
a **downward triangle** at each candidate labelled with frequency and depth (`320 Hz −2.1 dB`, a
pair's two depths), told apart by shape and text, never hue; labels overlap by F3's rule. **A click
on a triangle applies it; a drag applies it with the gain following the drag** (F3's grab; a pair's
two gains move together). Either is one undo step, BACK during the drag leaves nothing, and a band's
point under the pointer comes first. **The keyboard**, with Find on and no band selected: ← → walk
the proposals, the focused one's label always drawn, and ADD applies it. **Every path writes a band's
every field** (`slot_writes`): a static, unkeyed bell at its placement, whatever an Off slot held.

**The Resonances card**: **Find**, **Amount** and **Apply all**, which writes every proposal shown
into the lowest free slots as one undo step, disabled with none. With every slot in use nothing is
proposed and Find's hover says why. Hover only, no help text.

**A second column** (decision 30). The column stands at 1,048 of the 1,080 px budget, so a fourth
card cannot go under the others, and the owner allowed the window to grow sideways (2026-10-15,
*"you can also expand sideways"*). The cards become **two columns** right of the graph: **Band**
in the first, **Output, Analyser and Resonances** in the second; Match went under Band (decision 34). The window opens 328 px
wider, inside the 1,920 px budget, hugged to the taller column. §5.1's rules hold for the pair:
never under the graph, the graph shrinking to its floor first, then the surface scrolling sideways;
the columns scroll vertically together.

**A listen ends** with Find off, the analyser off, Freeze on or the editor closed; a reset or a new
rate starts it again, as the display's windows restart (§4), and what an earlier listen found is
never shown in a later one. **The search runs on a worker thread**, on a copy of what was heard,
one at a time, joined when Find or the editor ends: its fits take about 4 ms at 192 kHz, too long
for a frame, while listening and the copy stay inside the analyser's budget (§4).

**Not in F4**: continuous or dynamic suppression; proposing dynamic bands; a decay test (it needs the
excitation to stop, and ringing is a poor guide to audibility, §8.4); excluding a note's multiples
by pitch tracking (§8.2); finding in the side chain or the output; auditioning a candidate before it
is applied.

**Tests** (written at the end): a fixed Q-10 resonance at 1 kHz under a melody of quarter-second
notes is found within a forty-eighth of an octave, its Q within 20 %, and neither the melody's
harmonics nor a lone note's in a region nothing else reaches; resonances at 60 Hz (Q 15) and 8 kHz
(Q 20) found; every rate read without a gap to as high as a band runs; a left-only resonance a Left band, a side-only one a Side band, unequal channels a
Left and Right pair, and a centred one Stereo; quiet frames not counting, a reset keeping the
listen unless into other channels; a candidate no longer found kept for 10 s of heard input, one
near it standing for it; at 100 % the applied bell cancelling a cookbook-shaped peak within 1 dB across its width;
two candidates a sixth of an octave apart, applied together, each within 0.5 dB of its target, and
two overlapping in different placements within 1 dB; a target past the cap held at −12 dB; the Q
bounds; a treated candidate not proposed, and proposed again when its band is bypassed or Amount
raised, with no flicker at the threshold; the count, spacing and free slots, a pair passed over with
one slot left; every field written over a stale Off slot; Apply all, a click and a drag each one undo
step, BACK leaving nothing; the keyboard walk and ADD; the toggles' rules; Amount saved, an old
project at 20 %, Find never saved; the opening size and narrow window with two columns, the keyboard
reaching the new card; the finder inside the analyser's 2 ms budget (§4) at 192 kHz.

**Delivery** (tests at the end, one review over F4.1–F4.2):

| Step | Delivers | Gate |
|---|---|---|
| **F4.0** | Decisions 27–30, signed. **Done** (2026-10-15) | Owner |
| **F4.1** | The finder's analysis: the rungs, the five spectra, peaks per frame, persistence, the long-term spectra, candidates, placement and ranking (`editor/finder.rs`). **Built** (2026-10-15): the finder's tests, the measurements above found by them | Unit tests at the end |
| **F4.2** | Proposals: depth, Q, treated candidates, the solve; the graph's curve and triangles; Apply all, click, drag, keyboard; the Resonances card, Amount and the second column (opening 1,532 × 692); the DOX: `plugins/mxm-fx-eq-n/AGENTS.md` and its `NOTES.md`, and the Child DOX Index rows in `plugins/AGENTS.md` and the root `AGENTS.md`. **Built** (2026-10-15), with the editor's tests; its cost at 192 kHz is `finder_cost`'s, recorded in the plugin's `NOTES.md` (*Find*) | On Windows `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test`, the release cost measurement; one agent-loop review of F4.1–F4.2 |
| **F4.3** | The owner by ear, on real material | Owner sign-off |

### 7.6 F5 — EQ match to a reference (revision 28; redesigned in revisions 32 and 34)

The next phase in the owner's order (decision 9). Matching is solved in outline, the same in every
source (research §10.1): the long-term spectra of a reference and of the source, their difference
in dB as the wanted response, smoothed, scaled by an amount, and fitted to bands. What is not
settled is where the reference comes from, what the fit may touch, how much it takes, and the
pitfalls every maker warns of (§10.7): matching notes instead of tone, narrow features from too
little smoothing, the extremes over-corrected, short analyses, an analysis taken through the EQ
itself, and the level.

**Each EQ learns its own track, and is matched to another's** (decision 31; the owner,
2026-10-28, having tried two learns in one window in Resolve), as every maker's match works
(§10.1): Pro-Q's instances each analyse their own track, and one chooses another to match to.
**Learn**, one toggle on the Match card: on, it learns this track anew from what plays, from
nothing, **its input before the EQ**, which the match changes, and **its output after it**, the sound
another EQ matches to; off, it keeps both **with the project**, host state, never a preset's
(`crate::learned`). **Match to** chooses what this track is matched to: another MXM EQ in the
process that has learned, by its track's name (§7.8, decision 41), as that one
learned it, whether or not its window is open and after a reload, the registry's entry sharing its
persisted field; or a saved reference (decision 48). A voice-over is matched clip to clip: the EQ on
the clip that sounds right learns while it plays, the EQ on the others learns while they play, and
the second is matched to the first. Like Find, Match is offered only with the analyser on, on
every layout.

**What was learned is kept.** A reset breaks Learn's frames and keeps what it heard, as Find's
does (§7.5: Resolve resets as it plays on from clip to clip). What it keeps is the long-term
spectra the fit reads, as **cells** (below), so it does not depend on the rate it was learned at:
per cell the input's covariance and the output's level, while it sounds. **Save** keeps this
track's output under a name (decision 48) for any track in any project.

**What it writes** (decision 32): **bands in the free slots only, the EQ counted.** The player's
bands are never changed; what the EQ as heard already does is part of the match, so the fit adds
what is left: a band already there is not proposed again, and after an Apply Match shows nothing
left to add. Applied, the bands are ordinary static bands, editable and deletable, as F4's.

**The track is learned before the EQ**, so the analysis is never taken through this EQ (§10.7); a
reference, another EQ's output, is heard after that track's EQ, as it sounds.
- **The long-term spectra.** Learn's width frames on F4's rungs (§7.5: 8,192 points, Hann,
  half a frame's hop, each rung its own frequencies), as **power means** over every frame of the
  learn, per bin and in linear units: the output's channels' power averaged (mono: the one); the
  input's **covariance**, `S_LL = E|X_L|²`, `S_RR = E|X_R|²` and the complex `S_LR = E[X_L·conj(X_R)]`,
  so the EQ can be counted as heard (below). Both are averaged into F4's 1/48-octave cells as power
  per hertz, the output's taken to dB, and kept so. **Every frame of a learn counts**, a quiet one
  with its little power and its time, so each rung's mean is
  a time average alike and a silent stretch in a learn scales every rung the same, a constant
  the level takes out: it changes nothing but by what frames straddling it mix in, which a longer
  listen dilutes. (Gating the frames, as F4 does, weighed a quiet passage differently on rungs of
  different frame lengths, a step at their edges — found building F5.1.) Only a heard frame (at
  least −60 dBFS, §7.5) counts towards the seconds heard. Proposals are fitted about twice a second
  while Learn learns and whenever the EQ, Amount, Detail, what was learned or what it is matched to
  changes; the card reads how long this track has been learned, and Learn's hover advises 30 s or
  more (FabFilter's advice, §10.1).
- **How sure it is** (decision 49): no fixed time is waited for. Every second of sound heard, a
  learn's frames go to its other half; each half's level is smoothed as the match reads it, and
  their difference, its mean over the range taken out (a level the match takes out), gives each
  cell's **spread**: the RMS of what is left over the smoothing's window, halved for the mean of
  two, widened by `√(1 + 2/n)` while the smaller half holds `n` seconds, as an estimate from few is
  less sure. Kept with what was learned, and in a saved reference (one without it is taken as
  sure). **The difference's shape is shrunk by it**: about its mean over the range (a level the
  match takes out, so never shrunk), each cell's `s` becomes `s·s²/(s² + u²)`, `u` the track's and
  the reference's spreads together, `√(a² + b²)`: a difference well over its spread is kept nearly
  whole, one the size of it halved, one well within it mostly taken back, and none asked for where
  a half has nothing yet. So a short learn proposes what it knows, the highs within seconds, and a
  large difference is never left out for being unsure. Measured on noise, the spreads fall under
  0.5 dB in three seconds in the highs and about thirty in the lows; a voice's stay at a dB or more
  for longer, its halves holding different sounds (NOTES, *Match*).
- **Smoothed** in dB by a Hann window in log-frequency **as wide as the wider of a third of an
  octave and one ERB** (`24.7 + 0.108 f` Hz, as octaves `log2((f + ERB/2)/(f − ERB/2))`): a third of an octave above about 200 Hz removes the
  comb of a key's notes (§10.2: twelve peaks an octave; a third of an octave is four of their
  periods, where a Hann window's response is zero), and the ERB below, half an octave at 100 Hz,
  averages across bass notes in different keys. A key's coarser pattern, one or two cycles an
  octave, is tone balance at that resolution and stays, as in every product. Linear in dB,
  smoothing each spectrum then subtracting is smoothing their difference (§10.2).
- **The difference** `D(f) = L_ref(f) − L_src(f)`, `L_src` the track's channels' power averaged.
  Where the reference has no level — a cell above what its rate read, or one no frame reached —
  `D` is not known there, and the fit's weight there is 0.
- **What the EQ as heard does**, `E(f)`, for its static curve: the source's smoothed level
  through the EQ less its own. Per cell the EQ is the 2×2 transfer `H` of the graph's two stages at
  the cell's centre (§7.3: the channels' `diag(dl, dr)`, then mid/side's `diag(dm, ds)`), each band's
  complex response from the numbers the audio runs (`Cascade::response`), gain scale, bypass and
  placement included; the output power is `trace(H·S·Hᴴ)` on the cell's linear covariance `S`, only
  then taken to dB and smoothed as the spectra are. So a Mid or Side band, whose
  power on a stereo source depends on how alike its channels are, counts as heard. A dynamic band
  counts at its drawn gain, its curve at rest; what it does while it moves is not modelled. Mono:
  the cascade's own response. A proposal is a Stereo band, the same response on both channels,
  which commutes with both stages, so the source through the EQ and the proposals stands exactly at
  `L_src + E +` the proposals' response.
- **The target** is `T(f) = Amount × D(f) − E(f) − c`, the level taken out after the Amount
  (§10.1: an offset left in spends the first band on the mean level). **The range** is
  **30 Hz – 16 kHz**: the fit's error is weighted by `w`, 1 there, falling to 0 by a raised cosine
  over the half octave beyond each end, so no corner is fitted and the extremes, where every maker
  found over-correction (§10.3, §10.7), ask for nothing — neither the difference measured there nor
  its absence — the EQ's own bands there left alone and a shelf free to run on past an end.
  (Easing the target itself to 0 dB had the fit spend a band undoing a shelf's tail below 30 Hz —
  found building F5.1.) The constant `c` keeps the loudness: it is solved, by bisection (over
  ±60 dB, sixty halvings), so that the source's K-weighted power (BS.1770's full K-weighting, its
  shelf and its high-pass) through the EQ and the eased target, `L_src + E + w·T` on every cell,
  equals it through the EQ alone, `L_src + E`. So
  applying the proposals moves the tone and not the level, at any Amount, and a reference at
  another loudness, only a constant in `D`, matches the same.

**The fit**, Ramos & López's order with Abel & Berners' refinement (§10.4), on points a
twenty-fourth of an octave apart from 21.2 Hz to 22.6 kHz (the range and its ease), none above the
DSP's `max_frequency` at the rate:
1. What is left, weighted, `R = w × (T −` the proposals' response`)` (none at first), splits into
   **regions**:
   maximal runs of consecutive points where `R` has one strict sign (a point where it is exactly 0
   belongs to none). A region's **extreme** is its point of largest `|R|`, the lowest on a tie; its
   **half-height crossings** are where, walking out from the extreme on each side, `R` read on the
   region's sign first falls to half the extreme's, so the walk may pass the region's own end,
   interpolated linearly in log-frequency between that point and the one before it; a side whose crossing lies beyond 30 Hz or 16 kHz, in the ease, or which has none,
   **reaches that end**. Its width `BW` is in octaves
   between the crossings, and its Q the cookbook bell's for that half-height width,
   `√(2^BW) / (2^BW − 1)` (§9.2), held in 0.3–4. A region **counts** when its extreme exceeds the
   deviation just audible for that Q, F4's `1.5·Q^0.3` dB (§8.8, Toole & Olive), a shelf's at Q 0.71.
   Of those, the one with the largest area, `|R|` summed over its points times a twenty-fourth of an
   octave, is taken; on a tie, the lower.
2. It **seeds one band**: a region that reaches the low end a **low shelf**, the high end a **high
   shelf**, both ends the one on the side its extreme is nearer in octaves — the EQ's second-order
   shelf at Q 0.71, its gain the extreme, its corner the region's inner crossing (with both ends
   reached, the extreme); any other a **bell** at the extreme, its gain the extreme, its Q as above.
   A seed's gain is held to ±12 dB at once.
3. **Every proposal is refined together**: Levenberg–Marquardt on the points' weighted dB error, each band's
   log-frequency, gain and log-Q (a shelf's corner and gain), on the bands' actual responses
   (`Cascade::response_db`) at the rate the EQ runs; bounded, gain **±12 dB** (Ramos & López's cap,
   §7.5's; Nercessian's ±10), a bell's Q **0.3–4** (narrower is detail the smoothing removed;
   Nercessian 0.1–3) and every centre inside 30 Hz – 16 kHz. The scheme is F4's `fit_bell`'s:
   forward differences at fixed steps, Marquardt's damping from 10⁻³ (×10 on a rejected step, ÷3 on
   an accepted one, at most six tries an iteration), the damped normal equations solved by Gaussian
   elimination with partial pivoting, each step clamped to the bounds; sixteen iterations, no early
   exit.
4. Again from 1, until **no region counts, Detail is reached or the free slots run out**.

Every step is fixed and the fit runs in `f64` on one thread, so the same input gives the same
bands on the same build. Bands come in order of importance, so stopping early
always keeps the best ones (§10.4, *Reconciled*). 15–20 such bands were nearly indistinguishable
from a 2,048-tap FIR (Ramos & López, §10.4), and after a third of an octave's smoothing Bank's rule
asks about 15 over the range, so the free slots are rarely what stops it.

**The fit runs on F4's worker thread** (§7.5), one job at a time, **the latest winning**: a request
made while a fit runs waits, a newer one replacing it, and runs when the worker is free. Each fit
carries a copy of the spectra, the listen it was made for and **its basis**: what was heard so far,
the reference matched, the EQ as heard, the free slots, Amount and Detail. A result is drawn only while Match is on that listen;
one made on an older basis is drawn until the newer fit arrives, but **Apply writes only proposals
fitted on the current basis** — the EQ, the free slots, Amount and Detail as they stand; what was
heard moves on with every frame, and a fit half a second old is still the match — and is
disabled while they are not.

**How much** (decision 33). One **Amount**, 0–100 %, **40 % by default** (Ozone: keep it under
50 %; Melda suggests 30–40 %; §10.3): `D` scaled in dB, as Pro-Q's gain scale and Logic's Apply
scale theirs, not a dry/wet (§10.3, *Derived*). **Detail**, Pro-Q's control, is **at most how many
bands the match proposes**, 1–24, **8 by default** (Ramos & López's error was about 0.5 dB by five
to ten sections, §10.4): since bands come in order of importance, a lower Detail keeps the most
important ones, and the fit still stops where nothing left is just audible, so a higher Detail
never adds an inaudible band. Amount and Detail are view state, saved with the analyser's settings
(§4), Amount in whole percent: a project from before loads 40 % and 8, a change marks the project
dirty. What each EQ learned and what it is matched to are host state, saved with the project
beside the view settings (`EqParams::learned`, `match_to`); the proposals are the editor's moment,
never saved.

**On the graph**, while Match has proposals: **the curve they would make**, dashed over the EQ's,
as F4's, and each proposed band a **hollow diamond** at its centre and gain, labelled with its
frequency and gain (`2.4 kHz +1.8 dB`), told apart from F4's triangles by shape, never hue; labels
overlap by F3's rule. The proposals are one fit, so they are applied together: **Apply**, on the
card, writes every proposal into the lowest free slots as one undo step, each slot's every field
(`slot_writes`), a static, unkeyed Stereo bell or shelf. With every slot in use nothing is
proposed and Match's hover says why.

**One proposal set at a time**: turning Learn on, or choosing something to match to, ends Find and
Freeze; turning on Find or Freeze, or the analyser off, ends Learn, keeping what it learned, and
Match proposes nothing while Find or a freeze is on. Choosing what to match to again brings its
proposals back. What was learned, being cells, outlives a new rate; what an earlier learn found is
never drawn in a later one.

**Saved references** (decision 48; the owner, 2026-10-28: the reference saved, so any track can be
matched to it). **A reference is not a preset**: a preset holds parameter values only (§5.4, the
kit's conventions), and a reference is a measurement. A file per reference, in a `references`
folder beside the EQ's user presets (`<config>/mxm/dk.mxm.mxm-fx-eq-n/`, mxm-kit's
`mxm_preset::library` layout), so an install never touches them; **a folder, not a dialog**, as the
kit's banks are: **Folder** shows it, and a reference is renamed or deleted there.
- **The file**, JSON: a format name and version, the reference's name, the seconds heard, the rate
  it was learned at, and its level per 1/48-octave cell from 20 Hz in dB while it sounds,
  unsmoothed (the fit smooths), `null` where nothing was read. One that does not parse, or of a newer version, is listed
  marked *cannot read*, the reason on the list's hover, and cannot be chosen.
- **Save** keeps this track's output as learned under a name, asked for in a text field where
  Match to was, the track's name offered first; Save or Enter saves, BACK leaves it; a name already
  taken is replaced only on a second press, the button reading *Replace*. Saving needs something
  learned, Learn off; what this track is matched to does not change.
- **Match to remembers a saved one** by name with the project. If its file has gone when the
  project opens, the list reads *Missing* and Match proposes nothing until another is chosen. Another
  EQ chosen is remembered by its instance's id (§7.8), read *not learned* while it has nothing, and
  *Track not found* while it is not in the process.

**The Match card** (decision 34) holds five rows, top to bottom as it is used: **Learn**, its
**lamp**, how long this track has been learned and, while matched, how many bands are proposed
(*no change* when none); **Save** and **Folder**, which shows the folder;
**Match to** (*Nothing*, every other EQ that has learned by its track's name, then the saved
references by name, one sharing a track's name marked *saved*); **Amount** and **Detail** side by
side; **Preview** and **Apply**, whose hover says why it is disabled: nothing differing enough to
hear at this Amount, or a fit on its way. Hover only, no help text. **The lamp** (the owner, 2026-10-28) is green and
filled while Learn hears sound, its seconds heard rising within the last `LAMP_SECONDS`, and a ring
otherwise, so its state never rests on hue: on and dark says nothing is playing on this track. The
card sits **under the Band card in the first column**, which is then the taller, and the window
opens hugged to it (`REFERENCE`, the opening-size tests). §5.1's rules hold.

**Preview** (decision 50; the owner, 2026-10-28): while on, the proposals are heard as if applied,
each in the free slot Apply would write, its gain under the gain scale, following Amount and Detail
as they move; off plays the EQ as it is, so the switch is the comparison. **A transient command**,
as band solo is (§5.2; `plugins/AGENTS.md`'s exception): no parameter is written, so a project never
reopens previewing, a render never plays a proposal and the host's undo and automation see nothing.
The editor hands the bands over (`preview.rs`); each block the shell plays them only with an editor
connected and never offline, each only in a slot that is Off, static, unkeyed and Stereo, so a slot
the player has since used is the player's; closing the window ends it. After Apply what it played
is held for half a second, while the host takes the bands written, so the sound does not dip. While
it is on, the dashed curve is the one heard.

**What was learned is drawn** (the owner, 2026-10-28): while Learn is on, once it has been on in
this window, or while this track is matched to something, the graph draws the reference matched to
and this track's input as learned as two solid lines, lettered
**Ref** and **Track** at their ends and never told apart by hue alone, behind the bands, each
smoothed as the match reads it. They sit on the analyser's scale, so they line up with the live
spectrum: a cell's power per hertz times the display's twelfth-of-an-octave band, both scaled so a
full-scale sine reads 0 dB, tilted as the display is (`learned_display_db`). **A learn's level is
read while it sounds**: every frame counts towards its long-term mean, pauses too, so its cells are
raised by the seconds learned over the seconds heard; a learn left on through a voice-over's
pauses reads the level of the voice. It is one level for every cell, so it changes no match, and a
saved reference holds it. The lines are made anew at most every `LINES_SECONDS` while Learn runs.

**Not in F5**: a reference from an audio file; matching left, right, mid or side separately; a
dynamic band's motion; separate boost and cut ceilings or frequency handles; applying one proposal
of a fit alone; linear-phase matching (F6); matching dynamics.

**Tests** (written at the end): the reference a noise through a known EQ (a +4 dB low shelf at
120 Hz, a −3 dB bell at 800 Hz Q 1.4, a +3 dB bell at 3 kHz Q 1, a −2 dB high shelf at 9 kHz) and
the source the same noise: at 100 % and at 40 % no region of what is left counts (the stopping
rule, checked on the final fit, at Detail 24), and at 100 % each of the four has a band of its sign within a
sixth of an octave, the shelves seeded as shelves, at most eight bands in all; the reference 10 dB
louder proposing the same within 0.5 dB; at 40 % and 100 % the source's K-weighted power through
the EQ and the proposals within 0.5 dB of it through the EQ alone; a Mid band and a Left band in
the EQ on a correlated stereo source counted as heard (the source's level through EQ and target
against the rendered output's long-term spectrum, within 0.5 dB); a difference that is a comb of
lines a semitone apart (each 12 dB over a flat floor, at any offset) smoothed flat within 0.25 dB
from 30 Hz up, so proposing nothing; the EQ already carrying the difference proposing nothing,
and after an Apply nothing left; with three free slots, and at Detail 3, exactly the unbounded
fit's state after its third band; a silent stretch in the reference changing the match by under
0.5 dB anywhere over the range; three seconds of the known difference proposing the 3 kHz bell and
nothing under 300 Hz, twelve proposing the low shelf too; a difference shrunk by its spread, kept
well over it, halved at it, none where nothing is known; a reference's unread cells weighing
nothing; Preview heard in an Off slot with an editor, giving way to a band the player puts
there, never offline, never allocating, and handed over in the slots Apply writes, nothing once off
or closed; Learn kept when
turned off, read back whole from the project's state by another instance, and learning anew when
turned on again, the lamp lit while it hears and dark once nothing comes, a reset keeping what it
heard and a restart forgetting it; one EQ matched to another that has learned, proposing, one that
has learned nothing not offered; a learn with pauses reading within 0.5 dB of one without, from
60 Hz to 8 kHz; the learned lines of white noise and of a tilted one at the analyser's level at
1 kHz, within 1 dB; a reference saved and read back whole, cell for cell; a sine's power read from a
reference learned at 44.1 kHz and at 96 kHz the same within 0.2 dB at 1 and 8 kHz, and 0.5 dB at
100 Hz, where a cell is a few bins (two noise learns at different rates differ by their noise, up
to 1 dB in 11 s, and test nothing); a reference's unread cells weighing nothing; a file that does
not parse or is of a newer version listed with its reason; a name taken replaced only on
the second press, saving not changing what is matched to; the saved one chosen holding what this
track sounded like, the choice saved with the project, *Missing* when its file has gone;
the fit deterministic, the region rules' edges (a zero point, a region at each end and at both,
the tie); a newer request superseding a waiting one, an older listen's result dropped, Apply
refused on a stale basis; Learn's rules against Find, a freeze and the analyser, Find hiding the
proposals and choosing again bringing them back; Apply one undo step, every field written over a stale Off slot; Amount and Detail
saved, an old project at 40 % and 8, Match never saved; the Match card under the Band card, the
opening size hugged to the first column, and the keyboard reaching it; the listening inside the analyser's 2 ms budget (§4) at 192 kHz.

**Delivery** (tests at the end, one review over F5.1–F5.2):

| Step | Delivers | Gate |
|---|---|---|
| **F5.0** | Decisions 31–34, signed; their statuses and the plan's status line updated. **Done** (2026-10-15) | Owner |
| **F5.1** | The match's analysis and fit: the two long-term spectra and the source's cross-spectrum, smoothing, the difference, the EQ as heard, the target and its level, the greedy fit and its refinement (`editor/matcher.rs`), run on the worker. **Built** (2026-10-15), with the match's tests: the ease made the fit's weight and every frame counted, found by them | Unit tests at the end |
| **F5.2** | The editor: the Match card, Amount, Apply, the graph's curve and diamonds, the toggles' rules, the key's switch, the worker's latest-wins jobs; the DOX: `plugins/mxm-fx-eq-n/AGENTS.md` and its `NOTES.md`, the scope descriptions in the existing Child DOX Index rows of `plugins/AGENTS.md` and the root `AGENTS.md`, and this plan's status line and F5's delivery rows. **Built** (2026-10-15), with the editor's tests; its cost at 192 kHz is `match_cost`'s, recorded in the plugin's `NOTES.md` (*Match*) | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin and its host tests, `cargo test -p mxm-fx-eq-n`, the release cost measurement; one agent-loop review of F5.1–F5.2 |
| **F5.3** | The owner by ear, in Bitwig: one track matched to another | Owner sign-off |
| **F5.4** | Decisions 31, rewritten, and 48, signed; their statuses and the plan's status line updated. **Done** (2026-10-28) | Owner |
| **F5.5** | The learns: the matcher's reference and track learned apart, each held; the reference as cells; its three sources, another track watched while it is learned; a reset keeping a learn; the fit reading cells, unread ones unweighted. **Built** (2026-10-28) | Unit tests at the end |
| **F5.6** | Saved references: the folder, the file and its version, Save and Replace, the list, *Missing*, Folder; the card's new rows; the chosen name saved with the project; the DOX: the plugin's `AGENTS.md` and `NOTES.md` (*Match*), and this plan's status line and rows. **Built** (2026-10-28) | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin, `cargo test -p mxm-fx-eq-n` |
| **F5.7** | The owner by ear, in Resolve: one voice-over clip matched to another | Owner sign-off |
| **F5.11** | Decision 49, rewritten: the difference's shape shrunk by its spread (`matcher::shrunk`), no cell left out for it; the learned lines solid; the card's reading the bands proposed, Apply's hover why it waits; the DOX: the plugin's `NOTES.md`, and this plan's status line and rows. **Built** (2026-10-28) | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin, `cargo test -p mxm-fx-eq-n` |
| **F5.10** | Decisions 49 and 50: a learn's halves and spreads, kept and saved; the fit's weights by how settled each cell is, no fixed time; the learned lines faint where unsettled; Preview (`preview.rs`), the shell playing it in the Off slots; the card's five rows; the DOX: the plugin's `AGENTS.md` and `NOTES.md`, and this plan's status line and rows. **Built** (2026-10-28) | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin, `cargo test -p mxm-fx-eq-n` |
| **F5.9** | Decision 31, rewritten again, and 48: one Learn per EQ, its input and its output kept with the project (`crate::learned`, the registry sharing it); Match to and its choices; the fit reading cells; the card's four rows; the DOX: the plugin's `AGENTS.md` and `NOTES.md` (*Match*), and this plan's status line and rows. **Built** (2026-10-28) | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin, `cargo test -p mxm-fx-eq-n` |
| **F5.8** | Decision 34, rewritten: the card under the Band card, its rows, a lamp for each learn (`update_lamps`); what was learned drawn (`learned_lines`), a learn's level read while it sounds; the DOX: the plugin's `AGENTS.md` and `NOTES.md` (*Match*), and this plan's status line and rows. **Built** (2026-10-28) | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin, `cargo test -p mxm-fx-eq-n` |

### 7.7 F6 — phase modes (revision 29, for the owner's approval)

The next phase in the owner's order (decision 9). Every band so far is minimum phase: a cascade of
matched SVF sections (§3.1), its magnitude fitted to the analogue prototype's, and since an
analogue bell, shelf or cut is minimum phase too, its phase follows the analogue's wherever the
magnitude does. Research §5 says what the other mode buys and costs. A **linear-phase** EQ is a
symmetric FIR: no phase shift between frequencies, so a placed band adds no phase between the
channels — a Mid or Side band puts sound in the other channel only by its magnitude, never where
it is flat (§6) — and a steep cut leaves transients' shape alone; but its latency is the kernel's
centre, its ringing sits before a transient as much as after, and narrow low cuts pre-ring for
100–400 ms (§5.2, 10–40 times the backward masking). FabFilter: "not better or more transparent …
it is different" (§5.2). So it is a mode the player picks for a job, never the default.

**The modes** (decision 35): **Zero latency**, the EQ as it is, and **Linear phase**. Pro-Q's third
mode, Natural phase, matches the analogue phase too at a small latency (§5.3, method undisclosed);
here Zero latency already does, being minimum phase with the analogue's magnitude, except in the
top octave where the matched magnitude leaves the prototype's, so a third mode would buy little.

**Linear phase.** The EQ's magnitude, as Zero latency would hear it, made into a zero-phase FIR
kernel and run by uniformly partitioned FFT convolution:
- **The kernel** is designed on N points: per bin the EQ's static magnitude — every band in the
  sound, its design, gain scale, bypass and placement as §7.3 has them — taken as a real,
  zero-phase response, inverse-transformed, shifted circularly by N/2 and multiplied by the
  periodic Hann window `w[m] = sin²(π·m/N)`, zero at m = 0 and symmetric about N/2, each tap pair
  about N/2 then mirrored (the mean of the two taken for both): so the kernel is symmetric to the
  bit about its centre tap N/2 over taps 1…N−1 (tap 0 is zero), its tails end at zero, and a feature narrower than a few bins is smoothed, which is what the resolution means.
  It is then transformed into partitions of B. With every band Stereo it is one kernel
  for both channels; with Left or Right bands, one per channel; with Mid or Side bands the two
  stages' magnitudes make a real 2×2 matrix per bin (`L′ = a·l·L + b·r·R`, `R′ = b·l·L + a·r·R`,
  `a, b = (m ± s)/2`, each zero phase), four kernels, so placed bands stay exactly linear phase
  with no stage run twice (Pro-Q 4 doubles its latency there, §5.2).
- **Its length N** (decision 36) is the resolution: **Low, Medium or High**, 4,096, 8,192 or
  16,384 points at 44.1–48 kHz (85, 171 or 341 ms), twice that at 88.2–96 kHz and four times at
  176.4–192 kHz, at another rate the power of two nearest the same time. A bin is `fs/N` wide and
  the window blurs about two: a band wider than eight bins is heard within a quarter of a dB of
  Zero latency's magnitude — at 48 kHz, Medium any band wider than 47 Hz (Q 1 at 50 Hz, Q 20 at
  1 kHz), High one wider than 23 Hz (Q 2 at 50 Hz); a narrower one is smoothed, and drawn so
  (below). A narrow low cut is what pre-rings longest (§5.2), so the resolutions stop at High.
  **Medium** by default.
- **The latency** is `N/2 + B`, B the convolution's partition, 256 samples at 44.1–48 kHz (5.3 ms)
  and scaled with the rate as N is: Medium is 4,352 samples at 48 kHz, 91 ms.
- **A change is heard through a new kernel, designed off the audio thread.** At an update boundary
  (every 4·B samples, 21 ms at 48 kHz) after anything the kernel hears has moved, the audio thread
  copies what it hears — every band's design and placement and the gain scale, plain data — into a
  request. The engine's own **design thread** (spawned when the engine is built in Linear phase,
  joined when it is dropped; it looks for a request every millisecond) designs the kernels into one
  of three banks preallocated at activation and publishes it through atomics. **The audio thread
  never waits**: at a boundary it takes the newest finished bank. In the host's **offline** mode it
  waits, at the boundary after it asked, for the bank it asked for, so a bounce is the same every
  time. So a change is heard from the second boundary after it at the earliest, and fully, its
  crossfade done, by the third; in realtime as soon after as the thread has designed it.
- **Old and new kernels hear the same past.** The convolver keeps one frequency-domain delay line
  of the input's partitions; a bank is only coefficients, so a new bank's first output is already
  its convolution of everything heard, and over the interval after it is taken both banks run on
  that same line and their outputs crossfade. Glides and fades are the crossfade's; the output gain
  stays a ramp after the convolution, as in Zero latency. A drag is heard in 21 ms steps, each
  crossfaded, behind the latency.
- **An empty EQ is a pure delay**: with no band in the sound the convolution stops and the input
  comes out of a delay line of the latency, bit-exact; the output gain applies as in Zero latency.
- **Dynamic bands** (decision 37) **are held at their drawn gain** while Linear phase is on: a
  kernel steps every 21 ms, coarser than any Attack; a detector on the input as heard would lead
  the delayed output by the latency (a look-ahead nobody asked for), and one delayed to line up
  would answer 21 ms late at best. Their Range, side chain and meter stay set and come back in
  Zero latency; the card's hover says so, and the graph draws no ring.
- **Solo** plays its band pass as in Zero latency, on the input delayed by the latency, so an
  audition lines up with what it auditions.
- **The tail** is the kernel's: the output is exactly zero once the last partition holding sound has
  left the delay line, from N + 2B samples after the input's last nonzero sample on at the latest
  (the delay line, empty, holds spectra that are exactly zero); `KeepAlive` until then.

**One latency per activation** (the kit's *Latency is fixed*): **Phase** and **Resolution** are
permanent stepped parameters, appended after the placements (`phase`, 0–7: 0 Zero latency,
1 Linear phase; `resolution`, 0–7: 0 Low, 1 Medium, 2 High; the rest unassigned, read *Unavailable
n*, heard as Zero latency and Medium), **not automatable** (`ParamFlags::NON_AUTOMATABLE`), so
saved in presets and projects but never ridden. **Activation builds the mode the parameters hold
and reports its latency** (`ActivateContext::set_latency_samples`; 0 in Zero latency); a change
while active is reported from `process` (`ProcessContext::set_latency_samples`), which nice-plug
turns into the host's restart (CLAP `request_restart`), and until the host restarts the plugin
the mode in force stays — the bands keep sounding, never a gap. Bitwig restarts at once; MXM
Player's headless path does not restart (its `request_restart` is a flag), so there a change
applies at the next activation. **What runs and what is asked**: the shell publishes the mode and
resolution in force (`Telemetry`), set at activation; the editor's curve and Phase's latency
hover follow what is in force, and while a change waits for the restart the selectors show what
was asked and their hover says it applies when the host restarts the plugin. A state or preset
from before F6 loads Zero latency and Medium (`default_missing_legacy_parameter`,
`add_missing_at_defaults` and the baselines they extend); the factory set is regenerated from its
design (`write_the_factory_presets`), Init and every preset Zero latency and Medium.

**The DSP crate** gains its first runtime dependency, mxm-kit's **`mxm-fft`** (MSRV 1.87, MIT,
`realfft` beneath): the owner's decision 6 rules one shared FFT, never another in a crate. The
convolver and the kernel design live in `crates/mxm-fx-eq-n-dsp/src/linear.rs`, allocated when
activation builds the engine for Linear phase (`Engine::with_phase`), at its resolution, never on
the audio thread; `Engine::new` stays Zero latency. The convolution runs in `f64`, as the cascade
does: the gain range reaches +1,440 dB, past `f32` (code review, 2026-10-15). A reset asks for
the settings' kernel at once: offline it waits; in realtime, never waiting, it is taken the first
partition it is ready, unfaded while only silence has been convolved since, else faded in.

**In the editor**: **Phase** and **Resolution** selectors in the Output card, one row each (side
by side they do not fit the column), Resolution enabled only in Linear phase; Phase's hover gives the latency in milliseconds. **The
drawn curve is the heard curve** in both modes: in Linear phase the editor draws the kernel's
own magnitude, from the DSP's design function, so a band narrower than the resolution reads as it
sounds: the kernel the current settings make, which the bank heard reaches within the update
intervals above. The Output card grows by two rows; the window opens taller, hugged, inside the budget.

**Cost** (plan §3.5, brief A6's 35 % steady and 70 % moving): at 192 kHz, High, four kernels and
every slot in use, in release, by the cost example (`eq_n_cost`), not by wall-clock assertions in
tests: the audio thread's share of a core steady and while a bank fades (two banks on the line),
and the design thread's time per bank. Within A6, or the plan says what does not fit.

**Not in F6**: Natural phase; per-band phase; dynamic bands moving in Linear phase; a resolution
above High; Find's and Match's proposals made linear (they are bands like any other, heard in
whatever mode is on).

**Tests** (written at the end): at every resolution, a band wider than eight bins heard within
0.25 dB of Zero latency's magnitude, and the drawn curve within 0.1 dB of what the kernel does;
the kernel exactly symmetric about tap N/2, so its group delay is the latency at every frequency;
a hard-left sine through a Mid bell (+6 dB, 1 kHz, Q 1) in Linear phase putting exactly
`(|m| − 1)/2` of it into the right channel, within 0.1 dB, at 100 Hz, 1 kHz and 10 kHz (Zero
latency adds a phase term there, §6); an empty EQ the input delayed by the latency, bit for bit;
the latency reported at activation for each mode, resolution and rate family, and a change asking
for a restart while the mode in force stays until it comes; after 2 s of sustained noise a kernel
change, its output once the fade is done the new kernel's convolution of the whole input within
−90 dB of the output's RMS, and click-free (during the fade no step between samples larger than
the larger of the two kernels' own outputs' over the same samples, plus 1 % of the signal's
peak); in offline mode a change heard from the
second update boundary after it and fully by the third, the same on every run; the audio thread
never waiting in realtime mode; a dynamic band static in Linear phase and moving again in Zero
latency; solo lined up with the delayed input; exact silence from N + 2B samples after the input
ends; the editor drawing what is in force while a change waits; states and presets from before F6
in Zero latency and Medium, the factory set regenerated; the parameters 292 and permanent, their
stepped places pinned; the allocation guard in Linear phase with every slot changing; the Output
card's row in the opening size and the keyboard; the cost at 192 kHz.

**Delivery** (tests at the end, one review over F6.1–F6.3):

| Step | Delivers | Gate |
|---|---|---|
| **F6.0** | Decisions 35–37, signed. **Done** (2026-10-15) | Owner |
| **F6.1** | The DSP: `linear.rs` (the kernel design, the 2×2 matrix, the partitioned convolver and its one delay line, the banks, the design thread and its handoff, the offline wait, the crossfade), the engine's mode and its latency, `mxm-fft`; the DSP's `AGENTS.md` and `NOTES.md` (the cost). **Built** (2026-10-15), with the DSP's Linear-phase tests; its cost is `eq_n_cost`'s, in the DSP's `NOTES.md` | Unit tests at the end; the cost measurement |
| **F6.2** | The shell: `phase` and `resolution` (292 parameters), activation's report, the change reported from `process`, the mode in force published, the tail, old states and presets, the factory set regenerated; the host test of a linear-phase EQ through MXM Player. **Built** (2026-10-15), with the shell's tests and the host test | Unit and host tests at the end |
| **F6.3** | The editor: the Output card's row, the drawn curve in Linear phase as in force, the hovers; the DOX: the plugin's `AGENTS.md` and `NOTES.md`; the root `AGENTS.md`'s MSRV row (the DSP's one runtime dependency) and the parameter count in its and `plugins/AGENTS.md`'s index rows; this plan's status. **Built** (2026-10-15), with the editor's tests | On Windows `cargo fmt --all -- --check`, `cargo clippy` on the plugin, its DSP and host tests, `cargo test` on both; one agent-loop review of F6.1–F6.3 |
| **F6.4** | The owner by ear, in Bitwig: Linear phase on a low cut and on a Mid band, the restart on a change | Owner sign-off |

### 7.8 F7 — auto gain and other tracks' spectra (revision 30, for the owner's approval)

The next phase in the owner's order (decision 9). §7's F7 row listed four candidates, each the
owner's call; **the owner chose two** (2026-10-16): **auto gain** and **other tracks' spectra**,
with Pro-Q 4's collisions. Set aside: character or saturation (research §15: a nonlinear effect
beside the curve, no measured study, oversampling against aliasing) and band modulation (research
§14: Pro-Q has none and its maker declined it; a host's modulators already ride the band
parameters).

#### Auto gain

An EQ that boosts sounds better partly because it is louder, so a comparison with the EQ off
judges the level, not the curve. Seven of the fifteen EQs surveyed have auto gain (research §19's
table). **Pro-Q's is an estimate from the curve**: "an educated guess based on the current EQ
settings", "not a dynamic process based on actually measured levels" (FabFilter, *Pro-Q 4 help,
Output*, read 2026-10-16; no formula published).

**The estimate** (decision 38): the loudness change the drawn curve makes on an average mix,
undone at the output. At the points `f_k = 20·2^(k/24)` Hz, k = 0…K, `K = ⌊24·log₂(f_max/20)⌋`,
`f_max` the lower of 20 kHz and 0.45 of the rate (239 points at 44.1 kHz, 240 from 48 kHz up),
the curve's power gain P(f) is weighed by `w(f) = S(f)·K(f)²·f` — S the average mix's power per
hertz, K BS.1770's K-weighting, f because the points are log-spaced — and the make-up is
`−10·log₁₀(Σ w·P / Σ w)` dB, held inside ±24 dB; an empty EQ sums nothing and is exactly 0 dB:
- **The average mix** (decision 39) is the research's corpus mean (§10.6): per FFT bin, falling
  5.79 dB an octave from 94 Hz up (Elowsson & Friberg's straight line to 15.7 kHz, held on above
  it), and below 94 Hz falling 12 dB an octave towards 20 Hz, a model of Pestana's rise to
  90–100 Hz and low cut round 60 Hz, not a measurement. Computed (2026-10-16): 5 % of the weight
  lies above 4 kHz, so a +6 dB step above 4 kHz is made up by 0.65 dB, and the same step below
  100 Hz by 1.5 dB; pink noise, the alternative, puts 43 % of the weight above 4 kHz and makes up
  the treble step by 3.6 dB. A broadband +6 dB is made up by exactly 6 dB under either.
- **K-weighting** is BS.1770's two stages as a magnitude (`research:listening/perceptual-measures.md`
  §9: −13.3 dB at 20 Hz, −1.1 at 100 Hz, +0.7 at 1 kHz, +4.0 from 4 kHz up): the standard's
  48 kHz biquads' response at `e^{jω}`, `ω = 2π·f/48,000`, whatever the session rate (every point
  lies below 24 kHz), so the weighting is the standard's at every rate; it weighs frequencies and
  filters nothing.
- **The curve** is what Linear phase's kernel hears (§7.7): every band in the sound at its
  targets, the gain held to ±30 dB, then the gain scale's target; a dynamic band at its drawn
  gain, so the make-up never moves with a detector; in mono only Stereo bands. **Placed bands**
  count by their share of an uncorrelated stereo signal: with the channel stage's magnitudes l, r
  and the mid/side stage's m, s, `P = ((l² + r²)/2)·((m² + s²)/2)`, so a Left band or a Mid band
  acts on half the power; in mono `P = l²`.
- **When**: at the first control interval after a setting the curve hears has moved, then at
  most every 10 ms while it keeps moving; the output gain's ramp (20 ms, §3.4) carries each new
  make-up, so a drag is followed in steps without a click. A settled EQ costs nothing; while one
  moves, 24 bands at 240 points every 10 ms, which `eq_n_cost` measures at 192 kHz.
- **Where**: the DSP, `crates/mxm-fx-eq-n-dsp/src/loudness.rs`, its points and weights computed
  when the engine is built; the engine applies the make-up with the output gain, after the bands
  or the convolution, in both phase modes, so the host's bypass compares at the same loudness.
- **The make-up and Output gain are separate factors**, each held inside ±24 dB, so ±48 dB
  together, the output's bound (`OUTPUT_BOUND`, +24 dBFS) clipping after them as now: Output gain
  stays what its control says. **Switching auto gain on or off** — by hand, by automation or by a
  state restored while active — ramps the make-up from 0 dB to the estimate, or back, over one
  fade, as a new estimate does; a reset or an activation starts at the estimate at once (§3.4).

**A parameter** (decision 40): `autogain`, appended after `resolution` (293 parameters), on or
off, automatable, saved with presets and projects, **off by default**: Init, the factory set
(regenerated) and every state or preset from before F7 load off. An empty EQ's make-up is exactly
0 dB, so Init stays bit-exact dry either way.

**Cost** (brief A6's 35 % steady, 70 % moving, at 192 kHz): `eq_n_cost` gains a load with auto gain
on and every band moving, so an estimate every 10 ms on top of F6's moving load; within A6, or the
plan says what does not fit.

**In the editor**: an **Auto gain** switch in the Output card, on Phase's row as Bypass shares
Placement's (a row of its own put the window 1 px over the budget, found building it); its hover
says what it does and gives the make-up in force, which the shell publishes in `Telemetry`. The
graph draws the curve as before: the make-up is a level, not a shape. The window opens at
1,532 × 1,045, the External selector's row added.

#### Other tracks' spectra

Seeing where this track's sound sits against another's is the users' repeated wish (research §19:
"other tracks' spectra in the analyser"), and Pro-Q 4 marks where the two may collide (research
§12). Its SC/Ext menu shows one external spectrum, the side chain or another instance's post-EQ
spectrum, and Show Collisions marks it; FabFilter: "merely an indication, and not exact science.
Always use your ears" (*Pro-Q 4 help, Analyzer*, read 2026-10-16). Its Instance List shows every
instance and edits them (research §12).

**One external spectrum** (decision 43): the Analyser card's **Side chain** switch becomes an
**External** selector: **Off**, **Side chain** (on a layout with the port, as now) and every other
mxm-fx-eq-n instance in this process, by name. The one chosen is drawn as the side chain is now, a
dashed line lettered with its name, its post-EQ spectrum read through this editor's own analyser
settings (resolution, speed, tilt). While Match listens (§7.6) the side chain is shown, as now.
**Collisions**, a switch beside it enabled while an external spectrum is shown, marks along the
graph's bottom edge where this track's post-EQ spectrum and the external one may collide:
- both as drawn (tilted, §4) and smoothed to a third of an octave, so comb ripple marks nothing:
  at each drawn frequency the mean **power** (not dB) of the drawn points within a sixth of an
  octave either side, a window cut short by the graph's edges rather than padded;
- **only while both hold sound**: each analyser has read a window and is not stale (§4: a gap,
  a new generation or 0.3 s without audio restarts it and drops its reading), and only at
  frequencies where both levels are above −80 dB as drawn; silence, a stopped transport or a
  source gone marks nothing;
- strength `clamp((E − O + 12)/6, 0, 1) · clamp((O − (peak − 30))/6, 0, 1)`, O and E this track's
  and the external level, peak this track's loudest drawn level: full where the other is within
  6 dB of this track or above it while this track is within 24 dB of its peak (sound that matters
  here, with something near enough to mask it), nothing where the other is 12 dB under or this
  track 30 dB under its peak;
- drawn as diagonal hatching whose opacity is the strength, never a hue; its hover names the
  track and says it is an indication, to listen before cutting.
The External choice and Collisions are view state (`ViewSettings`, saved with the project, no
undo), and not part of what a freeze holds (§7.4).

**How instances meet** (decision 41): **in one process**, through a registry every instance joins
when it is made and leaves when dropped (`plugins/mxm-fx-eq-n/src/instances.rs`). One process
covers Bitwig's default hosting, *Together*, and *Within Bitwig*, *By manufacturer* and *By
plug-in* (Bitwig user guide, *Plug-in handling*, read 2026-10-16), REAPER's default and newDAWn;
under Bitwig's *Individually* each instance is alone and the list is empty, which the selector's
hover says. Sharing across processes is not in F7.
- **The audio thread never waits on another instance.** Each instance has a second tap, post-EQ
  only, preallocated at construction (32 blocks), which copies blocks into its own lock-free ring
  only while an editor watches it (an atomic count of watchers): no lock, no allocation. When the
  count rises from none, the tap starts a new generation and the entry drops the blocks it held,
  so audio buffered before a watch never reads as current.
- **Editors share what they read.** The ring has one consumer, so it lives in the instance's
  registry entry behind a mutex that only editor threads take. A watching editor drains it each
  frame into the entry's last 64 blocks (allocated at the first watch, on that editor's thread)
  and feeds its own second analyser the blocks newer than it has seen; two editors watching one
  track each see every block, and a gap restarts the window, as the analyser's own handoff does
  (§4). A watch is counted while the editor is open and that track chosen, and dropped with
  either.
- **Identity**: each instance keeps a random 64-bit id (`std`'s per-process random hasher over
  the time and a counter) in the **host's state**, a persisted field (`#[persist = "instance"]`)
  beside the view settings, not a parameter: **never in a preset** (presets are parameters only,
  §5.4), untouched by Init and undo. nice-plug restores it with the host's state; the registry then
  takes the restored id, and of two instances holding one id (a duplicated track) the one that had
  it first keeps it and the other gets a new one. An External choice saves the id in the view
  settings, so a reloaded project finds the same track whichever instance loads first; a choice
  whose track is not in the session is shown as not found, draws nothing, and is found again when
  the track appears.
- **Lifetimes**: the instance and its editor hold its registry entry; the registry and every
  viewer hold it weakly, a viewer upgrading it for the frame it drains, so an instance removed
  while watched is gone at the viewer's next frame and nothing outlives it but the frame in
  progress. A watch is a guard, dropped with the window or the choice, so the count goes back and
  the share tap stops copying; a name the host changes shows at the next frame.

**Names from the host** (decision 42): an instance is listed by its **track's name**, which
CLAP's track-info gives (`clap.track-info`; nice-plug hands it to the plugin's editor object,
which exists while the window is closed). mxm-kit's `mxm-editor` does not pass it on to a plugin's
panel today, so **F7 adds that to mxm-kit**, a default `Panel::track_info_changed` the app
forwards, which no other plugin need implement; **pushing mxm-kit's public `main` is the owner's
to approve, which signing decision 42 does**. With no name from the host an instance is "Track n"
in the order instances were made; two of one name get " 2", " 3"; the list runs alphabetically and
never lists the editor's own instance.

**What it costs**: a viewer, one more stream, post only, through a second analyser, within the
editor's analysis budget (`spectrum::BUDGET_MS`), which `analyser_cost` checks with an external
spectrum on; the watched instance's audio thread, a copy of its output into a block every 256
frames, which a new release check beside it, `share_cost`, measures at 192 kHz as a share of
real time, watched and not, within A6.

**Not in F7**: Pro-Q's Instance List and editing other instances; Match (§7.6) taking another
instance as its reference; sharing across processes; renaming an instance; a make-up measured
from the signal; auto gain per channel or per mid/side (DMG keeps the width that way, research §6).

**Tests** (written at the end): the make-up exactly 0 dB for an empty EQ, and within 10⁻⁶ dB of
0 for bands that cancel (a +6 and a −6 dB bell alike); a flat +6 dB power gain made up by −6 dB
within 10⁻⁹ dB (the weights normalised), and a +6 dB low shelf with a +6 dB high shelf at 1 kHz,
Q 0.71, complementary analogue shelves, within 0.1 dB of it; a +6 dB high
shelf at 4 kHz made up by less than 1 dB and a +6 dB low shelf at 100 Hz by more than 1 dB; held at
±24 dB; a Mid band of power gain G counting `(1 + G)/2` where the same band in Stereo counts G;
mono counting only Stereo bands; the make-up ramping over the fade on a new estimate and on
switching auto gain on and off, no step between samples larger than the ramp's own plus 10⁻⁹ of
the signal's peak; Output gain and the make-up each held at ±24 dB; a bounce twice bit-identical; no
allocation with auto gain on and every slot changing; the cost at 192 kHz. Through MXM Player: a
restored state with auto gain on and a broadband boost rendering within 0.1 dB of the dry level. The parameters 293 and
permanent; older states and presets off; the factory set regenerated. The registry: instances
joining and leaving; a duplicate id renewed for the later one; names from track info, the
fallback and the suffixes; the list alphabetical without its own instance; the share tap copying
only while watched; two viewers each seeing every block, a gap restarting the window; a choice
saved and found after a reload whichever instance loads first; an instance dropped while a viewer
drains it, gone at the next frame; a closed window dropping its watch; a renamed track at the next
frame; through MXM Player, the host's state round trip keeping the id, and a preset never carrying
it. The collision strength at its edges on made-up spectra, and nothing marked on silence, stale
input, a gap or a source gone; the hatching only while Collisions is on and an external spectrum
shown; the
External selector and Collisions in the keyboard's reach; the opening size. In mxm-kit:
`mxm-editor` forwarding track info to the panel.

**Delivery** (tests at the end, one review over F7.1–F7.4):

| Step | Delivers | Gate |
|---|---|---|
| **F7.0** | Decisions 38–43, signed; with 42, the push to mxm-kit's `main`. **Done** (2026-10-16) | Owner |
| **F7.1** | Auto gain: `loudness.rs` and the engine's make-up (the DSP's `AGENTS.md`, `NOTES.md` with the cost); `autogain` (293 parameters), older states and presets, the factory set regenerated, the make-up in `Telemetry`; the Output card's switch and hover; the host test of auto gain. **Built** (2026-10-16): `eq_n_cost`'s gliding load with auto gain 18.1 % at 192 kHz against 17.2 % without | Unit and host tests at the end; `eq_n_cost` within A6 |
| **F7.2** | mxm-kit: `mxm-editor`'s `Panel::track_info_changed`, its test and `AGENTS.md`, pushed with the owner's approval; this repository relocked on it. **Built** (2026-10-16), mxm-kit `5f34311` | The kit's own checks |
| **F7.3** | Other tracks: the registry, ids and names, the share tap; the External selector, the external spectrum, Collisions and their hovers; the host test of the id's state round trip. **Built** (2026-10-16): `analyser_cost` worst 0.400 ms of 2 with another track; `share_cost` +0.03 points at 192 kHz | Unit and host tests at the end; `analyser_cost` and `share_cost` within their budgets |
| **F7.4** | The DOX: the plugin's `AGENTS.md` (the host's state now holds the instance id beside the view settings; presets still parameters only) and `NOTES.md`, the DSP's, the index rows (293), this plan's status. **Built** (2026-10-16) | On Windows: `cargo fmt --all -- --check`; `cargo clippy -p mxm-fx-eq-n -p mxm-fx-eq-n-dsp -p mxm-fx-eq-n-host-tests --all-targets -- -D warnings`; `cargo test -p mxm-fx-eq-n-dsp`, `cargo +1.87.0 test -p mxm-fx-eq-n-dsp`, `cargo test -p mxm-fx-eq-n`; `cargo xtask bundle mxm-fx-eq-n --release` into this repository's own `target/bundled`, `clap-validator validate target/bundled/mxm-fx-eq-n.clap`, `cargo test -p mxm-fx-eq-n-host-tests`; one agent-loop review of F7.1–F7.4 |
| **F7.5** | The owner in Bitwig: auto gain against the host's bypass; EQs on two tracks, the external spectrum and collisions | Owner sign-off |

### 7.9 F8 — working with several bands (revision 31, for the owner's approval)

The last phase in the owner's order (decision 9): §7's F8 row, **multi-band selection with
relative editing, copy and paste of bands, global transforms**, what research §17.2–§17.4 found
established across the products surveyed. v1 kept the door open for it (§5.2: "Selection is a set
of bands, even though v1 selects one at a time. Commands take their target set, and an undo action
may span several bands"). What F8 needs decided first, the row says, are the selection gestures
and what a transform keeps.

**Pro-Q 4, the benchmark** (FabFilter, *Pro-Q 4 help, EQ display*, read 2026-10-16): "Click and
drag on the display background to select adjacent bands by dragging a rectangle around them";
"Hold down Ctrl (Command on macOS) and click another dot to select multiple bands"; "Hold down
Shift and click a dot to select a consecutive range"; dragging a selected dot moves them all, and
"the gain of all selected bands will be scaled relative to each other"; the wheel and Ctrl/Cmd-drag
change every selected band's Q; Copy from a dot's or the background's menu, Paste from the
background's. Kirchhoff has Paste In Place and Paste Inverted, Melda Invert; Bitwig and DMG a global
Shift; Pro-Q, EQ Eight, Bitwig and DMG a gain scale, which this EQ has had since v1 (research §17.4).

**The selection** (decision 44) is a set of bands and its **primary**, the one the Band card shows,
solo plays and the keyboard's MOVE edge takes. Editor state, never saved, never a parameter.
**Bands are ordered along the axis** by frequency, equal frequencies by slot number.
- **Mouse**: a click on a point selects it alone (as now); **Shift-click** adds a point to the
  selection, the primary moving to it, or takes it out, the primary then the selected band nearest
  it along the axis (of two as near, the lower); a **drag on the background** (not on the line, a point, a proposal or a
  peak, where the press already means something) draws a rectangle and selects the bands whose
  points it takes in, Shift adding them to the selection; a click on the background selects none
  (as now). **A click is egui's**: a press released before egui's drag threshold
  (`Response::clicked`, not `drag_started`); so a Shift-click toggles membership and writes no
  parameter and no undo step, and a Shift-drag past the threshold is a fine drag of the group, a
  tenth of the speed, as Shift on one band is now.
- **Keyboard**: **SELECT** with ← → adds the next band along the axis that is not selected yet,
  from the primary in that direction (with nothing selected, from the keyboard's place on the
  line), the primary moving to it; with none left that way it does nothing. So reversing direction
  passes over the bands already selected. The language's own meaning ("the selection, by extending
  it", mxm-keys). **Ctrl/Cmd+A** selects every band in use. **MENU** opens the primary's menu (as
  now) or, with nothing selected, the background's at the keyboard's place on the line.
- **A point's menu acts on the selection**: a right-click on a selected point makes it the primary
  and its menu acts on the whole selection; on a point not selected, it first selects that point
  alone, as a click would, so the menu acts on it and nothing else is touched. Every item applies to
  the selection: a shape set on every band (the card's rule), Delete, Copy, Cut, Paste and Invert
  gain.
- **Standard shortcuts, not the keymap's**: Ctrl/Cmd+A, C, X and V are, as in newDAWn, standard
  shortcuts outside the shared keymap (newDAWn's `crates/newdawn-ui/AGENTS.md`: "Ctrl/Cmd+C, X, V
  work on parts and notes"; its tests press "standard shortcuts" as keys), so the keyboard language
  and its one keymap are unchanged and nothing lands in one and not the other. The plugin's window
  (egui-baseview) delivers them as keys, as it does Ctrl/Cmd+Z, so the editor reads them where it
  reads undo (`handle_shortcuts`). **While a value is being typed** (`inert`) they are the text
  field's, not the selection's; a value typed into the Band card, entered, is a card edit like any
  other and moves the group (below).
- **The primary after a gesture**: it stays if it is still selected; else, after a rectangle or
  Select all, the first selected band along the axis; after SELECT, the band added; after DUPLICATE
  or a paste, the copy of the primary that was copied.
- **Drawn**: every selected point ringed as the selected one is now, the primary's ring twice as
  thick; the rectangle a dashed outline. Never a hue. The Band card's title reads "Band 3 + 2" with
  two more selected, its hover naming them.

**Relative editing** (decision 45): an edit of the primary, by drag, wheel, arrows or the Band
card, moves every selected band with it, so the curve keeps its shape:
- **frequency** by one ratio (the same distance on the log axis), **gain** by one offset in dB,
  **Q** by one factor; the change is the primary's, and **a band moves in a field only if it has
  that field**: gain the gain shapes, Q a band whose shape and slope have one (`has_q`). A gesture
  in a field the primary lacks (a vertical drag on a cut, the wheel on a slope without Q) moves
  nothing, as on one band now;
- **the group stops where its first member meets a limit** (10 Hz, this rate's top, ±30 dB, Q's
  range), so no member is squeezed against an end while the others go on;
- from the card, a **discrete** field is set on every selected band it applies to: shape and
  bypass on every one; slope on those whose shape has one; placement in a stereo layout; the
  side-chain switch on the dynamic shapes. The dynamics move as the continuous fields do, on the
  bands with dynamics (the gain shapes): **Range** and **Threshold** by one offset within their
  ranges (Range ±30 dB, Threshold its own), **Attack** and **Release** by one factor within theirs,
  the group stopping at its first member's limit;
- **§7.5's proposal pairs become a group**: a drag that applies a pair selects both, the dragged
  one primary, and they move as any group, by one gain offset, which is what `Drag.partner` does
  now (it goes, the selection doing its work);
- **one gesture is one undo step** however many bands it moves (§5.4's history already takes
  several writes in one gesture).
- What stays one band's: solo (the primary's), the keyboard's MOVE on an edge, F3's grab, the
  dynamic meter (the primary's).

**Delete and duplicate**: DELETE, Backspace or the menu's Delete removes every selected band, one
undo step; DUPLICATE copies every selected band into the lowest Off slots, refused when not all
fit, the copies selected so the arrows take them, as one band's copy is now.

**Copy and paste** (decision 46): **Ctrl/Cmd+C** copies the selected bands, every field (shape,
bypass, frequency, gain, Q, slope, the four dynamics, the side-chain switch, placement), in slot
order, and which of them was the primary; **Ctrl/Cmd+X** copies and deletes; **Ctrl/Cmd+V** pastes
them into the lowest Off slots, the copied bands in their slot order onto the free slots in theirs,
at their own values, the pasted bands then the selection, the copy of the primary its primary; one
undo step. **All or nothing**: when they do not all fit nothing is pasted, and the clipboard, the
selection and the history are as they were; the menu's Paste says why on its hover. DUPLICATE maps
its copies the same way and is refused the same way. The point's menu gains Copy, Cut, Paste and
Invert gain; a right-click on the background (or MENU with nothing selected) opens a menu with Paste
and Select all. With nothing selected, Copy, Cut and Invert gain are disabled, their hover saying
no band is selected; with nothing copied, Paste is, saying so. **The clipboard is the process's**
(a mutex-guarded static, editor threads only), shared by every instance as the registry is
(decision 41), so a band copied on one track pastes on another; a copy holds no slot numbers, so
pasting never overwrites.

**Global transforms** (decision 47) **come from the selection**: Select all, then a drag or the
arrows shift every frequency by one ratio (Bitwig's and DMG's Shift), move every gain by one offset
or every Q by one factor, keeping their relations, with the group's limits above; the gain scale
(§2.1) stays the one automatable global. The point's menu gains **Invert gain**, the selected bands'
gains turned over (Melda's Invert, Kirchhoff's Paste Inverted), one undo step. No Shift parameter.

**Costs**: none on the audio thread: a group's gesture writes more parameters, each one the host
already takes, the DSP gliding each as it does now.

**Not in F8**: soloing several bands at once (users' wish, research §17.3; the DSP's audition is one
band); snapshots; a library of band presets; a saved selection; the system clipboard; Paste
Inverted as its own command (Invert gain after Paste does it).

**Tests** (written at the end): a rectangle selects the points inside, Shift adding, the primary
by the rule above; Shift-click adding and taking out, the primary following (the lower of two as
near), writing nothing and no undo step; a Shift-drag a fine group drag; SELECT with ← → adding the
next unselected band along the axis (equal frequencies by slot), the primary the band added,
reversing passing over the selected, from the keyboard's place with nothing selected, nothing at
the end; a right-click on a selected point acting on the selection with it primary, and on an
unselected one selecting it alone first, its Delete and shapes touching nothing else; Ctrl/Cmd+A every band in use; a background click
selecting none; MENU with nothing selected opening the background's menu; a group's drag moving
every frequency by one ratio and every gain by one offset, stopping all at the first member's limit;
mixed shapes, a cut in a group keeping its gain and a 6 dB/oct cut its Q; the wheel, EXTENT and the
card's Q by one factor; the card's discrete fields on every selected band that has them, the
dynamics by offset and factor within their ranges; a proposal pair dragged as a group; each gesture
one undo step; DELETE and DUPLICATE on a selection, the copies in slot order onto free slots in
theirs, the copy of the primary primary, refused whole when they do not fit with the selection and
history unchanged; copy, cut and paste exact in every field, the same mapping, refused whole, one
undo step, from one editor into another in the process and from two threads at once; Invert gain;
the menus' disabled items and their hovers; Ctrl/Cmd+C, X, V, A doing nothing to the bands while a
value is typed, and a typed value moving the group; the selected and primary rings, the rectangle
and the title; the menus' items in the keyboard's reach; the opening size unchanged.

**Delivery** (tests at the end, one review over F8.1–F8.3):

| Step | Delivers | Gate |
|---|---|---|
| **F8.0** | Decisions 44–47, signed. **Done** (2026-10-16) | Owner |
| **F8.1** | The selection: the set and its primary, the mouse and keyboard gestures, the rings, the rectangle and the title; relative editing from the graph, the keyboard and the card, the group's limits; DELETE and DUPLICATE on a selection. **Built** (2026-10-16) | Unit tests at the end |
| **F8.2** | Copy, cut and paste through the process's clipboard; Invert gain; the point's and the background's menus. **Built** (2026-10-16) | Unit tests at the end |
| **F8.3** | The DOX: the plugin's `AGENTS.md` and `NOTES.md`, the index rows, this plan's status. **Built** (2026-10-16) | On Windows, the editor's change alone (no parameter, state, DSP or audio path moves, so neither the DSP's suites nor the host tests, the slow tier for audible changes): `cargo fmt --all -- --check`; `cargo clippy -p mxm-fx-eq-n --all-targets -- -D warnings`; `cargo test -p mxm-fx-eq-n`, the layout and painting tests among them; the bundle installed for F8.4; one agent-loop review of F8.1–F8.3 |
| **F8.4** | The owner in Bitwig: selecting, moving and copying several bands, between two tracks | Owner sign-off |

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
