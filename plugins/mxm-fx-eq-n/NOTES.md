# NOTES.md — plugins/mxm-fx-eq-n

The detail behind this folder's AGENTS.md: the reasons for each deviation, the solo and handoff
design, the editor's interaction in full, the analyser's measurements and the factory set. AGENTS.md is the
contract; this file is the reference it links to.

## Deviations from the shared parameter rules, and why

- **A band's gain is stored in dB**, against *gain parameters store linear gain*. That rule exists
  because nice-plug's logarithmic smoother is invalid across a dB range spanning zero. No nice-plug
  smoother runs here. An EQ band's gain is its curve's height, which the DSP glides and scales in dB.
- **Nothing is smoothed by nice-plug.** The DSP glides frequency, gain and Q in their own domains
  and fades every structural change. A host smoother on top would make the heard curve lag the drawn
  one.
- **Frequency and Q are `Skewed`, not logarithmic**: nice-plug has no logarithmic range. The skews
  put the middle of the travel near 1 kHz and Q 1, where a log axis would. The editor works in the
  natural units and steps in octaves, semitones and cents (brief A4).
- **Readings choose their unit and precision from the rounded value** (`plugins/AGENTS.md`,
  *Parameters*). Frequency switches from Hz to kHz, and Q from three to two to one decimal places.
  These readings are the EQ's own, so they stay here rather than in `mxm_params::text`.
  `every_reading_survives_the_hosts_text_round_trip` runs the shared walk
  (`mxm_plugin_test::text_checks`) with Q's 9.995 switch as this plugin's one extra boundary, and
  the preset tests assert `mxm_preset::stale_factory_texts` is empty.

## Band solo is a transient command, never a parameter

`plugins/AGENTS.md`'s transient-command exception, for one act. An audition is what somebody
listens to while holding a key or a button. Saved, a project would reopen soloed. Automated, a
render would play one band's band pass instead of the EQ. Recalled by a preset, it would change what
the EQ does without changing the EQ. So solo has no id, state or preset value.

`audition.rs` holds it: one atomic word with the band asked for and a count of the times the shell
ended a solo. The editor asks with the count it read, so a request made before an ending cannot
undo it, and a request to turn solo off fails only when the shell already turned it off.
- **The shell ends solo** in `reset` and `deactivate`. A state restore and an activation both run
  `reset` (the wrapper's state transaction reactivates and resets).
- **Each block it passes the request to the DSP's `set_solo`** only with an editor connected and
  never in an offline render.
- **Never waits, locks or allocates.**

The editor ends it on close (E3). `solo_needs_an_editor_never_plays_offline_and_ends_with_the_shell`
and `audition.rs`'s tests hold the lifecycle.

## The handoffs in full

- **Analyser** (`analyser.rs`, plan §4): preallocated blocks of pre- and post-EQ audio, every channel
  kept, through a bounded `rtrb` ring. Every block made spends a sequence number, so a block dropped
  on a full ring is a visible gap. Activation, deactivation and reset start a new generation, and a
  block carries its sample rate. The audio thread copies nothing with the editor closed or the
  analyser off. `process` works in pieces of one analyser block, so the pre-EQ copy needs no buffer.
  The engine's control clock runs on across calls, so the pieces change no sample
  (`a_hosts_block_sizes_change_nothing`).
- **The ring's consumer goes to the editor once** (`MxmFxEqN::editor`). Its half is
  `editor/spectrum.rs`, below.
- **Out meter** (`telemetry.rs`): mxm-kit's `PeakMeter` per channel, the collection's one policy: a
  peak max-combined and reset on read, a clip at 0 dBFS latched until acknowledged, a non-finite
  peak a clip and never a level. The shell publishes nothing with no editor connected. Opening
  the editor clears the meter as closing does: a block that read "connected" just before a close
  can publish after it (`a_block_publishing_across_a_close_is_not_shown_on_reopening`). The peak
  keeps a non-finite sample (`f32::max` would drop NaN), so broken output latches the clip.

## The editor in full

- **Layout**, the named exception to paged cards (`plugins/AGENTS.md`): the graph takes every spare
  point, an Out meter rail (`shell::vertical_level_meter`, post output gain) stands beside it, and a
  fixed-width column of three cards, Band, Output and Analyser, sits to its right and never under
  it. Narrower than graph and column, the whole surface scrolls sideways with the column still
  beside; taller than the space, the column scrolls on its own. The window opens at `REFERENCE`,
  inside the quarter-4K budget and as tall as the column's tallest state
  (`the_opening_size_is_inside_the_budget_and_holds_the_column_without_scrolling`), and narrows to
  `MINIMUM` (`the_app_bar_holds_in_the_minimum_window`). Every card is one of the cursor's
  (`navigation::bar_card`), the graph first.
  Each card body is a `mxm_ui::tree` and passes the tree checks in every Band state; the Band card
  is reserved to its tallest shape, so nothing below it moves. With no band selected its title
  alone says so ("Band —").
- **The graph**: clicking the composite line adds a 0 dB bell in the lowest Off slot, writing every
  field of the slot, so nothing a deleted band left behind returns. **A point is drawn at its own
  gain** (Pro-Q's convention, the composite drawn separately); a gainless shape sits on 0 dB.
  Dragging sets frequency and gain as one gesture, Shift at a tenth of the speed. The wheel turns Q
  only over the selected point with the graph focused, or over any point with Alt, one gesture per
  turn, ended by a pause or a press. A right-click, or MENU on the selected band, opens the shapes and Delete; the current shape's row
  takes the keyboard, so ↑ ↓ walk the rows and Enter or Space picks one. Identity never rests on hue: every point carries
  its number, the selected one a ring, a bypassed one is hollow; pre is an outline and post a fill.
  Every point has an accessible name and text value.
- **The keyboard is the collection's keyboard language** (mxm-kit's design system §11, engine
  `mxm-keys`, the keymap in use in the folder newDAWn and every editor share), through the cursor
  (`navigation::paged_with_bar`): the arrows walk the column's controls and the bars, VIEW + arrows
  go between the graph and the cards, and F1 shows the keys. **The graph is a surface**
  (`navigation::surface`): with the cursor on it, the language's outputs come to `on_surface`.
  ← → select the previous or next band by frequency, and step along the line a semitone where
  there is none; COARSE + ← → leaves the bands and walks the line an octave a press, so the
  keyboard's place can pass a cluster or sit between two close bands before ADD; MICRO + ↑ ↓ does
  nothing. MOVE + ← → moves the band's frequency (a semitone,
  COARSE an octave, MICRO a cent) and ↑ ↓ its gain (0.5, 3, 0.1 dB); VALUE + ↑ ↓ its gain;
  EXTENT + ← → narrows or widens it (a share of Q's logarithmic travel); MICRO + ← → takes its start
  or end edge, which **MOVE then moves alone, the other edge staying put**, so a bell, notch or band
  pass moves its centre too (a shelf's or a cut's edge changes Q alone); DUPLICATE + arrows carries
  a copy in the lowest Off slot where they go (with none free, nothing moves); ADD makes a 0 dB
  bell at the keyboard's place;
  REMOVE, DELETE and RIPPLE remove the band; BACK with nothing to cancel lets go of the edge, then
  of the band. A gesture is one undo step when OUT or the release keeps it, and BACK puts every
  parameter it opened back. A pointer press, or the cursor turning inert, ends a keyboard gesture,
  kept, through the language's own interrupt. The keyboard's place is painted only once the
  keyboard is used (`navigation::shown`).
- **Solo is a toggle** (the owner, 2026-10-14, testing in MXM Player: *"The hold button should be
  a toggle. I cannot hold it down and change the eq at the same time"*). The Band card's Solo
  button, or OPEN on it, latches solo on the selected band. A latch belongs to that band while it
  is selected and in the sound: selecting another band, bypassing or removing it, a preset load or
  a host restore, the shell's ending and the editor closing each let it go, and nothing brings it
  back but a press. A Ctrl/Cmd press on a point still solos it for as long as it is held. A bypassed
  band, the tilt and an Off slot are refused, as the DSP refuses them.
- **The gain scale fades while it has nothing to scale** (the owner, 2026-10-14, on Hum notch:
  *"gain scale does nothing"*). It scales bell, shelf and tilt gains only (brief A5), so with only
  notches, cuts and band passes in the sound, or none, it changes nothing; the slider is drawn at
  the disabled opacity and its hover says why. It stays a control: every parameter is operable from
  an empty Init (plan §8), and *No help text* keeps the reason off the panel.
- **Undo is plain** (plan §5.4; `editor/history.rs`). The editor's host context is a recorder, so
  every write is seen, from the column's controls, the graph and the keyboard alike, and the shared
  binding needed no observer. BACK during a drag puts the band back (`mxm_ui::drag::cancelled`),
  as the shared controls do. An action is the parameters one gesture changed; undo writes their
  "before" values as one gesture, redo their "after" values: Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z or
  Ctrl+Y, and the app bar's buttons. **Epochs**: whatever the preset bar writes (a load, Init) is a
  whole-patch replacement, and a host restore is counted by the view field's `set`; either clears
  the history, and **ends a gesture still open where the host has it, writing nothing**
  (`abandon_gesture`), so BACK can put back nothing from before. That is every gesture the
  recorder saw begin, a shared column control's as well as the graph's and the keyboard's: each is
  ended at the host once, egui's drag is stopped, and what the still-held control sends for it is
  dropped until it begins anew (`History::abandon_open`). A preset load abandons them at its first
  write, and the recorder ends them at the host before the load's own `begin`, so no begin is ever
  nested in another on the same parameter; the load's writes always pass. A band turned Off
  outside the editor does the same for the gesture on it. A restore also forgets the values sent that the host had not
  applied yet; a load's own writes replace theirs.

  **What was sent is read back** (`normalised_now`, `band_now`). nice-plug's CLAP context changes a
  parameter only when its queued event is written, at the end of a process cycle or a flush
  (`nice-plug/src/wrapper/clap/context.rs`), so a parameter the editor just wrote reads as before
  until then. Every editor read goes through the history's record of values sent and not yet
  applied: an ADDed band stays selected and takes its slot, MOVE moves it from where it was made,
  DUPLICATE's first arrow carries the copy. Each frame first retires a value whose readback has
  moved (`History::settle`), so automation that puts a parameter back where it was is not taken for
  the edit still on its way, and none stays pending past `PENDING_FRAMES` (half a second at 60 Hz,
  for a write applied and put back between two frames, which no readback can tell from one on its
  way). A gesture's starting value comes the same way, so BACK puts back only its own. The tests run
  this on a host that applies writes only when told. These reads look each parameter up alone
  (`EqParams::param`), and the recorder keeps its pointer list from attach, so a frame's reads and
  a drag's writes allocate nothing (`the_editor_s_hot_reads_and_recorded_writes_allocate_nothing`). **A known limit**: the column's controls are
  mxm-kit's shared binding, which reads the parameter itself in every editor of the collection, so a
  slider pressed inside the same host buffer as a graph edit starts from the value before it. No
  person crosses from the graph to a slider that fast; a pending-aware read in the binding would
  close it for every editor (plan §7, kit follow-up).

  **The narrow layout scrolls the cursor's target into view** the frame it moves there
  (`scroll_to_rect` on its last drawn rectangle), asked inside each scroll area, since one takes
  only the requests made inside it: down the column, then sideways for the surface. A change of the
  window's size or zoom asks again, on that frame and the next, so a target the cursor did not
  leave stays in view. With no band
  selected the Band card holds no control and is no stop of the cursor's (`bar_card_absent`):
  VIEW from the graph would land on nothing and fall back, and at the minimum window it is the
  card nearest the graph's middle.
  **Closing the editor** ends, besides its own gestures, whatever a shared control still held
  open, through the recorder, kept as one action. The history is the instance's and survives the editor closing; it keeps 256 actions.
- **The analyser** (`editor/spectrum.rs`, on mxm-kit's `mxm-fft`, plan §4): band power over a twelfth
  of an octave per drawn point, averaged over the channels, so a full-scale sine reads 0 dB and
  noise reads alike whatever the window. A short window of about 85 ms at the full rate, and below
  300 Hz a long window on a decimated stream (an eighth-order low pass to about 3 kHz), 0.34 s at
  Medium and 1.4 s at High. A gap or a new generation restarts both windows; a block at another
  rate is ignored. Rises show at once, falls at the chosen speed; the tilt pivots at 1 kHz.
  **Settings are view state** (`view.rs`): on/off, Show (pre, post, both), Resolution, Speed and
  Tilt, defaulting to 1.5 dB/octave, the research's corpus slope on a band-power display. They
  persist with the project, mark it dirty through a parameter-free GUI transaction, and are no
  preset's or undo's. With the analyser off nothing is copied or transformed, and coming back on
  starts from silence. A stream Show reveals is analysed at once, without waiting for new audio. A
  settled frame allocates nothing (`a_settled_frame_allocates_nothing`).
- **The analyser has a budget on the GUI thread**, `spectrum::BUDGET_MS` of a 60 Hz frame.
  Measured on the Threadripper 3970X in release (`analyser_cost`, 450 points, 2026-10-15):
  0.04–0.12 ms at 44.1 kHz, 0.06–0.19 ms at 48, 0.07–0.22 ms at 96 and 0.12–0.40 ms at 192; the
  side-chain line adds about 0.1 ms (worst 0.49 ms, 192 kHz, Both). Both FFT crates are optimised
  in dev builds, so a debug bundle animates too.
- **Freeze and grab** (plan §7.4, `editor/grab.rs`): while running, the editor keeps the shown
  source's highest level per point over two seconds (`Hold`; post when shown, else pre). Freeze
  holds that line, the other line's last frame, the side-chain line, and the peaks `grab::peaks`
  finds in it. Both the hold and a freeze carry a `grab::Basis` (the view settings and the audio's
  generation): a change of any view setting, a reset (seen with the first block after it) or a
  new rate restarts the hold and ends a freeze, as do the analyser off, a width change (it moves the frequency grid), a restore
  or preset load, and closing the editor (`Frozen::holds`, `unfreeze`); it is never saved. A peak
  near the display's edge counts once it has fallen 6 dB before the edge. On a basis change the
  display starts from its new readings (`Spectrum::snap`), not a fall from the old levels, so the
  hold takes nothing of before. A peak's drawn label is part of it for the pointer; its marker
  stays inside the graph (`graph::peak_marker`), so a peak above 0 dB under Tilt is still grabbed. A drag from a peak makes a static bell at its frequency with its Q, only the gain
  following the drag, one undo step; frozen with no band selected, ← → walk the peaks and ADD
  makes the focused one's bell. With every slot in use nothing is made and the hover says why.
- **Find, the resonance finder** (plan §7.5, `editor/finder.rs`), in the order the work goes:
  - *Listening.* The analyser's pre-EQ blocks, while Find is on, into three rungs: the input
    decimated to at most 64 kHz, then a quarter and a sixteenth of that, each a frequency's only
    home (from 1.6 kHz, from 400 Hz, below), the display's anti-alias low pass on left and right;
    above 64 kHz a fourth at the full rate owns the top octaves. A rung reads to Nyquist, or to
    its anti-alias corner when decimated, and owns nothing within a third of an octave of that, the
    faster rung taking it (`ownership`): a peak there needs its key point's reach above it. The
    edge test makes its resonances with the EQ's own matched bell, since the cookbook biquad's
    bandwidth shrinks by sin w0 near Nyquist (its "Q 20" at 23 kHz and 48 kHz measured Q 290,
    narrower than any movement frame resolves).
    8,192-point frames for width, 2,048 for movement (1,024 in the lowest rung's top octave, the
    shortest that tells a semitone apart there), Hann, a hop of half a frame; a frame under
    −60 dBFS counts for nothing. Five spectra, mid and side formed from the channels' spectra.
  - *The long-term spectrum* is the heard width frames' level averaged **in dB**, each bin clipped
    at its running mean + `CLIP_DB` (3 dB). Under mean power each note's harmonic lines tower over
    the resonance's bump: a candidate near 1 kHz measures Q 47 against a built 10. A clip of 1.5 dB
    biases every measurement; 6 dB leaves lines 2–3 dB high. No morphological opening for the
    lines: on noise it widens bumps (a Q-10 bell fits at 8.6 with one, 10.0 without). The
    consequence, stated in Find's hover: a resonance is measured where broadband sound reaches it.
  - *The clip starts at the third width frame*: one frame is too noisy a mean to clip against (a
    bin that read low would hold every later frame down; clipping from the second broke the 8 kHz
    and placement scenes). A candidate therefore needs three width frames, so the clip is always in
    force, and sixteen eligible movement frames: with ten, a cell of noise passes "half the frames"
    by chance somewhere among hundreds (a 73 Hz candidate from noise alone at 4.5 s).
  - *Candidates* are its peaks by F3's rule, then **fitted**: Levenberg–Marquardt of the cookbook
    bell on a sloping floor over a third of an octave either side, Huber-weighted (1.5 dB) and run
    three times, 64 points. One-point measures — a parabola's top, the half-height crossings —
    follow whichever line rides highest on the bump; the fit reads it whole and is the shape the
    cut inverts. Exact on a noiseless bell (`the_fit_finds_a_cookbook_bell_exactly`); on noise
    alone Q within about 10 %. Only the final bands are fitted, and left and right when both have
    the peak (the pair rule needs their true heights). A fit narrower than four width bins is
    dropped: §8.6's 4/B, under which the width is the window's (noise at 73 Hz fitted Q 200).
  - *Persistence* per 1/48-octave cell. A cell stands out in a movement frame when its highest
    level within half a semitone stands 6 dB (F3's prominence) over the **median** of the cells a
    twelfth to a third of an octave away: against the lowest of them, as F3 measures, one frame's
    noise put a "peak" within half a semitone of anything. Near the edge of what a rung reads, the
    median of what is read there, one whole side at least: asking for both sides left the top
    third of an octave (18.7–23.5 kHz at 48 kHz) and the third below a rung's anti-alias corner
    unable ever to stand out. The excitation is the loudest of the
    rung's other frequencies, outside the cell's third, by semitone, and "two excitations" means
    the commonest accounts for at most two thirds of the frames it stood out in: the plan's
    strongest other peak *in that third* sees only "none" where broadband sound excites a
    resonance (an 8 kHz resonance lost its verdict), and one odd frame flipped a yes/no flag.
    Only persisting peaks enter the placement rule, so a note's line on the other channel does
    not make a one-channel resonance a pair.
  - *What the EQ takes* is the difference of two fits from the same guess, without and with the EQ
    as heard on that spectrum: two fits from different starts settle up to 0.5 dB apart with no EQ
    at all.
  - *Cost.* Listening and a copy of the evidence on the GUI thread; the search (candidates and
    proposals) on a worker thread (`finder::Worker`), one at a time, so its fits never cost a
    frame. At 192 kHz in release (`finder_cost`, 2026-10-15), the full-rate rung included:
    0.54 ms a frame on average and 1.36 ms at worst on the GUI thread, the copy 0.41 ms; a search
    4.5 ms, too long for a frame. With the analyser's 0.27 ms, inside its 2 ms.
  - *Test scenes.* A melody's notes need played edges (20 ms raised cosine) and untempered pitches:
    hard-edged notes smear lines tens of hertz wide, and a tempered scale shares harmonics between
    half its notes, which no persistence test can tell from a resonance (research §8.2). Under the
    melody a broadband bed at −10 dB; at −30 dB the bump is mostly seen through lines.
- **Match, the EQ match to a reference** (plan §7.6, `editor/matcher.rs`):
  - *Listening.* The side chain (the reference) and the pre-EQ input (the source) on F4's rungs
    (`finder::rung_specs`), width frames only. **Every frame enters the long-term power means**; only
    a heard one (−60 dBFS) counts towards the seconds heard. Gating the frames as F4 does weighed a
    quiet passage differently on rungs of different frame lengths: a 3 s gap in the reference put a
    step at 400 Hz and moved the low shelf by 1.2 dB. Counted, a quiet stretch is a constant the
    level takes out; what frames straddling it mix in still moves a 16 s match by under 0.5 dB.
  - *The EQ as heard* is exact for its static curve: the source's covariance per bin (`|L|²`,
    `|R|²`, `L·conj(R)`) through the 2×2 transfer of the channel stage then the mid/side stage
    (`through_power`). With the known EQ counted, the measured reference and the computed
    through-the-EQ level agree within 0.003 dB on every cell.
  - *The ease* weighs the fit's error, not the target: easing the target to 0 dB had the fit
    answer a +4 dB low shelf with a −4.5 dB one at 30 Hz, undoing its own tail.
  - *The fit* names F4's `fit_bell` scheme; its regions are read on the weighted residual, and the
    half-height walk reads the region's sign, so it passes the region's own end.
  - *Cost.* At 192 kHz in release (`match_cost`, 2026-10-15): 0.34 ms a frame on average and
    0.85 ms at worst on the GUI thread, the copy included; a fit of the plan's four-band difference
    4.8 ms on the worker. With the analyser's 0.27 ms, inside its 2 ms; Find and Match never run
    together.
  - *Test fixtures.* The resonances and the known EQ are the EQ's own matched bands
    (`Filter::fresh`), so their shapes hold to Nyquist; the source the same noise as the reference,
    so `D` is the EQ's response alone.

- **Phase, the latency and its restart** (plan §7.7):
  - *Reported at activation.* nice-plug defers `ActivateContext::set_latency_samples` until
    `activate` returns, and a latency different from the last one asks the host for a restart once
    the plugin is active; reporting the same again asks nothing, so the restart does not loop.
    Through MXM Player (`a_restored_linear_phase_eq_delays_the_render_by_its_latency`) a restored
    Linear phase's render is the dry one later by exactly the latency, 2,304 frames at Low.
  - *Asked while active.* `process` reports the asked phase's latency once (`phase_change`); the
    engine keeps the phase activation built until the host restarts the plugin, and the editor
    draws the phase in force, not the one asked for. A resolution changed in Zero latency asks
    nothing.
  - *The Output card.* Phase and Resolution side by side were 333 px in a 308 px column, so they
    stack; the window opens at `REFERENCE`, the second column grown by the two rows.

**Choices made for the owner's by-eye check** (plan E0.5, moved to E3): points at their own gain;
solo a toggle (the owner, 2026-10-14, reversing held); the bands' keys are the language's, MOVE on an edge keeping the other one.
The probe's band curves (owner: "mostly visual noise") are not drawn.

## The factory set

The brief asks for about a dozen presets named for the job and gives eight examples (brief B,
*Factory presets*, signed 2026-10-12); the fifty-preset floor is for instruments. Thirteen ship
(E4, 2026-10-14): the eight examples, then *Bass weight*, *Band pass sweep*, *Snare crack* and
two hum notches, chosen so every shape is used at least once — the low shelf and the band pass
had no example. Each is a starting point, a few bands of the usual move, not a finished mix.
F1 adds the two dynamic ones the owner signed (brief §C, 2026-10-14): *De-esser*, a bell at 6.5 kHz
cutting up to 6 dB over −30 dB with a 1 ms attack, and *Resonance tamer*, a narrow bell at 2.5 kHz
(Q 4) cutting up to 6 dB over −35 dB; both at a 0 dB drawn gain, so they touch only what gets loud.

- **Why the lowest slots.** Automation belongs to the slot number (decision 10) and a new band
  takes the lowest Off slot, so a preset in slots 1…n leaves the player's next band in n + 1, and
  a preset's band 3 is always slot 3 in a saved project.
- **Why Init's output and gain scale.** A preset that also moved the output would be louder or
  quieter for a reason the graph does not show; the gain scale at 100 % means the drawn gains are
  the heard ones.
- **Why a design table in the editor's units.** A file stores normalised values, and a design
  written in them says nothing until it is generated and its `text` read back (`mxm-mono-08`'s
  design table, whose own comment says so). This table says *Bell 300 Hz, −3 dB, Q 1.2*, and the
  generator turns it into the file. A gainless shape
  stores 0 dB and a shape without a slope the default slope, as the editor would.
- **The values** are conventional moves, not measured ones: a 60 Hz low cut at 12 dB/octave; voice
  presence at 3.5 kHz; mud at 300 Hz; air as a 12 kHz shelf; a kick's thump at 60 Hz, box at
  350 Hz and beater at 4 kHz; the telephone band's 300 Hz – 3.4 kHz; mains hum at 50 and 60 Hz
  with three harmonics each, notched at Q 10. E5's listening judges them.

## The slower checks

- The analyser's cost: `cargo test -p mxm-fx-eq-n --release --lib analyser_cost -- --ignored
  --nocapture`; the finder's, `finder_cost` the same way.
- The pictures: `cargo test -p mxm-fx-eq-n --lib pictures`, into `target/layout-tree/mxm-fx-eq-n/`.
- The factory set: `cargo test -p mxm-fx-eq-n --lib write_the_factory_presets -- --ignored`.
- The validator's random parameter fuzz once found a subnormal output, now flushed in the DSP.
- The editor reads what it sent because nice-plug changes a parameter only when its queued event is
  written: the graph, the keyboard's edits and the column's own decisions (its menus, Solo) would
  otherwise read the value from before their own write.
