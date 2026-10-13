# NOTES.md — plugins/mxm-fx-eq-n

The detail behind this folder's AGENTS.md: the reasons for each deviation, the solo and handoff
design, the editor's interaction in full and the analyser's measurements. AGENTS.md is the
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
  turn, ended by a pause or a press. A right-click opens the shapes and Delete. Identity never rests on hue: every point carries
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
- **Solo is held, never latched**: from the press to the release of the Band card's Hold button, a
  Ctrl/Cmd press on a point, or OPEN (Enter) held with the cursor on Hold. A bypassed band, the tilt and an Off slot are
  refused, as the DSP refuses them; the editor ends solo when it closes.
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
  dropped until it begins anew (`History::abandon_open`). A band turned Off outside the editor does
  the same for the gesture on it. A restore also forgets the values sent that the host had not
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
  this on a host that applies writes only when told. **A known limit**: the column's controls are
  mxm-kit's shared binding, which reads the parameter itself in every editor of the collection, so a
  slider pressed inside the same host buffer as a graph edit starts from the value before it. No
  person crosses from the graph to a slider that fast; a pending-aware read in the binding would
  close it for every editor (plan §7, kit follow-up).

  **The narrow layout scrolls the cursor's target into view** the frame it moves there
  (`scroll_to_rect` on its last drawn rectangle), asked inside each scroll area, since one takes
  only the requests made inside it: down the column, then sideways for the surface. With no band
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
  Measured on the Threadripper 3970X in release (`analyser_cost`, 450 points): 0.05–0.11 ms at
  44.1 kHz, 0.07–0.16 ms at 48, 0.08–0.19 ms at 96 and 0.13–0.35 ms at 192, the long window adding
  about 0.1 ms. Both FFT crates are optimised in dev builds, so a debug bundle animates too.

**Choices made for the owner's by-eye check** (plan E0.5, moved to E3): points at their own gain;
solo held, not latched; the bands' keys are the language's, MOVE on an edge keeping the other one.
The probe's band curves (owner: "mostly visual noise") are not drawn.
