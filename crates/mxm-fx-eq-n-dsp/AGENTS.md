# AGENTS.md — mxm-fx-eq-n-dsp

Parent: [`../../AGENTS.md`](../../AGENTS.md). The detail behind the rules: [NOTES.md](NOTES.md).

# Purpose

Framework-free, zero-runtime-dependency DSP for the original `mxm-fx-eq-n` effect: up to 24
parametric bands in series, each a trapezoidal SVF whose design is matched to its analogue
prototype, with click-free transitions, parking, band solo, dynamic bands (F1), the gain scale and
the output gain.

The product contract is [`../../plans/plan-mxm-fx-eq-n.md`](../../plans/plan-mxm-fx-eq-n.md) §2–§3
and §7.1 (F1) and [`../../docs/briefs/mxm-fx-eq-n.md`](../../docs/briefs/mxm-fx-eq-n.md) A1–A6 and §C; the evidence base is
`research:effects/parametric-equalizers.md`. No existing equaliser's code was opened.

# Ownership

- `src/design.rs`: shapes and slopes, the matched and bilinear designs, the analogue prototypes,
  solo designs and response evaluation. The editor draws from it.
- `src/filter.rs`: the running sections, their per-sample parameter ramp and their state.
- `src/dynamics.rs`: a dynamic band's law, its knee, its ballistics and its settings' ranges.
- `src/engine.rs`: the bands, glides, transitions, parking, solo, dynamics, gain scale, output gain
  and activity.
- `tests/design.rs`: meanings and analogue match; `tests/engine.rs`: plan §8's audio properties;
  `tests/dynamics.rs`: plan §7.1's; `tests/realtime.rs`: the allocation guard.
- `examples/`: `eq_n_cramping`, `eq_n_precision`, `eq_n_modulation`, `eq_n_tail`, `eq_n_cost`,
  `eq_n_dynamics`, the
  measurements in [NOTES.md § Measured and chosen facts](NOTES.md#measured-and-chosen-facts).

Does **not** own parameters, ids, ranges as host parameters, smoothing styles, telemetry, the
analyser or the editor: those are `plugins/mxm-fx-eq-n`'s (E2, E3). It owns the brief's ranges as
constants, because the DSP holds every value inside them whatever arrives.

# Local Contracts

## Every section is an SVF, and its design is matched — [NOTES.md § Why](NOTES.md#why-every-section-is-an-svf-and-why-its-design-is-matched)

- **Simper's trapezoidal SVF** (or its one-pole), `g`, `k` and the mix `m0·x + m1·bp + m2·lp`.
- **`Method::Matched` ships** (Vicanek 2016): impulse-invariant poles, a numerator fitted to the
  analogue magnitude in `Ω = tan(ω/2)/g`. `Method::Bilinear` exists only to rerun the measurement
  that rejected it.
- **Every pole is at or below its band's frequency**, so there is no regime boundary under
  automation; a bell or low-shelf cut is the inverted boost and a high-shelf boost the inverted
  cut, so **a cut cancels the equal boost exactly**.
- **The fit point is continuous** (`At::new`); bells and band passes keep Vicanek's extremum, their
  peak on their frequency. **An inverted section's zeros are kept off the axis** (`invertible_fit`).
- **The drawn curve is the heard curve**: the editor calls `design` and `Cascade::response_db`; at
  exactly 0 dB the engine runs `design_running`, so a fade through 0 dB keeps its section count.

## Meanings (brief A5), as the tests hold them

- A bell reaches its gain at its centre and half of it at its half-gain band edges, at every gain
  (Miller's probe); its peak is on its frequency up to 19 kHz at 44.1 kHz.
- A shelf is exactly half its gain at its frequency and its full gain at its plateau.
- A cut is −3.01 dB at its corner at Q 0.707 and **Q itself at the corner** for every order above
  the first (Q scales the least damped Butterworth pair); a 6 dB/octave cut is first order and has
  no Q. Stopbands follow the Butterworth table.
- A tilt is 0 dB at its pivot and −gain at DC; the notch is silent at its centre, the band pass
  unity there.
- Solo plays a band pass at the band's frequency and Q for a bell, notch or band pass, a low or high
  pass at Q 0.707 for a shelf, the mirrored cut for a cut, and nothing for the tilt.

## Values glide, structures fade, and identity costs nothing — [NOTES.md § The detail](NOTES.md#values-glide-structures-fade-and-identity-costs-nothing-the-detail)

- **Glides** in log frequency, dB gain and log Q, over `SMOOTHING_SECONDS`, stepped once per
  `CONTROL_SECONDS` interval that **runs on across calls**: the output is bit-identical however a
  host splits its blocks.
- **Work only where something moves**: `set_band` reads targets only when they change, a band is
  redesigned only when a value its shape hears moves, and `is_settled` says nothing audible is
  moving or waiting to.
- **Ramps**: `g`, `k` and the mix move linearly sample by sample (`Filter::ramp_to`), every
  section's coefficients derived before the signal passes through any.
- **Structural changes** fade over `FADE_SECONDS`, a gain shape by its gain from or to 0 dB and
  anything else by a crossfade; a change during a fade waits, and the newest wins.
- **Parking**: an Off or bypassed band, and a gain shape settled at exactly 0 dB, run nothing and
  re-enter as creation. Everything parked, no audition and the output at 0 dB: **bit-exact dry**.
- **The output gain ramps from the sample it is set at**; **a solo audition fades in from the
  interval it is built in**.
- **Reset** finishes every transition and clears every state, so a saved EQ is heard from its first
  sample; it also ends solo.
- **Solo** is honoured only while the band is live and offers it; it runs on the dry input, follows
  the band, and fades in and out.

## Dynamic bands (F1) — [NOTES.md § Measured](NOTES.md#dynamic-bands-measured)

The design is plan §7.1, the signed values brief §C; `dynamics.rs` holds them as constants.
- **Bells and shelves only** (`Shape::is_dynamic`); `Engine::set_dynamics` per slot, read at the next
  interval; **Range 0 is the v1 band**, bit for bit.
- **The law** (`dynamics::law`): 3:1 for a cut, 1:1.5 for a lift, a 6 dB knee, easing into Range;
  monotonic in level and in Range, never past it.
- **The detector** is the band's solo design, its own state, on the EQ's **input**: its loudest
  sample each control interval, linked across channels, is the level. It follows the band's
  frequency and Q. So the gains never depend on slot order.
- **The ballistics** move the signed moving part in dB: Attack away from rest, Release back or
  across it; a release ends at exactly 0 below `REST_DB`, a move toward any other target stops
  within `LAND_DB` of it. Range glides as the gain does. A listening band is settled when its last
  step did not move it and its peak asks for no move: once an interval is complete any move, before
  that only an attack.
- **The bound**: the heard gain, the drawn gain plus the moving part, is held inside ±30 dB each
  interval, before the gain scale; `Engine::gains_db` gives the editor the drawn and heard gains,
  and `Engine::gain_scale` the scale where it has glided, which they are heard under.
- **A band that listens never parks**: Range or a moving part not 0, and the gain scale not 0 %;
  otherwise it parks and re-enters as v1 bands do.
- **Activity**: a moving part keeps `is_ringing` true, so a host's sleep starts from rest.
- **The side chain** (plan §7.2): `process_stereo_keyed` and `process_mono_keyed` take the key, and
  `set_side_chain` points a slot's detector at it; the old calls pass none, and a keyed band with no
  key hears silence. A switch starts the detector fresh, never steps the gain. The key is read only
  while something hears it, so with no band keyed the output is F1's, bit for bit; while it is read
  it counts as input, so a key over a silent input ends no tail. Solo plays what its band listens
  to; a source switch fades the audition out and back in on the new source, as a shape change does.

## Stereo placement (F2) — plan §7.3

- `Engine::set_placement` puts a slot on Stereo, Left, Right, Mid or Side (`Placement`); the
  placement is part of the band's structure, so a change fades.
- **Two stages** in `render`: every Stereo, Left and Right arm, then — only while a band has a Mid
  or Side arm — every Mid and Side arm on `M = (L + R)/2`, `S = (L − R)/2`. Within a stage the bands
  commute, so slot order never matters; with every band on Stereo the second stage never runs and
  the output is v1's, bit for bit. A crossfade within a family mixes its arms in their stage; across
  the boundary each arm fades against identity in its own (`tick`'s `mid_side`), and a fade counts
  down once a sample in `advance`.
- A placed arm runs one channel of its filter; in mono it does nothing. Its detector reads its own
  channel of the input or the key, a Stereo band the linked peak (decision 20); solo plays the
  band's channel where it sits in the image.
- `Cascade::response` is the complex response the editor combines into each channel's line.

## Numeric and realtime contracts — [NOTES.md § Reasons](NOTES.md#the-numeric-contracts-reasons)

- `f64` coefficients and state, `f32` only at the buffers; denormals flushed on every state write;
  **no processed output sample is subnormal**; the dry path passes input bit for bit.
- A non-finite input is silence in the wet path; a non-finite output is zero and clears every
  state; **`Filter::process` recovers on its own**.
- `OUTPUT_BOUND` holds the output finite; every value is held in the brief's ranges in the DSP
  itself; **every public entry point** takes its rate through `usable_sample_rate`.
- **Activity**: `is_ringing` while any state is nonzero; after an interval of silent input every
  state under `QUIET_FLOOR` is cleared, so a host can sleep the plugin; `is_dry` when untouched.
- `process_*` allocates, locks and blocks on nothing (`tests/realtime.rs`); the engine is one
  fixed-size value, boxed by the plugin at activation.

# Work Guidance

- Do not add framework, GUI, parameter, telemetry or analyser code here.
- A design change reruns `eq_n_cramping` and the design tests; a per-sample change reruns
  `eq_n_cost` on an idle machine; replace the figures in NOTES.md rather than adding to them.
- A new shape names its prototype in `analogue`, its fit, its solo and its meanings, and gets a row
  in the design tests and the cramping table.
- Do not extract the SVF or the designs until a second shipped implementation needs the same API.

# Verification

```bash
cargo test -p mxm-fx-eq-n-dsp
cargo clippy -p mxm-fx-eq-n-dsp --all-targets
cargo +1.87.0 test -p mxm-fx-eq-n-dsp
cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_cramping
cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_precision
cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_modulation
cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_tail
cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_cost
cargo run -p mxm-fx-eq-n-dsp --release --example eq_n_dynamics
```

What the tests cover: [NOTES.md § What the tests cover](NOTES.md#what-the-tests-cover).

# Child DOX Index

No child AGENTS.md files.
