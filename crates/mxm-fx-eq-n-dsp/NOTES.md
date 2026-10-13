# NOTES.md — crates/mxm-fx-eq-n-dsp

The detail behind this folder's AGENTS.md: the reasoning behind each contract, the measurements and
what the tests cover. AGENTS.md is the contract; this file is the reference it links to.

## Why every section is an SVF, and why its design is matched

- **The structure is Simper's trapezoidal SVF** (or its one-pole), five numbers a section: `g`,
  `k` and the mix `m0·x + m1·bp + m2·lp`. Any stable biquad is one with `g > 0`, `k > 0`, so the
  matched designs lose nothing, and the SVF keeps its proven stability under per-sample changes of
  `g` and `k` (Wishnick 2014; McClellan 2026).
- **The shipped design is `Method::Matched`**, after Vicanek 2016: impulse-invariant poles and a
  numerator fitted to the analogue magnitude, solved directly for the mix in the warped variable
  `Ω = tan(ω/2)/g`. `design.rs`'s module comment gives each shape's fit. `Method::Bilinear`, the
  collection's SVF table, exists only so the measurement that rejected it can be rerun.
- **Every impulse-invariant pole is at or below its band's frequency**, so there is no regime
  boundary under frequency automation. That is why a bell or low-shelf cut is the inverted boost and
  a high-shelf boost the inverted cut: the inverse is exact, so **a cut cancels the equal boost to
  1e-8 dB** for bells, shelves and the tilt, the brief's half-gain-in-dB promise.
- **The fit point is continuous.** Shelves, low-pass sections, first-order sections and the tilt
  meet their analogue value a last time at 20 kHz, or halfway from the band to Nyquist above about
  18 kHz (`At::new`). Fitting at Nyquist itself left each cut section about a decibel high in its
  passband, and a 96 dB/octave cascade 8 dB high. Bells and band passes keep Vicanek's extremum
  instead, so their peak sits exactly on their frequency; fitting them at the top too moved a Q 1
  bell's peak 37 cents at 18 kHz.
- **An inverted section's zeros are kept off the axis** (`invertible_fit`): Nyquist gain no more
  than 6 dB under the analogue value, damping no less than Q 40's. Without that floor a +60 dB, Q 6
  low shelf near 3.6 kHz put a zero on Nyquist and its cut a pole on the unit circle.
- **The drawn curve is the heard curve.** The editor calls `design` and `Cascade::response_db`, the
  sections the audio runs; `tests/engine.rs` measures the engine against them to 0.02 dB. At
  exactly 0 dB a gain shape draws as no sections; the engine runs `design_running`, which keeps the
  section at the identity mix, so a fade or glide through 0 dB never changes the section count.

## Values glide, structures fade, and identity costs nothing: the detail

- **Glides**: log frequency, dB gain and log Q, a one-pole with `SMOOTHING_SECONDS` time constant
  carried as the remaining distance (it lands exactly), stepped once per control interval
  (`CONTROL_SECONDS`, so the same in time at every rate). **The control interval runs on across
  calls**: settings are read at its start wherever a host's blocks fall, so the output is
  bit-identical however the host splits them, one-frame blocks included.
- **Work only where something moves.** A band's targets are read, and their logarithms taken,
  only when `set_band` changes them; it is redesigned only when a value its shape hears moves (a
  cut's stored gain or a tilt's Q may glide unheeded); a ramp to where a filter already is does
  nothing; a soloed audition is redesigned only when its band's frequency moves, or its Q for the
  shapes whose audition hears it (`solo_hears_q`). `is_settled` says nothing audible is moving or
  waiting to — no unread setting, pending structure, fade, audible glide, gain-scale glide a gain
  shape hears, section ramp, solo change or output ramp — so every section runs on its
  precomputed coefficients.
- **Ramps**: a moving section's `g`, `k` and mix move linearly sample by sample across the interval
  and its coefficients are derived from them every sample (`Filter::ramp_to`; plan §3.4). Each
  sample derives every section's coefficients before the signal passes through any of them, so
  the divisions run side by side rather than waiting on the signal chain. Stepping per interval
  instead was a zipper the click test measures at 2.6× a tone's largest step; ramped it is 1.10×
  at 48 kHz, 1.07× at 96 and 1.25× at 192, where the fades' own modulation is the figure.
- **Structural changes** (shape, slope, bypass, Off) fade over `FADE_SECONDS`. A gain shape enters
  and leaves by ramping its gain from or to 0 dB, its state warming up on the way in; anything else
  crossfades. A fade's last interval ramps over only the samples the fade has left, so a band is
  never dropped with its coefficients still on the way (at 44.1 kHz the fade is not a whole number
  of intervals). A change during a fade waits, and the band rereads its target when the fade ends, so
  the newest wins and at most one transition is ever outstanding.
- **Parking**: an Off or bypassed band after its fade, and a gain shape settled at exactly 0 dB
  (the gain scale's 0 % included), run nothing. A parked band's state is cleared, and leaving
  parking is creation again: from silence, through the entry fade. With every band parked, no
  audition and the output at exactly 0 dB, `process_*` leaves the buffer untouched — bit-exact dry,
  NaN and infinity included.

- **The output gain ramps from the sample it is set at**, empty EQ or not: scheduling the ramp
  ends the interval's dry shortcut. **A solo audition starts fading in in the interval it is
  built in.**
- **Reset** finishes every transition at once and clears every state, so activation and a host
  reset hear a saved EQ from its first sample; it also ends solo.
- **Solo** is a request the engine honours only while the band is live and offers solo. The
  audition runs on the dry input, follows the band's values, and fades in and out over a fade; a
  change of the band's structure fades it out and rebuilds it from silence.

## The numeric contracts' reasons

- `f64` coefficients and state; `f32` only at the buffers. Denormals are flushed on every state
  write, and **a processed output sample is never subnormal**: a value normal in `f64` can be
  subnormal once cast to `f32`, so the cast flushes it (`processed_output_is_never_subnormal`;
  found by `clap-validator`'s parameter fuzz at E2). The untouched dry path passes input bit for bit. Non-finite input becomes zero inside the wet path; a non-finite output sample is replaced
  by zero, and every state is then cleared at that sample, because an overflowed recursion never
  recovers. **`Filter::process` recovers on its own** for any caller: a non-finite input is
  silence, and a non-finite output clears the filter.
- `OUTPUT_BOUND` (+24 dBFS) holds the output finite under settings that ask for far more; no
  reasonable setting reaches it. Every value is held inside the brief's ranges in the DSP itself
  (`Design::bounded`, `BandEngine::targets`), and **every public entry point** — the engine and
  every design and response function — takes its sample rate through `usable_sample_rate`, so a
  rate below `MIN_SAMPLE_RATE` or not finite is that minimum and no clamp's bounds can cross.

- **Activity**: `is_ringing` is true while any state is nonzero. After a whole control interval of
  silent input, every state is cleared once all are under `QUIET_FLOOR`, so a host can sleep the
  plugin on real silence; `is_dry` says the engine is touching nothing.
- `process_*` allocates, locks and blocks on nothing (`tests/realtime.rs`); the engine is one
  fixed-size value of tens of kilobytes, which the plugin boxes at activation.

## Measured and chosen facts

All measured on the development machine (AMD Threadripper 3970X, Windows, rustc 1.98, release),
2026-10-13. Rerun the example and replace a figure; do not append.

- **Cramping** (`eq_n_cramping`), largest error to 20 kHz against the analogue prototype,
  bilinear / matched, dB:

  | Case | 44.1 kHz | 48 kHz |
  |---|---|---|
  | Bell +12 dB, Q 1 at 10 / 15 / 18 kHz | −3.82 / −7.48 / −7.44 → 0.84 / 0.75 / 0.74 | −3.30 / −6.07 / −4.79 → 0.53 / 0.41 / 0.45 |
  | Low shelf +12 dB at 10 kHz; high shelf −12 dB at 15 kHz | −1.57 / −3.26 → −0.11 / 0.19 | −1.34 / −2.81 → −0.05 / 0.09 |
  | High cut 18 dB/oct at 12 kHz; 48 dB/oct at 15 kHz | −32.8 / −71.5 → −0.51 / 0.82 | −20.8 / −43.5 → −0.25 / 0.37 |
  | High cut 96 dB/oct at 18 kHz | −82.8 → 4.11 | −45.8 → 2.16 |
  | Notch Q 8 at 15 kHz; band pass Q 2 at 10 kHz | 8.08 / −13.8 → −0.05 / 1.32 | 6.55 / −9.43 → −0.03 / 0.84 |
  | Tilt +12 dB at 10 kHz | 5.90 → 0.32 | 4.67 → 0.19 |

  The bilinear bell and shelf figures reproduce the research's (§4.2) to the hundredth, and the
  matched bell's are Vicanek's there (§4.3). Over the ordinary range — 20 Hz to 16 kHz, Q 0.3 to
  4, ±24 dB, every slope — the matched worst case at 44.1 kHz is about 1.3 dB for bells, 2 dB for
  cuts (the steepest, near the top) and 1.6 dB for shelves at Q 2. **Two known limits**: a shelf
  with a strongly resonant knee (Q above about 3) at large gain misplaces its dip, 4.7 dB at Q 4
  and 11 dB at Q 10 for ±24 dB, because three fit points cannot place a sharp dip whose zeros sit
  near the top; and a very wide notch (Q 0.3) near the top is up to 6 dB off. The bilinear design
  is worse in every one of those cases (by 7 to 24 dB), and the drawn curve shows them as heard.
- **Precision** (`eq_n_precision`): the `f64`-state SVF reproduces the designed gain exactly on the
  research's low-frequency cases (20 Hz Q 10 +12 dB and 10 Hz Q 20 +6 dB at 192 kHz). The same
  sections with `f32` state are within 0.003 dB, with round-off 106 to 133 dB below the output.
  **`f64` was chosen**: it is the repository's default for recursive state, and in a serial
  cascade it is no slower (research §4.6).
- **Modulation** (`eq_n_modulation`): switched every 64 samples between 300 Hz and 3 kHz, a Q 8,
  +18 dB bell peaks at 0.91× its static peak on the same noise, 0.69× through the engine's glides.
  Square-wave gain of ±15 dB every 64 samples puts −29.3 dB above 5 kHz (a fixed-core bell,
  poles still and only a mix gain moving: −26.8 dB). **For F1's dynamic bands**: envelope-rate
  gain, 0 to −12 dB with a 1 ms attack and 60 ms release, puts −89.7 dB above 5 kHz against the
  fixed core's −92.6 dB, so F1 can keep this bell.
- **Click-free transitions** (`tests/engine.rs`): shapes, slopes, bypass and Off changing every 7 ms
  on three bands give a largest step 1.10× the largest a tone of the output's peak makes at 48 kHz,
  1.07× at 96 and 1.25× at 192 (bound 1.6; a one-sample fade measures 75×); a parked band
  re-entering and solo fading both 1.00×.
- **Tails** (`eq_n_tail`, 48 kHz): quiet after 0.01 s for a 1 kHz Q 1 bell, 0.5 s for 100 Hz Q 4,
  6.3 s for 20 Hz Q 10 +12 dB and 136 s for 10 Hz Q 40 +30 dB — the filter's own ring to −200 dB.
- **Cost** (`eq_n_cost`), the brief's worst case, 24 bands of 96 dB/octave cuts, stereo, per frame
  as a share of one core, three runs. *Crossfading* switches every band between a 96 dB/octave
  low and high cut at each fade's end, so every band always runs two eight-section filters;
  *crossfading and gliding*, **the transition worst case and A6's acceptance figure**, also moves
  every band's frequency every control interval, so the new bank redesigns and ramps throughout:

  | Load | 44.1 kHz | 192 kHz | Budget (A6, 192 kHz) |
  |---|---|---|---|
  | Steady | 4.4–4.5 % | 19.4–20.6 % | 35 % |
  | Every band crossfading | 8.8–9.8 % | 38.1–42.3 % | 70 % |
  | Every band crossfading and gliding | 15.1–18.1 % | 51.2–57.2 % | 70 % |
  | Every band gliding | 10.3–13.4 % | 31.8–36.7 % | 70 % |

  Deriving every section's coefficients before the signal passes (`filter.rs`), and taking
  `tan(w/2)` once per band, brought the worst case down from 69.4–75.6 % — over the budget — and
  the steady case from about 29 %. Typical work is far below the budget: 24 parked 0 dB bells
  cost about 0.1 % of a core, and 8 settled bells 0.5–0.7 % at 44.1 kHz and about 2.5 % at
  192 kHz.

  Run to run the same build varies by up to a factor of 1.7 on the small figures (a parked engine
  read 26 and 45 ns a frame on consecutive runs), so compare builds back to back.

  The analyser's audio-side handoff spends from the same budget (plan §3.5): the steady case
  leaves it 14–16 % at 192 kHz, the transition worst case 13–19 %.
- **Chosen, not measured**: `SMOOTHING_SECONDS` (5 ms, inside the collection's 5–20 ms window),
  `FADE_SECONDS` (20 ms, `mxm-fx-curve`'s), `CONTROL_SECONDS` (a third of a millisecond),
  `QUIET_FLOOR` (−200 dB of state), the 20 kHz fit point, the invertibility floors (6 dB, Q 40)
  and `OUTPUT_BOUND`.

A design change reruns `eq_n_cramping` and the design tests; a per-sample change reruns
`eq_n_cost` on an idle machine; replace the figures above rather than adding to them.

## What the tests cover

The tests cover: every design in range finite and stable from 1 to 768 kHz; the identity at 0 dB;
boost/cut cancellation; the bell's centre, edges and peak; shelf midpoints; cut corners, Q and
stopbands; the tilt, notch and band pass; the 20 Hz, Q 10, 192 kHz case; the cramping limits
above against both methods; solo per shape; every public entry point at any sample rate; bit-exact dry on noise, silence and NaN; fades to
nothing; the curve heard to 0.02 dB; a saved EQ from the first sample; click-free structural
changes, re-entry and solo; no jump at a fade's end; an output-gain ramp starting at its sample
on an empty EQ; a solo fading in from the interval it is built in; `is_settled` at every control boundary of a glide; no audition redesign for a Q
it does not hear; bit-identical output under any block
partition; no design at all for ignored automation or a static solo once settled; a filter's
recovery from a bad sample; DC through jumping bells, notches and cuts; bounded square-wave
modulation; tails, reset, hostile settings at every rate, the gain scale, mono against stereo,
and no allocation. The plugin's wrapper, its host status and
real DAWs belong to E2 and E5.
