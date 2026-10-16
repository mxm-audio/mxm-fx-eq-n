# AGENTS.md — mxm-fx-eq-n

DOX rail for this repository. Project-wide instructions, durable workflow rules, and the
top-level Child DOX Index.

---

# DOX framework

- DOX is a highly performant AGENTS.md hierarchy installed here
- Agents must follow DOX instructions across any edits

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees
- Work products, source materials, instructions, records, assets, and durable docs must stay
  understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it

## Read Before Editing

1. Read the root AGENTS.md
2. Identify every file or folder you expect to touch
3. Walk from the repository root to each target path
4. Read every AGENTS.md found along each route
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and
   continue from there
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX

Do not rely on memory. Re-read the applicable DOX chain in the current session before editing.

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning AGENTS.md when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child index changes. Update
child docs when parent changes alter local rules. Correct stale or contradictory text immediately, and move its history to `NOTES.md` rather than deleting it; a discarded idea or replaced design is deleted outright, not archived: git keeps it (the owner, 2026-10-08).
Small edits that do not change behavior or contracts may leave docs unchanged, but the DOX pass
still must happen.

## Hierarchy

- Root AGENTS.md is the DOX rail
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index
- Each parent explains what its direct children cover and what stays owned by the parent
- The closer a doc is to the work, the more specific and practical it must be

## Child Doc Shape

- Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules,
  responsibilities, workflow, materials, or quality standards
- Work Guidance must reflect current project standards or user instructions; leave it empty if
  there are none yet
- Verification must reflect an existing check; leave it empty until one exists

Default section order: Purpose · Ownership · Local Contracts · Work Guidance · Verification ·
Child DOX Index

## Style

- Keep docs concise, current, and operational
- Document stable contracts, not diary entries
- **A value the code holds is named, not copied** (the owner, 2026-09-24: *"Why are you writing the
  opening pages size in such detail. Is that not already described in the code?"*). A size a test
  derives — an opening size, a minimum, a card floor — or a constant the code declares is stated in
  DOX as its rule, its constant and the test that holds it, never its number: a copied number goes
  stale the day the code moves. Two exceptions: the design system states its own tokens and
  rules, because it is the normative source the code implements; and a plan's revision history
  records what was measured when — a dated record, not the current value
- Put broad rules in parent docs and concrete details in child docs
- Prefer direct bullets with explicit names
- Do not duplicate rules across many files unless each scope needs a local version
- Keep AGENTS.md current: correct a stale note and move its history to `NOTES.md` instead of explaining it here;
  a discarded idea is deleted, never kept as history
- Trim obvious statements, repeated rules, misplaced detail, and warnings for risks that no longer
  exist

## Closeout

1. Re-check changed paths against the DOX chain
2. Update nearest owning docs and any affected parents or children
3. Refresh every affected Child DOX Index
4. Correct stale or contradictory text; move the reasoning still in force to `NOTES.md`, and delete a
   discarded idea outright
5. Run existing verification when relevant
6. Report any docs intentionally left unchanged and why

---

---

# Purpose

**mxm-fx-eq-n** is an MXM effect: a parametric EQ with only the bands you need, up to 24, each
matched to its analogue curve right up to the top of the range, with an animated spectrum. Its
contract is [`plans/plan-mxm-fx-eq-n.md`](plans/plan-mxm-fx-eq-n.md) and the signed brief
[`docs/briefs/mxm-fx-eq-n.md`](docs/briefs/mxm-fx-eq-n.md).

It is one of the MXM products, each in its own repository under
[github.com/mxm-audio](https://github.com/mxm-audio), built on the MIT-licensed
[mxm-kit](https://github.com/mxm-audio/mxm-kit) — the design system, keyboard navigation, presets,
modulation, the control map and the checks every plugin shares. Until 2026-10 all of it was one
repository (`mxm-collection`). This product was begun there after the split by mistake and moved
here on 2026-10-13; its own `plans/` and `docs/` carry what it was built from. A reference to
another `plans/plan-…` file names the archive's design history.

Reference-quality open source: clarity beats cleverness, and every nontrivial algorithm names the
technique or paper it comes from.

# Ownership

Root owns `Cargo.toml`, `Cargo.lock`, `LICENSE`, `NOTICE.md`, `TRADEMARKS.md`, `README.md`,
`CONTRIBUTING.md`, `.cargo/`, `.github/`, `bundler.toml`, `test-bundles.txt` and `xtask/`;
`plans/` holds this product's plan, and `docs/briefs/` its brief. `test-bundles.txt` names the
plugins from other repositories that the host tests load: `mxm-mono-01`, the source they play.
Each folder with an `AGENTS.md` owns its contents; the index is below.

**Dependencies follow each repository's `main`, and `Cargo.lock` pins the exact commit.** The kit comes from mxm-kit's
`main`, another product's crates from its repository's `main`, and nice-plug and
egui-baseview from their MXM forks (`[patch.crates-io]`).

**Two tiers of tests.** `cargo test` builds the plugin and its DSP only — the loop for a
change. `plugins/mxm-fx-eq-n/host-tests` loads the release bundle through MXM Player: it is a
separate package, outside `default-members`, so the fast tier never builds the player. **It reads
this repository's own `target/bundled`**, so bundle without `CARGO_TARGET_DIR`: a bundle built
into a shared target lands elsewhere, and every host test skips.

## Windows, Linux and macOS — all three, always

**An absolute requirement.** Everything here runs on all three; a change that works on one and
breaks another is a broken change. CI builds and tests on all three, when started by hand (the owner, 2026-10-06); before a push, Windows and Linux are checked
locally (*Verification*).

- **Anything platform-specific is `cfg`-gated with every arm implemented**, never one arm and a
  silent nothing elsewhere.
- **Linux needs system libraries** the other two carry in their SDKs — ALSA (and JACK) for audio,
  and X11, xkbcommon and a GL loader for the window.
- **A dependency that does not support all three cannot be taken**, whatever else it offers.

## MSRV is per crate

| Crate | MSRV | Why |
|---|---|---|
| `crates/mxm-fx-eq-n-dsp` | **1.87** | One runtime dependency, mxm-kit's `mxm-fft` (MSRV 1.87, for Linear phase; decision 6): the matched SVF bands, transitions, solo, the series cascade and F6's linear-phase convolver. Measurement tools are dev-only. **Verified on 1.87**, library, tests and examples |
| `plugins/mxm-fx-eq-n` | **1.95** | Its editor pulls in egui; depends on `mxm-fx-eq-n-dsp`, which stays at 1.87, and mxm-kit's `mxm-fft` (1.87) for the analyser |

## Licensing

**GPL-3.0-or-later** (`LICENSE`). `NOTICE.md` lists the third-party code in its builds. The MXM
name and logo are not covered by the licence: see `TRADEMARKS.md`.

- **MPL-2.0 is accepted** for symphonia, through `mxm-audio-file-decode` only, used unmodified;
  never vendor, patch or modify an MPL crate.
- Check the licence before porting any algorithm, and record source and licence in a comment at
  the top of the file. Cite techniques even when the implementation is original.

## Research citations

A citation written `` `research:<path>` `` names a page in MXM's private research repository. It
is plain text in a code span, never a link, and nothing here depends on it at build or test
time. Facts, numbers, our own measurements and short quotations cross into this repository;
third-party files, images and verbatim text never do.

# Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test                                   # the fast tier: the plugin and its DSP
cargo xtask bundle mxm-fx-eq-n --release      # also installs it in this user's CLAP folder
cargo xtask fetch                            # plugins from other repositories its tests load
cargo test -p mxm-fx-eq-n-host-tests         # the slow tier: through MXM Player
```

Before a push, run the first three on Windows; Linux and macOS are checked
later, together (the owner, 2026-10-06). CI runs the same on Windows, macOS and Linux, only when started by hand (the owner, 2026-10-06), so only CI reaches macOS.

# Child DOX Index

| Doc | Scope |
|---|---|
| [`crates/mxm-fx-eq-n-dsp/AGENTS.md`](crates/mxm-fx-eq-n-dsp/AGENTS.md) | The matched SVF bands, the cramping choice, transitions and parking, solo, precision and the measured cost |
| [`plugins/AGENTS.md`](plugins/AGENTS.md) | Shared plugin conventions: nice-plug, the init patch, presets, `process()` rules, the editor contract, and installing a bundle in a DAW |
| [`plugins/mxm-fx-eq-n/AGENTS.md`](plugins/mxm-fx-eq-n/AGENTS.md) | Identity, the 293 parameters and their stepped domains, F1's dynamic bands and their side chain, F2's stereo placement, the empty-EQ Init, solo, the analyser and meter, F3's freeze and grab, F4's resonance finder, F5's EQ match, F6's phase modes and latency, F7's auto gain and other tracks' spectra, F8's work with several bands, the graph-and-columns editor, the graph as a surface, plain undo, the factory set, the host tests |
