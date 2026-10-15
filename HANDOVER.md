# Handover — mxm-fx-eq-n, 2026-10-15

Temporary: written when the owner moved to another machine for four days. Delete it once read; the
durable record is the plan, `plans/plan-mxm-fx-eq-n.md`.

## Where things are

- **This repository** (`mxm-audio/mxm-fx-eq-n`, private): everything is on `main`. The plan is at
  revision 27; the editor and DSP are as F3's approved code review left them (`09da9f5`).
- **mxm-kit**: the lock pins kit `ba76bd1` (relocked 2026-10-15). F3 and F4 change no parameters:
  the kit doc's 290 still holds.
- **The research repository** (`research:`, private): on the work machine it now lives at
  `Z:\newDAWn\research` (moved 2026-10-15 from `C:\Users\maxm\Documents\01-mxm-collection-research`,
  then fast-forwarded to `ffa5f4d`, with the 182 files that commit takes out of git kept on disk).
  Nothing local was committed there. On another machine, use its own checkout. F4's evidence is
  `research:effects/parametric-equalizers.md` §8–§9.

## State by phase

| Phase | Code | Waiting on the owner |
|---|---|---|
| v1 (E0–E4) | built, reviewed | **E5**: listening, by eye, a DAW check |
| F1 dynamic bands | built, reviewed | **F1.5**: listening, Bitwig |
| External side chain | built, reviewed | **SC.4**: a kick keying a bass's low bell, Bitwig |
| F2 stereo placement, Split | built, reviewed | **F2.4**: the image on speakers and headphones |
| F3 Freeze, grab, SC overlay | built, review **approved** (round 7) | **F3.4**: by eye and ear |
| F4 resonance finder | designed (§7.5), **plan review in progress** | decisions 27–30, once the review approves |
| F5–F8 | not started | — |

## F3, built 2026-10-15

Freeze (two seconds' maximum, peaks marked), grab (drag a peak or its label: a bell at its
frequency, Q from its width, the drag the gain, one undo step), the keyboard walk and ADD, and the
side chain's spectrum as a dashed "SC" line. Commits `cdf5309`, `298ae63`, `b486112`, `b3ec8f6`,
then seven review rounds' fixes to `09da9f5`. Four reviewer points were rejected; the owner can
overrule any of them:

- Freeze ends with the first audio after a reset, not at the reset itself (play, freeze, stop, grab).
- At Gain scale 0 % a grab's drag sets no gain, as every band's drag (every bell is flat there).
- A band's point under the pointer comes before a peak's marker or label.
- Only a width change ends a freeze; a height change leaves the held picture exact.

Also fixed from the owner's report: the band menu (MENU or right-click) now takes ↑ ↓ and Enter.

Open: the peak rule labels only narrow resonances (Q above about 3.7); widen its window if broad
bumps should be labelled too (owner's call).

## F4, the resonance finder: where the design stands

§7.5 and decisions 27–30, all **proposed**:

- **27** The finder proposes static cut bells the player applies; never continuous per-bin
  suppression (the line that keeps clear of oeksound's US 10,587,238; a product reading, not legal
  advice).
- **28** Find listens to the input before the EQ and keeps only peaks that stay put (persistence on
  short frames, width from long frames), placed where the excess is (L, R, M, S).
- **29** One Amount, 20 % by default (Grachten's median), −12 dB at most, Q from the width at half
  the prominence (an exact inverse at 100 %).
- **30** A second column of cards (Band | Output, Analyser, Resonances), the window 328 px wider.

**The plan review** (agent-loop, plan mode, reviewer gpt-5.6-sol):
- Round 1: four MAJORs, all accepted and applied (the channel policy; which frames vote on
  persistence; "treated" judged by the EQ as heard; separate evidence for movement and width).
- Round 2: three MAJORs, all accepted and applied (placement across L, R, M and S; persistence
  needs at least two different excitations; the proposals' depths solved together). The
  reviewer confirmed round 1's fixes. **Not approved yet**: round 3 is next. §7.5 grew to 122
  lines with the fixes; cut it back to its first length before round 3, every decision kept.

**To continue on another machine**: the review workspace (`.agent-loop/20261015-1450-f4-finder-plan/`)
is excluded from git, so start a fresh plan-mode `/agent-loop` on `plans/plan-mxm-fx-eq-n.md` §7.5,
the facts for `task.md` being the research sections above and the decisions here. Keep going until
it approves, then ask the owner to approve decisions 27–30, then build F4.1–F4.2 (§7.5's delivery
table), tests at the end, one code review.

## The owner's standing rules

The assistant's memory stays on the work machine; these are the ones that govern this work.

- **Scope**: *"as flexible and usefull as possible"*; Pro-Q is the gold standard, *"But I want it
  simple"*. The column sits right of the graph, never under it; the window may grow sideways.
- **Plans** are reviewed with `/agent-loop` until approved; plan mode never implements; the owner
  approves a plan's decisions before any code.
- **Code**, when a feature is done: one `/agent-loop` code review, iterated without asking until it
  approves; no edits while a review round runs; MINORs found at approval are fixed without another
  round.
- **Tests at the end**: keep a list while coding, write and run them once; only the touched crates'
  suites; no mutation checks.
- **Commit and push to main after every finished step**, if the product still works.
- **Every delete names a full drive-letter path** (`Z:/...`, not `/z/...`), delegation prompts too.
- **Ask first** before anything outward-facing: a new repository, a tag, pushing a public `main`
  (mxm-kit is public). This repository is private: pushing its `main` is fine.
- **No help text on the panel** (hover only); identity never rests on hue.
- **Visible progress** in the main thread; write plainly.
- A player or DAW the assistant started may be stopped and restarted with the same setup.

## Machine notes

- Work machine: workspace `Z:\newDAWn`, `CARGO_TARGET_DIR=Z:/newDAWn/target-shared`; the home
  machine's workspace is `E:\newDAWn`.
- Bundle: `cargo xtask bundle mxm-fx-eq-n --release` (installs the CLAP); copy it to this repo's
  `target/bundled/` before the host tests. A stale `xtask.exe` rebuilds after touching
  `xtask/src/main.rs`.
- Left on the work machine: MXM Player running with the F3 bundle; the side-chain renders in
  `Z:\newDAWn\target-shared\keyrender-out\`; the research update's backup in
  `Z:\newDAWn\target-shared\research-pull-backup\` (344 MB, deletable).
- On the work machine three research folders (`sources/listening`, `sources/physical-modelling`,
  `sources/shimmer-reverb`) fail their SHA256SUMS: local-only files that machine never had, or
  text files with other line endings. The machine that made those sums has them.
