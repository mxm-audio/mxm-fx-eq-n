# mxm-fx-eq-n — UI design brief

Answers `docs/MXM_DESIGN_SYSTEM.md` §14's ten questions before the editor is built, as
`plugins/AGENTS.md` requires. It also settles the product contract that `plans/plan-mxm-fx-eq-n.md`
gives this brief as E0: the shapes, slopes and ranges, the per-shape meanings, the gain scale and the
worst-case audio budget. The evidence base is `research:effects/parametric-equalizers.md`.

**Status: signed by the owner, 2026-10-12** (*"The brief looks good. Go ahead"*). The values once
marked **Proposal** are approved as written. The plan's owner rulings are not reopened here.

**Two of §14's questions are answered in adapted form, as the other effect briefs do.** Question 5
(software-added controls against a source panel) and question 9 (what is removed from the source
hardware layout) assume a copied machine. **This effect copies none.** Both are answered against the
benchmark, FabFilter Pro-Q, and against a conventional fixed-band EQ.

---

## A. The product contract (E0)

### A1. Identity and host surface

- `mxm-fx-eq-n`, CLAP id `dk.mxm.mxm-fx-eq-n`.
- Mono→mono and stereo→stereo, with the two channels processed identically in v1.
- Zero latency.
- **24 band slots** (owner, 2026-10-12). Each slot has permanent ids for type (Off plus the shapes),
  bypass, frequency, gain, Q and slope.
- Two global parameters: **Output gain** and the **Gain scale**.
- **Init is the empty EQ**: every slot Off, output 0 dB, gain scale 100 %. Inserting the plugin
  changes nothing.

### A2. v1 scope

**In v1:**
- bands made on demand by clicking the curve, or `item.insert` from the keyboard;
- eight shapes (A3);
- the animated spectrum, pre, post or both;
- the band column to the right of the graph;
- newDAWn's held-key keyboard with keymap K1;
- band solo;
- gain scale and output gain;
- plain undo and redo;
- presets.

**Later, in order (plan §7):** dynamic bands, stereo placement, spectrum grab, the resonance finder,
EQ match, phase modes, candidates such as saturation and auto-gain, then workflow (multi-selection,
copy and paste, global transforms).

### A3. Shapes and slopes — **Proposal**

| Shape | Slope |
|---|---|
| Bell | — (second order) |
| Low shelf, High shelf | — (second order; Q shapes the knee, A5) |
| Low cut, High cut | 6, 12, 18, 24, 36, 48, 72, 96 dB/octave |
| Notch | — |
| Band pass | — (second order) |
| Tilt shelf | — (first-order tilt about a pivot) |

That is Pro-Q's core set, without Pro-Q's Flat Tilt and its 30 dB/octave step. Brickwall cuts need
linear phase and come with phase modes (F6). The type and slope reserve fixed domains from this
first release (plan §2.1), so this list can grow without remapping saved projects.

### A4. Ranges — **Proposal**

| Control | Range | Default | Steps on the keyboard (rows coarse / normal / fine) |
|---|---|---|---|
| Frequency | 10 Hz – 30 kHz, held below Nyquist at the running sample rate | where it was created | octave / semitone / cent |
| Gain | −30 to +30 dB | 0 dB | 3 / 0.5 / 0.1 dB |
| Q | 0.025 – 40, logarithmic | 0.707 for shelves and cuts, 1 for bells | the rows' 10 % / 1 % / 0.1 % of travel |
| Output gain | −24 to +24 dB | 0 dB | 1 / 0.1 / 0.01 dB |
| Gain scale | 0 – 200 % | 100 % | 10 / 1 / 0.1 % |

Frequency, gain and Q follow Pro-Q's ranges. Output gain follows `mxm-fx-curve`'s Input gain.

### A5. What each control means, per shape — **Proposal**

The bell's Q is the half-gain-in-dB convention, under which a cut exactly cancels the equal boost
(research §3.2). Every other row is chosen to stay consistent with it.

| Shape | Frequency is | Q is | Gain is | Solo plays | A shape change keeps |
|---|---|---|---|---|---|
| Bell | the centre | the bandwidth at half the dB gain | the dB at the centre | a band-pass at the band's frequency and Q, so a 0 dB bell is the owner's narrow-Q sweep | frequency, gain, Q |
| Low / high shelf | the midpoint, where the shelf reaches half its dB gain (one of the three definitions in research §3.4) | the knee: 0.707 is flat, higher adds the resonant bump and dip | the plateau's dB | the region the shelf changes (low-pass or high-pass at its frequency) | frequency, gain, Q |
| Low / high cut | the corner (−3 dB when Q is 0.707) | the resonance at the corner, 0.707 is flat; a 6 dB/octave cut is first order and has none | — (the stored value is kept and ignored) | what the cut removes: the mirrored cut at its corner and slope | frequency, Q, slope |
| Notch | the centre | the width at −3 dB | — (full depth) | a band-pass at the band's frequency and Q | frequency, Q |
| Band pass | the centre | the width at −3 dB | — | the band pass itself | frequency, Q |
| Tilt shelf | the pivot | — (fixed) | the tilt: +gain at one extreme and −gain at the other | not offered: a tilt changes the whole spectrum | frequency, gain |

**The gain scale** multiplies every bell, shelf and tilt gain. At 0 % those are flat, while cuts,
notches and band passes stay as drawn (owner, 2026-10-12).

### A6. The worst-case audio budget — **Proposal**

**The worst v1 configuration:**
- 24 bands, each a cut at 96 dB/octave, which is 8 second-order sections;
- stereo, at 192 kHz;
- with an editor open, so the analyser handoff runs too.

At the research's 3.5–4 ns per section-sample (research §4.6), that is about 1.5 µs a frame, 29 % of
the 5.2 µs sample period, steady. With every band mid-fade it is about 3.1 µs, 59 %. Those are
derived figures; E1 measures them.

**Budget, release build, on the development machine:**
- **steady worst case: at most 35 % of one core at 192 kHz;**
- **all bands mid-transition at once: at most 70 %.**

**What it implies for E1's cramping fix.** Oversampling every section 2× would double the cost and
miss the steady budget. So whatever E1 chooses (plan §3.3) cannot oversample the steep cuts. It can
correct bells and shelves, the shapes that cramp audibly, and still fit.

---

## B. §14's ten questions

### 1. Primary sound-design task

**Shape a sound with exactly the bands it needs.** That means a precise cut or boost where one is
wanted, and finding and taming a resonance by ear with a solo sweep. It does not mean dialling a fixed
row of bands.

### 2. Controls reached for most

A band's **frequency, gain and Q**, on the graph or with K1. Then its **shape**, **solo**, the **gain
scale** and **output gain**.

### 3. Signal flow that must be visible

The bands run in series, and the drawn composite curve **is** the signal flow: it is evaluated from
the shipped DSP at the running sample rate (plan §3.1). The spectrum shows the input and output of
that flow, pre-EQ as an outline and post-EQ as a fill.

### 4. Play view

An effect has no view bar. The single surface is the play surface: the graph, with the column to its
right.

### 5. Advanced controls and disclosure (adapted)

**Nothing is disclosed behind a toggle.** The selected band's details are always in the Band card.
Slope appears only for the shapes that have one. The Analyser card holds the view settings. The
keyboard's 3×3 map appears only while a key is held. Against Pro-Q: no Instance List, no
collision display, no per-band stereo placement and no dynamics in v1. Those are later phases, not
hidden controls.

### 6. Categories, cards and grouping

- **The graph** is a named exception to the cards contract, after `mxm-fx-curve`'s canvas (plan §5.1).
- **The column,** top to bottom:
  - **Band:** number, bypass, shape, frequency, gain, Q, slope, solo, delete.
  - **Output:** output gain, gain scale, the Out meter.
  - **Analyser:** on/off, pre/post/both, resolution, speed, tilt.

  The column stays together. It never moves under the graph, and it scrolls vertically when it is
  taller than the space.

### 7. Identity accent

**Proposal:** the collection cyan, as `mxm-fx-curve` uses.
- **Contrast:** for the curve, the points and both spectra, measured in both themes with
  `mxm_ui::theme::contrast` at E3.
- **Band identity never rests on hue:** every point carries its slot number, and the selected one has a
  ring. The owner is red-green colour-blind.
- **Pre and post spectra** are told apart by outline against fill.

### 8. Live visualisation

- the composite curve, as the shipped response;
- each band's own contribution, drawn faintly;
- the animated spectrum, with the plan's resolution and reduced-motion rules (§4);
- the Out meter rail;
- the soloed band, marked on the graph.

### 9. What is removed, and why (adapted)

| Against | Removed | Why |
|---|---|---|
| Pro-Q | The floating band panel under the graph | The owner's ruling: a variable band count must not reflow a panel under the graph. The column replaces it |
| Pro-Q | Dry/wet Mix (Pro-Q has none either) | Mixing a minimum-phase EQ with dry comb-filters. The gain scale gives "less of this EQ" without that |
| Pro-Q | Linear and natural phase, dynamics, M/S placement, spectrum grab, EQ match | Later phases F1–F7, in order |
| A fixed-band EQ | A row of eight or twenty always-present bands | Only the bands that are needed, up to 24 |

### 10. Fit, reflow, zoom and Init

- **Opening size:** inside the quarter-4K budget, hugged, with the shared `opening_size` checks.
- **Minimum:** one card wide, never narrower than the app bar's last compact step.
- **Narrowing:**
  - the graph shrinks to its floor first;
  - below graph-plus-column width, the whole surface scrolls horizontally, with the column still
    on the right;
  - the column scrolls vertically when it is taller than the space.
- **Zoom:** the same rules hold at 150 % and 200 % zoom in the same physical window. The tested DPI
  scale is recorded at E3.
- **Init:** the empty EQ (A1).

**Factory presets — Proposal.** About a dozen named for the job, authored at E4. For example: a gentle
low cut, presence for a voice, de-mud, air, a tight kick, a tilt brighter, a tilt darker, and a
telephone band. The fifty-preset floor is for instruments.

---

## C. F1 — dynamic bands (addendum, signed 2026-10-14)

The plan's §7.1 is the design; this is what the owner signed for it (2026-10-14, *"Approve, values as
proposed"*), after decisions 12–14.

**Which bands.** Bells and shelves. Cuts, notches and band passes have no gain; the tilt lifts one side
of its pivot and cuts the other, so it stays static.

**Ranges and defaults.**

| Control | Range | Default |
|---|---|---|
| Range | −30 to +30 dB; below 0 cuts what gets loud, above 0 lifts it | 0 (the band is static) |
| Threshold | −60 to 0 dB | −20 dB |
| Attack | 0.5 to 200 ms | 10 ms |
| Release | 10 to 2,000 ms | 150 ms |

The defaults of Threshold, Attack and Release are proposals the owner did not sign; F1.5's listening
confirms or changes them.

**The law.** Above Threshold, through a 6 dB soft knee, the band's gain moves at a fixed slope — 3:1
for a cut, 1:1.5 for a lift — and eases into Range: with `a` the movement that slope asks for, the
moving part is `Range · (1 − e^(−a/|Range|))`. It never passes Range, a louder band never comes out
quieter, and more Range never means less action.

**The bound.** The heard gain, the drawn gain plus the moving part, stays inside the band's ±30 dB at
every sample; the gain scale applies after it.

**Factory presets.** *De-esser* and *Resonance tamer* join the thirteen.

---

## Sign-off

- [x] A3 shapes and slopes (owner), 2026-10-12
- [x] A4 ranges and keyboard steps (owner), 2026-10-12
- [x] A5 per-shape meanings, including what solo plays and what a shape change keeps (owner), 2026-10-12
- [x] A6 worst-case budget (owner), 2026-10-12
- [x] B7 accent: the collection cyan (owner, 2026-10-12); contrast measured at E3
- [x] Factory preset direction (owner), 2026-10-12
- [ ] §15 QA gate on the real panel (E5)
- [x] C, F1's addendum: bands, ranges, the law (3:1, 1:1.5, 6 dB knee), the bound, the presets' names (owner), 2026-10-14
