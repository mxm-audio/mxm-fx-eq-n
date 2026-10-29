# NOTES.md — plugins/mxm-fx-eq-n

The detail behind this folder's AGENTS.md: what is built, the parameters in full, the reasons for
each deviation, the solo and handoff design, the editor's features and interaction in full, the
analyser's measurements, the factory set and the host tests. AGENTS.md is the contract; this file
is the reference it links to.

## What is built

**Built through E4**: the host surface, the preset seam, solo's command path, the analyser and the
meter (E2), the editor (E3), and the factory set and the host tests (E4), and F1's dynamic bands
through the shell, the editor and two presets (F1.1–F1.4), their side chain (plan §7.2), F2's
per-band stereo placement (plan §7.3), F3's freeze and grab (plan §7.4), F4's resonance finder (plan
§7.5), F5's EQ match (plan §7.6), F6's phase modes (plan §7.7), F7's auto gain and other tracks'
spectra (plan §7.8) and F8's work with several bands (plan §7.9). DAW and listening checks are E5's,
F1.5's, SC.4's, F2.4's, F3.4's, F4.3's, F5.3's, F6.4's, F7.5's and F8.4's.

The editor's modules, by phase: `mod.rs` its state, writes and keyboard commands; `panel.rs` the
frame; `graph.rs` the graph and its pointer; `column.rs` the cards; `history.rs` undo; `spectrum.rs`
the analyser's editor half; `grab.rs` F3's hold, peaks and freeze; `finder.rs` F4's resonance
finder, the rungs and the worker, its tests in `finder/tests.rs`; `matcher.rs` F5's EQ match, its
tests in `matcher/tests.rs`; `references.rs` its saved references; `crate::learned` what each EQ
learned and what it is matched to, the project's; `external.rs` F7's external spectrum and collisions; `selection.rs`
F8's selection, a group's edits and the clipboard; `tests.rs` the editor's tests.

## The parameters in full

- 24 slots, nested, each with `type`, `bypass`, `freq`, `gain`, `q` and `slope`; their ids are
  `<field>_<slot>`, `type_1` … `slope_24`, and their host names carry the slot number ("Band 7
  frequency"), never the shape;
- `output`, the output gain, as linear gain shown in dB;
- `gainscale`, 0–200 %;
- F1's dynamics, four per slot after the globals (plan §7.1): `range_<n>`, `threshold_<n>`,
  `attack_<n>` and `release_<n>`, named "Band 7 range" and grouped with their slot; their place
  in the editor's indices starts at `params::DYNAMICS_BASE`, so no v1 index moved;
- the side-chain switches after them (plan §7.2), `key_1` … `key_24`, "Band 7 side chain", from
  `params::KEY_BASE`;
- the placements (plan §7.3), `place_1` … `place_24`, "Band 7 placement", stepped 0–7
  (0 Stereo, 1 Left, 2 Right, 3 Mid, 4 Side, 5–7 unassigned, read *Unavailable n*, heard as
  Stereo), from `params::PLACE_BASE`;
- F6's phase after them (plan §7.7), **not automatable**: `phase` stepped 0–7 (0 Zero latency,
  1 Linear phase, the rest heard as Zero latency) and `resolution` 0–7 (0 Low, 1 Medium,
  2 High, the rest heard as Medium), at `params::PHASE_INDEX` and `RESOLUTION_INDEX`;
- F7's `autogain` last (plan §7.8), on or off, automatable, off by default, at
  `params::AUTO_GAIN_INDEX`.

`the_ids_are_complete_unique_and_293` holds them.

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

## Match's Preview is a transient command too

Plan §7.6, decision 50; the kit's exception for a transient, non-value action
(`plugins/AGENTS.md`, *Editor*), as band solo is.

- **Why not a parameter.** Preview is an audition of bands not yet applied. Saved, a project would
  reopen playing bands it does not hold; automated or rendered, a render would play them; so
  nothing of it is a parameter, and the host's undo and automation never see it.
- **The handoff.** `preview.rs` is a sequence lock of atomics with one writer, the editor: the
  bands by the slot Apply would write, between two counts of a sequence, odd while it writes. The
  audio thread reads the count, the bands and the count again, and keeps what it last read whole
  when they differ. Latest wins: it is a state, not a queue of acts, so nothing is dropped that a
  later write does not replace.
- **The process boundary.** `MxmFxEqN::sync`, each block: only with an editor connected and never
  offline; a band only in a slot that is Off, played static, unkeyed and Stereo, as Apply writes
  it; each slot set once a block, since the engine starts a band's transition on any change. A
  reset or a restore leaves it to the editor, which hands it over anew whenever the proposals
  change. **The editor ends it** on close; after Apply it holds what it played for
  `PREVIEW_HOLD_FRAMES`, while the host takes the bands written, so the sound does not dip.
- **Tests**: `preview_plays_in_an_off_slot_with_an_editor_and_never_offline`,
  `with_an_editor_open_processing_never_allocates` (a preview published mid-run),
  `preview_hands_the_proposals_to_the_free_slots` and `preview.rs`'s own.

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
- **Other tracks** (`instances.rs`, plan §7.8; NOTES, *Other tracks*): a second tap, post-EQ only,
  copies into its own ring only while another track's editor watches (an atomic count, the audio
  thread's one read); the ring's consumer lives in the instance's registry entry, which editors
  drain under a mutex the audio thread never takes. The instance id is host state
  (`#[persist = "instance"]`), never a preset's; a duplicated id is renewed for the later instance.

## One latency per activation

- **One latency per activation** (plan §7.7; the kit's *Latency is fixed*; NOTES, *Phase, the
  latency and its restart*): activation builds and primes the phase the parameters hold and
  reports its latency; one asked while active is reported from `process` and heard after the
  host's restart. A reset in Linear phase hears its kernel once designed, in realtime never
  waiting (the DSP's `AGENTS.md`).

## The editor's features in short

The full text of each follows, in *The editor in full*.

- **Dynamic bands** (plan §7.1, §7.2): Range, Threshold, Attack, Release, the Side chain toggle and
  a meter of the movement in a bell or shelf's card; the graph rings the point (dashed when keyed),
  draws where Range would take it and, while a band moves, the curve as heard over the drawn one,
  both from `Telemetry::gains`, published only with an editor connected.
- **Stereo placement** (plan §7.3): a **Placement** selector beside Bypass, a **Split** button (a
  Stereo band into Left/Right, Alt into Mid/Side) beside Delete band. With placed bands the graph
  draws each channel's response to itself (L, R; M, S dashed), lettered, as is a placed point. In
  mono (`Telemetry::mono`) a placed band is faint, refuses solo, and offers only Stereo.
- **The analyser** (`spectrum.rs`, mxm-kit's `mxm-fft`): band power per twelfth of an octave, a
  decimated long window for the lows; with no audio for `spectrum::STALE_SECONDS` the display
  falls to the floor and the next audio restarts both windows; settings are view state (`view.rs`),
  persisted, dirty-only; within `spectrum::BUDGET_MS` of a frame (`analyser_cost`). The
  **External** selector (plan §7.4, §7.8) draws one other spectrum, dashed and lettered: the side
  chain's on a layout with the port ("SC"; the audio thread copies the key only then) or another
  track's by name (`external.rs`, a second analyser, post only); **Collisions** hatch the bottom
  edge where both carry sound (NOTES, *Other tracks*).
  **Freeze** holds two seconds' highest levels and marks peaks a drag grabs (NOTES, *Freeze*).
- **Find** (plan §7.5, decisions 27–30; NOTES, *Find*): the Resonances card's toggle; it proposes
  static cut bells from the pre-EQ input, never a continuous curve, written as ordinary bands, one
  undo step each; never saved; its search on `finder::Worker`, listening inside
  `spectrum::BUDGET_MS` (`finder_cost`).
- **Phase** (plan §7.7, decisions 35–37; NOTES, *Phase*): the Output card's **Phase** and
  **Resolution** selectors, each change one undo step, Resolution offered only in Linear phase;
  their hovers give the latency and say when a change waits for the host's restart. **The drawn
  curve follows the phase in force** (`Telemetry::phase`): in Linear phase the kernel's own
  magnitude (`linear::heard_lines`), and a dynamic band shows no reach there, being held.
- **Several bands** (plan §7.9, decisions 44–47; NOTES, *Several bands*): a selection, its
  primary the band `selected` names; **every edit of the primary moves the group** (`group_edit`)
  by one ratio, offset or factor, stopping at its first member's limit, one undo step; Delete,
  DUPLICATE, copy, cut, paste (a clipboard in the process) and Invert gain take the selection.
- **Auto gain** (plan §7.8, decisions 38–40; NOTES, *Auto gain*): the Output card's switch, on
  Phase's row; the DSP estimates the make-up (`loudness.rs`), its hover gives it
  (`Telemetry::make_up`), the output at it or ramping to it within one fade.
- **Match** (plan §7.6, decisions 31–34 and 48; NOTES, *Match*): each EQ's Learn, kept with the
  project, and Match to another EQ or a saved reference, offered with the analyser on, on every
  layout; it proposes bells and shelves for the free slots, the EQ as heard counted, **Apply**
  writing them all as one undo step; its fits on the same worker, learning inside
  `spectrum::BUDGET_MS` (`match_cost`).

## The editor in full

- **Layout**, the named exception to paged cards (`plugins/AGENTS.md`): the graph takes every spare
  point, an Out meter rail (`shell::vertical_level_meter`, post output gain) stands beside it, and two
  fixed-width columns of cards sit to its right and never under it (decision 30): the Band card
  alone, then Output, Analyser, Resonances and Match. Narrower than graph and columns, the whole
  surface scrolls sideways with the columns still beside; taller than the space, the columns
  scroll together. The window opens at `REFERENCE`, inside the quarter-4K budget and hugged to
  the taller column
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
  Medium and 1.4 s at High. Across the octave about 300 Hz (212–424 Hz) the two are blended in dB:
  a transient reads louder in the short window than averaged over the long one, and a switch at
  one frequency drew that difference as a cliff of up to 14 dB, in Bitwig on a retriggered sine
  (2026-10-16); steady sound reads alike in both. **What is blended is each window's own line**,
  its readings under the display's rise and fall (`Spectrum::window_db`), never the raw readings:
  between two transients the short window reads next to nothing, and a raw blend sank the points
  just inside the blend under both windows, a V of up to 8 dB in the same check. Each line is the
  one its side draws: the long side reads the short window until the long one has analysed, after
  a gap or a silence too, as the points below the blend do, so the blend always lies between its
  neighbours. A restart clears those lines with the drawn ones; a change of view settings snaps
  them. A gap or a new generation restarts both windows; a block at another
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
- **Find in short** (plan §7.5, decisions 27–30): the Resonances card's toggle, offered only
  with the analyser on; Find and Freeze each end the other, the analyser off or the editor closed
  ends it, and it is never saved. On, it learns from the pre-EQ input that plays (`finder.rs`);
  off, it holds what it learned, as a noise reduction holds its profile, until on again learns
  anew (the owner, 2026-10-27: "Just the audio you play to it"). It proposes static
  cut bells, never a continuous curve: dashed curve and labelled downward triangles on the graph;
  a click, a drag (gain following, a pair together), ADD on the keyboard's proposal or **Apply
  all** writes them as ordinary bands, every slot field (`slot_writes`), one undo step each.
  **Amount** (whole percent, 20 % by default) is view state, saved, a change marking the project
  dirty and ending no freeze (`ViewSettings::display`). The search runs on `finder::Worker`, so
  the GUI thread pays only for listening and a copy of the evidence, inside
  `spectrum::BUDGET_MS` (`finder_cost`).
- **Find, the resonance finder** (plan §7.5, `editor/finder.rs`), in the order the work goes:
  - *Listening.* The analyser's pre-EQ blocks, while Find learns, into three rungs: the input
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
  - *Held* (`hold`, `MISSING_SECONDS`): a candidate a search no longer finds stays, as last
    measured, until that much heard input passes without it; one found within a sixth of an
    octave is it, measured again. Every rule has an edge: over the owner's voice-over
    (2026-10-28) the 1.5 kHz, 97 Hz and 370 Hz candidates stood out in 0.41–0.55 of their frames,
    the 7 kHz one at a 6.0–6.2 dB prominence, and the 370 Hz one moved between Stereo and Left;
    each came and went with the half-second updates. Held, the minute's proposals moved (the
    7 kHz one between 6.9 and 7.5 kHz) but went only twice.
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
- **Match in short** (plan §7.6, decisions 31–34 and 48): the Match card, under the Band card
  (the owner, 2026-10-28), offered with the analyser on, on every layout. **Learn** learns this
  track anew while on, its input before the EQ and its output after it, and keeps them with the
  project while off (`crate::learned`, `EqParams::learned`); **Match to** chooses another EQ that
  has learned, read through the registry entry that shares its field (`Instance::learned`), or a
  saved reference. The owner asked for it (2026-10-28: *"This shit is too convoluted. How does
  other plugins do it?"*), as Pro-Q's instances and Ozone's snapshots work. **Why each EQ learns
  only its own track**: a voice-over's clips never play together, so nothing that needs two tracks
  playing at once, a side chain, carries a reference; and one window learning another track
  depends on that track's EQ running while it is watched, which Resolve did not do for every EQ
  on the voice-over track, whatever played, until a new one was inserted there (2026-10-28).
  **The lamp** beside Learn is green while its seconds heard rose within `LAMP_SECONDS`
  (`update_lamp`), so a track sending nothing shows. **What was learned is drawn**
  (`learned_lines`, the owner: *"I still cannot see the eq curve of what has been learned"*): the
  reference and this track as lettered lines on the analyser's scale
  (`graph::learned_display_db`), each read while it sounds, solid: drawn faint where unsure, a
  voice's lines only flickered while their shape stood still (the owner, 2026-10-28). **No fixed
  time** (*"Yes drop the 10 secs"*): each difference's shape is shrunk by how unsure it is
  (`matcher::shrunk`). **Preview** plays the proposals in
  the free slots before Apply (`preview.rs`), a transient command as solo is; after an Apply it
  holds what it played for `PREVIEW_HOLD_FRAMES`, while the host takes the bands written.
  Learn and a choice end Find and Freeze;
  Find and Freeze end Learn, keeping what it learned, and hide the proposals. It proposes bells and
  shelves for the free slots, the EQ as heard counted; dashed curve and labelled hollow diamonds on
  the graph; **Apply** writes them all, every field, one undo step, only on the basis they were
  fitted on, what was learned and what it is matched to part of it. **Amount** (40 %) and
  **Detail** (8 bands) are view state, saved. Its fits run on the same worker, one job at a time
  (`editor::Task`); learning stays inside `spectrum::BUDGET_MS` (`match_cost`).
- **Saved references** (plan §7.6, decision 48; `references.rs`): **Save** names this track's
  output as learned, in a text field where Match to was, a name taken replaced only on a second
  press; a JSON file per reference in `references/` beside the user presets (`references::root`),
  listed every `REFERENCES_SECONDS` and chosen in **Match to** for any track; **Folder** shows the
  folder. What Match learned and what it is matched to are host state beside the view settings
  (`EqParams::learned`, `match_to`): `ViewSettings` is `Copy`, and they are not. Not a preset: a
  preset holds parameter values only. The editor's tests give it a sandbox; `None` otherwise, so
  no test writes in the player's folder.
- **Match, the EQ match to a reference** (plan §7.6, `editor/matcher.rs`):
  - *Learning.* The input before the EQ and the output after it, two streams of one block's
    audio, on F4's rungs (`finder::rung_specs`), width frames only. What is kept is cells, each
    one's power per hertz, the bins inside its 1/48 octave averaged, the input as its covariance; a sine read through them agrees
    across rates within 0.2 dB from 1 kHz up, and 0.5 dB at 100 Hz, where a cell is four or five
    bins. Two noise learns at different rates differ by their noise, about 1 dB under 400 Hz in
    11 s, so the test reads a sine. **Every frame enters the long-term power means**; only
    a heard one (−60 dBFS) counts towards the seconds heard. Gating the frames as F4 does weighed a
    quiet passage differently on rungs of different frame lengths: a 3 s gap in the reference put a
    step at 400 Hz and moved the low shelf by 1.2 dB. Counted, a quiet stretch is a constant the
    level takes out; what frames straddling it mix in still moves a 16 s match by under 0.5 dB.
  - *The EQ as heard*, for its static curve: the source's covariance per cell (`|L|²`, `|R|²`,
    `L·conj(R)`), kept with the project, through the 2×2 transfer of the channel stage then the
    mid/side stage at the cell's centre (`through_power`). Read per bin before it was kept per
    cell, the measured reference and the computed through-the-EQ level agreed within 0.003 dB on
    every cell; per cell they agree within the test's 0.5 dB with a Mid and a Left band in it.
  - *How sure.* A learn's frames alternate between two halves every `CHUNK_SECONDS` of sound;
    each half's smoothed level, their difference less its mean over the range, its RMS over the
    smoothing's window, halved and widened by `√(1 + 2/n)` for `n` seconds in the smaller half, is
    a cell's spread. Measured on the known difference's noise, each learn's spread in dB:

    | Heard | 35 Hz | 60 Hz | 120 Hz | 250 Hz | 500 Hz | 1 kHz | 4 kHz | 12 kHz |
    |---|---|---|---|---|---|---|---|---|
    | 3 s | — | — | — | — | 0.92 | 0.14 | 0.12 | 0.10 |
    | 6 s | 0.55 | 0.75 | 0.79 | 0.49 | 0.78 | 0.19 | 0.15 | 0.09 |
    | 10 s | 0.44 | 0.50 | 0.44 | 0.55 | 0.37 | 0.06 | 0.10 | 0.04 |
    | 20 s | 0.26 | 0.45 | 0.32 | 0.45 | 0.29 | 0.09 | 0.07 | 0.03 |
    | 30 s | 0.16 | 0.32 | 0.23 | 0.31 | 0.15 | 0.05 | 0.04 | 0.04 |

    Two separate 11 s learns' 1 dB under 400 Hz is what the spreads read there. **Why shrunk, not
    left out**: counting a cell only under 0.5 to 1 dB of the two spreads together, as it was
    built first, left a voice-over's almost every cell out (its halves hold different sounds) and
    proposed one +3 dB bell where the lines differed by 5 dB at 100 Hz, nothing at 40 % (the owner
    in Resolve, 2026-10-28). Shrunk, `s·s²/(s² + u²)`, a 4 dB difference unsure by 1 dB stays
    3.8 dB; the known difference's low shelf is matched in 12 s again. A silent stretch in the
    reference leaves its lows less sure, so shrunk a little more: 0.52 dB at 31 Hz after 16 s,
    diluted by 24.
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
  - *One latency per activation* (the kit's *Latency is fixed*): activation builds the engine in
    the phase the parameters hold (`Engine::with_phase`, Linear phase's kernel primed before the
    first sample) and reports its latency (`ActivateContext::set_latency_samples`); a phase asked
    for while active is reported from `process`, which nice-plug turns into the host's restart, the
    phase in force playing on until then (MXM Player's headless path never restarts, so there it
    waits for the next activation). `Telemetry::phase` publishes the phase in force; `deactivate`
    drops Linear phase's engine and its design thread.
  - *Reported at activation.* nice-plug defers `ActivateContext::set_latency_samples` until
    `activate` returns, and a latency different from the last one asks the host for a restart once
    the plugin is active; reporting the same again asks nothing, so the restart does not loop.
    Through MXM Player (`a_restored_linear_phase_eq_delays_the_render_by_its_latency`) a restored
    Linear phase's render is the dry one later by exactly the latency, 2,304 frames at Low.
  - *Asked while active.* `process` reports the asked phase's latency once (`phase_change`); the
    engine keeps the phase activation built until the host restarts the plugin. The editor draws
    the phase in force, not the one asked for, and the hovers give the delay in force and what a
    waiting change makes it. A resolution changed in Zero latency asks nothing.
  - *A reset.* A host resets on the audio thread, so in realtime the reset asks for the settings'
    kernel and never waits: it is taken the first partition it is ready, unfaded while only
    silence has been convolved, else faded in at the next boundary (code review, 2026-10-15).
    Offline it waits, so a bounce hears it from its first sample.
  - *The Output card.* Phase and Resolution side by side were 333 px in a 308 px column, so they
    stack; the window opens at `REFERENCE`, the second column grown by the two rows.

- **Auto gain** (plan §7.8, decisions 38–40): `autogain`, the 293rd parameter, off by default and
  automatable; the DSP estimates the make-up from the drawn curve (`loudness.rs`; the DSP's
  `NOTES.md` has the weighting and the cost) and applies it with the output gain, each held within
  ±24 dB; the shell publishes it (`Telemetry::make_up`) for the switch's hover. *Its row*: the plan
  gave it a row of its own under Output gain, but then the second column needed 1,005 px of 934,
  a window 1,081 px tall, one over the budget; on Phase's row, as Bypass shares Placement's, the
  window opens at 1,532 × 1,045.
- **Other tracks** (plan §7.8, decisions 41–43; `instances.rs`, `editor/external.rs`):
  - *The registry* is one per process; an instance joins when made and leaves when dropped. Its
    second tap (post only, `instances::SHARE_BLOCKS`) copies only while its count of watchers is
    above none, and a watch from none starts a new generation and drops the held blocks, so audio
    from before reads as nothing. Viewers drain the one consumer into the entry's last
    `instances::HELD_BLOCKS` and take the blocks newer than they have seen; one that fell behind
    gets a gap, which its analyser restarts on.
  - *Identity*: a random 64-bit id in the host's state (`IdentityField`), restored with it; of two
    holding one id the earlier keeps it, the check and the store under the registry's lock, so two
    restores at once never both take one (code review, 2026-10-16). A watched track whose id
    changes is let go and looked for again.
  - *Watches* begin and end under the feed's lock, so a last watch ending as a new one begins never
    interleaves (code review, 2026-10-16). The External choice saves the id (`ViewSettings::external`,
    0 for none, exclusive with `side_chain`); a chosen track not in the session reads "Track not
    found" and draws nothing until it appears.
  - *Names*: the host's track name through mxm-kit's `Panel::track_info_changed` (nice-plug hands
    CLAP's track-info to the editor object, which lives while the window is closed), else
    "Track n" in the order made; repeats get " 2", " 3"; the list alphabetical, without its own.
  - *Collisions*: only while both analysers hold sound (`Spectrum::live`: a whole window, audio
    within the staleness), at frequencies where both levels are above −80 dB as drawn, each
    smoothed to a third of an octave in power; strength `clamp((E − O + 12)/6) · clamp((O − (peak −
    30))/6)`; drawn as diagonal hatching along the bottom 10 px, each stroke as opaque as the
    strength under it; never on a freeze. Collisions read this track's post spectrum even when
    Show is Pre.
  - *Costs* (release, 2026-10-16): `analyser_cost` with another track's spectrum, worst 0.400 ms a
    frame against 2 ms (192 kHz, High, both); `share_cost`, a watched instance's audio thread at
    192 kHz with 8 bells, 1.21 % of real time against 1.17 % unwatched.
  - *The validator* passes with one warning, `process-audio-denormals`: about 2.7 times longer
    with denormal input, at Init, where the engine leaves the buffer untouched; not explained
    (a build from before F7 was not compared).

- **Several bands** (plan §7.9, decisions 44–47; `editor/selection.rs`):
  - *The model*: `selected` stays the primary, so every single-band path (the card, solo, MOVE's
    edge, F3's grab, the dynamic meter) is unchanged; `others` holds the rest, tidied each frame
    to bands in use (`tidy_selection`), a primary turned Off outside the editor handing over to the
    first left along the axis. `select` takes one band alone, `set_primary` keeps the others.
  - *A group's edit* (`group_edit`): the primary's twelve fields read before and after the edit,
    and the change carried to the others where they have the field, by one ratio (frequency, Q,
    Attack, Release), one offset (gain, Range, Threshold) or set (the discrete fields), held so the
    first member to meet a limit stops the group, the primary pulled back with it. Written as the
    edit was: `send` inside a drag's, the wheel's or the keys' gesture (`Writer::Send`); the card's
    band controls, whose writes close their own action, are wrapped in `group_edit(Instant)`, which
    holds the history open (`History::hold`/`release`, no parameter) so the control's write and the
    group's are one undo step. The menu's commands (a shape, Copy, Cut, Paste, Invert gain, Delete)
    are not group edits: each writes every selected band itself in one `instant`. §7.5's proposal
    pairs are groups now; `Drag.partner` is gone. A parameter a control already holds open (the
    primary's, pulled back at a limit during a card slider's gesture) is set inside that gesture,
    never begun again (code review, 2026-10-16).
  - *Copy and paste*: twelve normalised values per band, in slot order, and the primary's place;
    pasted onto the lowest Off slots in slot order, all or nothing. The clipboard is a static
    mutex, so every instance in the process shares it, and the tests that use it take turns
    (`clipboard_turn`). Ctrl/Cmd+A, C, X, V are read where undo is (`handle_shortcuts`), as
    newDAWn's standard shortcuts outside the keymap, and not while a value is typed (`inert`);
    egui-baseview sends each as a key press.
  - *The title's hover*: mxm-ui's card title takes no hover, so the editor lays a hover-only
    strip over the title's line (`group_names`), the heading's height inside the card's padding.
  - *The group's limits read frequencies as they run* (`plain_heard`): below a rate's top a band
    stored above it is at the top, so it stops the group going up and is not rewritten (code
    review, 2026-10-16).
  - *A hold, not a parameter*: a control's instant write and the group's join one action through
    `History::hold`/`release`, a counter, so no parameter's gesture, abandoned by a restore or not,
    is touched (code review, 2026-10-16).

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

## The host tests (`host-tests/`)

- They hold the bundle's boundary (plan §8): discovered as an effect with no map, the editor
  offered on the player's effect path, Init bit-exact dry in the chain, a restored band heard and
  ringing down to exact silence, a dynamic band cutting the loud note, a state from before F1
  restored static through the wrapper's own `filter_state`, the EQ on the plain stereo
  configuration, a restored Linear phase delaying the render by its latency to the bit, auto gain
  on a restored broadband boost at the dry level, the instance id kept through the host's state,
  and the player's bypass bit-exact dry. They read this repository's own
  `target/bundled/` (with a shared `CARGO_TARGET_DIR`, copy the bundle there first).

## The slower checks

- The analyser's cost: `cargo test -p mxm-fx-eq-n --release --lib analyser_cost -- --ignored
  --nocapture`; the finder's, `finder_cost` the same way; the share tap's, `share_cost`.
- The pictures: `cargo test -p mxm-fx-eq-n --lib pictures`, into `target/layout-tree/mxm-fx-eq-n/`.
- The factory set: `cargo test -p mxm-fx-eq-n --lib write_the_factory_presets -- --ignored`.
- The validator's random parameter fuzz once found a subnormal output, now flushed in the DSP.
- The editor reads what it sent because nice-plug changes a parameter only when its queued event is
  written: the graph, the keyboard's edits and the column's own decisions (its menus, Solo) would
  otherwise read the value from before their own write.
