# The release's decisions

How the circuit model became the CA-72 plug-in. The model's own decisions, made while it
was a device of the DAW it was developed in, are in [history.md](history.md). "The owner"
is the person who commissioned the work; an agent decision is one the owner has not
separately approved.

## R1. A plug-in of its own, the CA-72 by Idle Foundry
**Owner decisions, 2026-10-01.** The circuit-derived model is released as an open-source
plug-in, in this repository, separate from that DAW: the synth core, the SPICE runner, the
analysis, the lab, the netlists and the derivation documents are all here.

- **Nothing trademarked on the product or its interface.** The instrument it models is
  named only as a fact, in the documents (the README's notice). The plug-in is the CA-72;
  its maker, Idle Foundry.
- **The name plate turns over.** A click on it turns it about its long axis, in 0.65 s,
  to a second name, "Model DEEZ", and back. The plate is as wide as that name needs, and
  stays that width for both.
- **Only the real-time quality, Potato.** The model's No Compromises and High Fidelity
  qualities do not run in real time; they stay in the library and the lab, and the
  plug-in has no QUALITY control.

## R2. The panel: the controller in a column, the ticks as printed
**Owner decisions, 2026-10-01.** Of the agent's mock-ups the owner chose the one with the
left hand controller in a full-height column at the panel's left (GLIDE and DECAY at its
top, the wheels below). The ticks were measured again against a photograph, the knobs'
reflections made less pronounced, and the wood runs the whole width with no cheeks
(history.md has the measurements; the DAW's own panel was changed to match).

**Agent decision, 2026-10-01.** The plate is lettered in TeX Gyre Adventor, the panel's
face, 261 units wide. The approved mock-up drew it in Helvetica Bold, 270 wide, only
because its page named a bold font file that did not exist. Not separately approved.

## R3. The rear panel: the side chain, MIDI bend, no CV
**Owner decisions, 2026-10-01.** EXTERNAL INPUT is the host's side chain. The rear
panel's control voltage and trigger jacks are left out. How far a MIDI keyboard's bend
moves the pitch is a parameter, MIDI BEND RANGE (2 semitones by default).

**Agent decision, 2026-10-01.** PHONES and the column's jacks are drawn, as the
instrument has them, but do nothing: the plug-in's output is its only output.

## R4. VST3 and CLAP, GPL-3.0-or-later
**Owner decisions, 2026-10-01.** The formats are VST3 and CLAP, and the licence is the GNU
General Public License, version 3 or later. macOS signing and notarisation wait: the owner
has a Developer ID, and the builds are unsigned until it is set up.

**Agent decisions, 2026-10-01.** The plug-in is built with nih-plug, vendored with two
fixes that clap-validator needs (`third_party/nih-plug/PATCHES.md`). The output is mono,
the same on every output channel, in a stereo or a mono layout. POWER is the host's
bypass: the output fades over 10 ms, and the voice rests once silent. A third fix to
nih-plug came from Steinberg's validator: its VST3 wrapper matched the host's bus
arrangements against the wrong buses, reading past the end of the host's array, and so
refused even the mono layout it offers.

**Evidence (the Mac, 2026-10-01):** `scripts/validate.sh` on the release bundles:
clap-validator 0.4.1, 37 tests passed, 0 failed, 7 skipped; pluginval 1.0.4 at strictness
10, SUCCESS, its editor tests among them; Steinberg's validator (SDK 3.8.1), 47 passed, 0
failed. That validator's bypass test prints two errors and still passes: nih-plug ignores
a value the host sets on the controller while the plug-in is processing, and takes it from
the next block's parameter changes instead. Hosts send both, so it is left as it is.

## R5. The editor, drawn natively
**Owner decision, 2026-10-01.** The editor is drawn in Rust, not in a web view.

**Agent decisions, 2026-10-01.**
- The panel's art is SVG, written by `ca72-panel` and rendered with resvg. What never
  moves is rendered once for a size; each control, lamp and the plate is a layer of its
  own, rendered again only when it changes. The window is baseview's, its pixels shown
  with softbuffer.
- The editor reads the parameters each frame rather than waiting for the host's
  notifications, so automation, presets and undo all show the same way.
- The window resizes from a grip at its bottom right corner, in the panel's proportions,
  and the width is saved with the session. A host cannot resize it (nih-plug does not
  support that).
- **Fixed 2026-10-02** (the owner found that on the Mac a resize shrank the window while
  the panel kept its size, until the editor was opened again). baseview on macOS resizes
  its view when asked but reports no resize of its own making (only a change of scale), so
  the frames stayed at the old size. The editor now applies the new size itself when the
  grip resizes it; on Linux and Windows the report that follows finds it already taken.
  Test: the grip's, every frame at the new size.
- `crates/ca72-panel/tests/approved.png` is the approved mock-up, every control at half
  its travel. The panel as drawn must match it, the plate left out.

## R6. The editor opens at 80 % of the screen
**Owner decision, 2026-10-01.** The editor opened at 1720 logical pixels, the whole width
of a 16-inch MacBook Pro's screen; the owner asked for about 80 %.

**Agent decisions, 2026-10-01.**
- Until the grip resizes it, the editor opens at 80 % of the screen's usable width (less
  the menu bar, dock or taskbar), its height also kept within 80 % of the usable height.
  The screen is macOS's main screen, Windows' primary monitor, or the X11 screen. Where
  the size is unknown, it opens at 1380 pixels.
- The saved width is 0 until the grip sets one, so a session opens to fit whichever
  screen it is on. A session saved before this change keeps the 1720 it stored.

## R7. POLY, the full MIDI range, ANALOG and SPREAD
**Owner decisions, 2026-10-01** (the request made for the DAW's instrument and for this
plug-in alike): POLY switches between the one instrument and one per note, 1 to 10 voices,
easily; every MIDI note plays, 41 to 84 exactly as before; ANALOG and SPREAD, each 0 to
100 % and 0 by default, ANALOG giving each voice its own tolerances on the oscillators'
tuning, the cutoff and EMPHASIS, the contours' times and GLIDE, with the oscillators' and
cutoff's drift; SPREAD placing POLY's voices by their number, the first in the centre; the
voice count never asking more of the machine without saying so. With them all off a
session sounds as before. Ported from the DAW, where each part was measured first.

**Agent decisions, 2026-10-01** (not separately approved):

### POLY
- POLY is a parameter (a switch), VOICES another (2 to 10, 4 by default), both under the
  panel on the plug-in's own strip, not on the instrument's face. Ten voices are built when
  the plug-in is activated (off the audio thread: the first at a new rate takes up to a
  second, the rest are copies), so switching POLY or VOICES while playing allocates
  nothing; switching POLY lifts every key, and the notes sounding stop.
- Each voice is the whole instrument with its own keyboard circuit, which its note reaches
  as one key held while the note is. GLIDE slides from the last key that voice played.
- A new note takes a free voice (one let go whose output has stayed under 1e-6 for a whole
  block) by its number, else the one let go longest ago, else the one held longest.
- A note that takes a voice whose key was still down lifts the key and presses the new one
  once the voice's trigger contact has been open 13 ms: the modelled circuit re-arms its
  trigger only after about 12 ms (Q20 holds the reset line until C7 has drained), so its
  contours start again, as the instrument's S-trigger does. The note's pitch and attack
  come 13 ms after it. `crates/ca72/tests/retrigger.rs` finds the threshold between 11.5
  and 12 ms at 44.1, 48 and 96 kHz.
- The MIDI wheels and the panel reach every voice.
- POLY off is the plug-in as it was, to the sample (`tests/sound.rs`).
- A voice that has fallen silent is not run until its next note: it stops in time, and its
  contours resume from where they were (below the VCA's cutoff), not from rest. The same
  fuzz found that such a voice resumed with DECAY moved to its end could send the decay's
  Newton steps below the -10 V rail, and the follower's bracket check then panicked on the
  audio thread. Now a step that runs off below the rail is taken again from where the
  capacitor was, both nodes' steps held to 0.2 V, else the capacitor holds for that
  sample; every other step is as before. Test: `crates/ca72-plugin/tests/fuzz_bounds.rs`
  (every control at its ends, notes over all 128: a panic and NaN before, six seeds of 200
  rounds clean after).

### The full MIDI range
- Beyond the instrument's 44 keys, the circuit plays its end key (F or C) and the keys
  beyond are added at the keyboard's output, at the output's own volts a key, gliding with
  GLIDE while the trigger is closed: the plug-in's extension, not the circuit's. Moving the
  key string's foot in the circuit instead saturates the keyboard's amplifier (-2.86 and
  +6.92 V), so MIDI 0 to 7 and 122 to 127 each played one pitch.
- Every note's keyboard voltage is within 0.12 mV of the scale, and on 8' every note is
  within 5 cents (`crates/ca72/tests/midi_range.rs`). The oscillators' own limits, at the
  ends: on 2' the top notes run sharp (2.6 cents at MIDI 96, 8.4 kHz, to 53 at MIDI 127,
  50 kHz), on LO the lowest flat (3.7 to 10.5 cents at MIDI 0 to 36), as the circuit does
  there. Lowest-note priority covers all 128.
- A session made before this version may hold notes outside 41 to 84 that were silent;
  they now sound.
- **The converter's ceiling.** clap-validator's `param-fuzz-bounds` found that a key far
  beyond the 44 with an oscillator's RANGE at 2', FREQUENCY and the PITCH wheel at their
  tops (about 100 kHz) drove the exponential converter past where its model holds: above
  about 1e-2 A R42's feedback takes the tail node past any rail, the solve went to NaN and
  the voice stayed silent for good. The timing current is held at 5 mA
  (`vco::TIMING_MAX`, twice MIDI 127's at the panel's centre; up to it the solve converges),
  its Early slope as the converter gives it there. Below it nothing changes. Test:
  `crates/ca72/tests/midi_range.rs`, the case in every mode (NaN before, held after).

### ANALOG and SPREAD
- The DAW's character, the same draws and arithmetic
  (`crates/ca72-plugin/src/character.rs`): each voice's parts drawn once from its seed (the
  first voice's is the session's seed, as with one voice). At 100 % (twice the character's
  figures, `ANALOG_DEPTH`: the owner found them too subtle, 2026-10-02; 50 % is what 100 %
  was): each oscillator's tuning spread 8 cents (an offset at its range tap), the cutoff
  0.08 octave (at R51, 0.98 V an octave), EMPHASIS, the contours' ATTACK and DECAY and
  GLIDE 0.024 of their rotation; each oscillator drifts (a mean-reverting walk, 6 cents, 4
  s) and wavers (0.8 cent under 30 Hz), the cutoff drifts (0.03 octave). Sustains, levels
  and waveshapes are left alone.
- SPREAD places voice n at 0, -1, +1, -0.5, +0.5, -0.75, +0.75, -0.25, +0.25, -0.125 times
  the amount, a voice in the centre whole on both sides. The output became stereo for it;
  on a mono output it is the two channels together, so a session with SPREAD at 0 sounds
  as before on either.
- SPREAD is drawn at half opacity while POLY is off (the owner, 2026-10-02, approved
  greying it out for a patch of a single voice): the one voice is in the centre, so it does
  nothing. It stays operable (agent's decision), to be set before POLY goes on. The strip
  has no tips, so it is dimmed only. Test: `strip::tests::spread_dims_while_poly_is_off`.
  (A stereo spread of the three oscillators in one voice was weighed and dropped: they are
  summed before the one filter, so it would need a second mixer, filter and VCA.)

### POLY in real time
- **Since R11** (the owner's decision, 2026-10-02): POLY's voices are shared between the
  host's audio thread and up to four threads of the plug-in's own, and the strip's count
  measures the engine with them. What follows is how it stood before, with its figures.
- POLY's voices run in turn on the host's audio thread. The plug-in starts no threads of
  its own: the host sizes its own threads to the machine, and a plug-in's busy threads
  beside them would be more than the machine has.
- When it is activated, the plug-in measures a voice's cost in a block where its key
  changes, at the host's rate and largest block, and counts how many such fit in 60 % of
  the block's period (the DAW's figure). The strip shows that number beside VOICES and
  marks VOICES when it asks for more ("expect dropouts"). **Since R12** the strip shows no
  count and the plug-in measures none when activated.
- Measured (`tests/poly_bench.rs`: the engine as a host calls it, on one promoted thread,
  48 kHz, changing chords every half second with the modulation moving, ANALOG 50 %,
  SPREAD 70 %, 60 s, a block a period and never back to back; two clean runs a count):
  - **The Linux reference machine** (Ryzen 7 7800X3D), 256 frames: **4 voices** with no
    late block in two runs (mean 2.5 ms of 5.33, worst 4.7); 5 voices 1 late block in a
    minute, 6 voices 87, 8 voices 1,821, 10 every block. At 512 frames 8 voices were still
    late 842 times. The strip's own count there: 4 at 256 frames.
  - **The Mac** (M4 Pro): not measured cleanly. Ableton Live was running and playing there,
    and the block's time hardly changed from 3 to 6 voices, so the bench thread was held
    back rather than paying for the voices. The strip's own count there: 7 at 256 frames.
    To be measured again on a quiet Mac.
  - No run was demoted by the watchdog.
- **Evidence (the Mac, 2026-10-02, the release bundles of this version):** clap-validator
  0.4.1, 37 passed, 0 failed, 7 skipped (before the two fixes `param-fuzz-bounds` failed
  every time); pluginval 1.0.4 at strictness 10, SUCCESS, its editor tests skipped this
  time (`--skip-gui-tests`: the Mac was in use, and they open windows on its screen);
  Steinberg's validator (SDK 3.8.1), 47 passed, 0 failed. The workspace's tests and clippy
  pass on the Mac and on Linux. Linux's validators were not run locally (the Linux
  reference machine has no cmake or xvfb-run); CI runs them.
- So VOICES defaults to 4, what the Linux reference machine plays at 256 frames; the strip
  shows each machine's own count (a voice costs about the same share of any block, so a
  larger block barely changes it: 4 at 1024 frames on that machine too).

## R8. FEEDBACK: the phones' VOLUME knob
**Owner decisions, 2026-10-02.** The instrument has no overdrive control; players cable its
PHONES output into EXTERNAL INPUT and turn the phones' VOLUME up, overdriving the external
input's preamplifier and the mixer. The owner asked for a feedback control, and then for it
to take the place of the unused volume knob: the phones' VOLUME, which did nothing (R3), is
now FEEDBACK, its legend reading FEEDBACK. The same was done in the DAW's instrument.

**Agent decisions, 2026-10-02** (not separately approved):
- FEEDBACK (`feedback`, 0 to 10 on the knob's dial, 0 by default: no cable) sends each
  voice's output, before MAIN OUTPUT's VOLUME, into its EXTERNAL INPUT one sample later (a
  loop cannot be instantaneous), added to the side chain there. The level sent follows the
  knob's taper (the owner, 2026-10-02, found 3 on the dial already very intense; that
  should come at about 6): `feedback_law` = travel^2.357, so 6 sends 30 % of the output, 3
  about 6 %, 5 20 %, 10 all of it. The parameter (what automation sees) is the knob's
  position.
  As with the cable it is heard through the MIXER's EXTERNAL INPUT switch and VOLUME, and
  OVERLOAD lights.
- While FEEDBACK is on the preamplifier is solved as High Fidelity solves it: Potato's
  (its paths fitted to the circuit's gain) sent the loop into a low, dark oscillation the
  circuit does not make. With it Potato sounds as High Fidelity does at 30 % (level within
  1 %, brightness 30.5 % against 28.7 % above 1 kHz; `crates/ca72/tests/feedback.rs`).
  **Since R11** (the owner's decision, 2026-10-02): inside the loop Potato's preamplifier is
  its own model at twice the rate, its output as late as the circuit's
  (`preamp::Delayed`), within the circuit's own spread in the loop.
- Cost: a voice about 9.6 us a sample with FEEDBACK on, against 1.8 without (the Linux
  reference machine): about half a core at 48 kHz. The strip's real-time count is measured
  both ways at activation and shows the one in force: on that machine at 256 frames, 4 POLY
  voices without FEEDBACK, none with it. One voice (POLY off) with FEEDBACK costs about
  half the period. **Since R11:** FEEDBACK costs a voice about a fifth more.
- The approved panel (R2, `crates/ca72-panel/tests/approved.png`) differs only at that knob
  and its legend; the comparison still passes within its limits.
- The knob is dimmed while EXTERNAL INPUT is closed (the owner, 2026-10-02, approved: in a
  test the knob seemed to do nothing, as EXTERNAL INPUT was off, and the owner asked for it
  to be greyed out unless the external input is switched on and its volume above zero).
  While the mixer's EXTERNAL INPUT switch is off or its VOLUME is at 0 the knob is drawn at
  half opacity, as the parts with no parameter are, and its tip reads "FEEDBACK (silent:
  switch EXTERNAL INPUT on in the MIXER and turn its VOLUME up)". It stays operable
  (agent's decision), so the amount can be set first. The sound is unchanged. Tests:
  `controls::feedback_silent`'s cases and `render::tests` (the knob's pixels dimmed,
  nothing else on the panel changed).
- Tests: `crates/ca72/tests/feedback.rs` (not heard with EXTERNAL INPUT off; OVERLOAD lit and
  the sound brighter in every mode; Potato as High Fidelity; finite and within the rails
  with every control feeding the loop at its end), the threaded test (the same samples to
  the bit), the engine's bounds fuzz (`tests/fuzz_bounds.rs`, FEEDBACK at its ends too).

## R9. A floor of mismatch, and LOCK
**Owner decisions, 2026-10-02** (with the DAW's instrument). With ANALOG at 0 the three
oscillators were identical: at one pitch they locked in phase and sounded up to 3.5 dB
louder than with ANALOG on, which invites judging by loudness. The owner chose a small
built-in mismatch, with a switch that makes the oscillators perfectly identical, turned
down to match so it cannot win by loudness, and a tooltip.

**Agent decisions, 2026-10-02** (not separately approved):
- With LOCK off each oscillator keeps a floor of its own (`ANALOG_FLOOR`, 0.75 of the
  character's figures: a tuning spread of 3 cents, 2.25 cents of drift, 0.3 cent of waver);
  ANALOG adds to it. In the DAW the level then moves 0.4 dB across ANALOG's range.
- LOCK (`lock`, off by default) makes the oscillators the circuit as drawn and turns the
  output down by `lock_trim`: the switched-on oscillators that share a pitch and their
  volumes give the ideal gain of locking, of which 0.68 shows at the output (`LOCK_SHARE`,
  fitted in the DAW). Measured: three in unison +0.4 dB with LOCK, one oscillator 0.0
  (`tests/sound.rs`); octaves lock only in part and are left, about 0.7 dB.
- The plug-in has no rear panel, so LOCK is a parameter without a control on the panel: a
  host shows it in its list of parameters ("Lock (oscillators identical)").
- A session saved before this version, with ANALOG at 0, now has the floor (the plug-in is
  not yet released; the DAW keeps its older devices exact).

## R10. Presets: a library shared with the DAW, a bar and a drawer
**Owner decisions, 2026-10-02** (with the DAW's instrument). The owner asked for a few
factory presets, from well-regarded published patches under new names; for every preset to
be created, edited and deleted; and for favourites, search and tags. The plug-in gets the
same full browser as the DAW's (built there first); the two share one library; the presets
drop down from the bottom, in a drawer (agreed: a bar under the strip, a drawer sliding up
over the panel, not growing the window, which hosts may refuse).

**Agent decisions, 2026-10-02** (not separately approved):
- **A preset is named panel settings**: the dials' plain values by parameter ID (a choice
  by its index, a switch 0 or 1), POLY and VOICES too, never the PITCH wheel or the bypass;
  the MOD wheel is part of one. The plug-in's IDs and ranges are the DAW's instrument's,
  so one file serves both. Choosing one sets every other parameter to its default, POLY and
  VOICES only when it names them; each parameter changed is a gesture of its own. **Since
  R12** POLY, VOICES, ANALOG and SPREAD are set like the rest, and every factory preset
  names them.
- **The factory's 17** are `crates/ca72-plugin/sounds/minimoog.toml`, the DAW's file byte
  for byte (the DAW's record gives each one's source, their levelling and the adjustments
  made on the model). Built in, never changed.
- **The user's are files** in `<data>/Idle Foundry/CA-72/Presets` (`$IDLE_FOUNDRY_SHARED`
  overrides the `Idle Foundry` folder), `<name>.toml`: `format`, `name`, `description`,
  `tags`, `favorite`, `replaces`, `values`; written whole and moved into place. The factory
  presets' tags, favourites and deletions are the folder's `factory.toml`; saving over or
  renaming a factory preset writes the user's version standing in for it (REVERT takes it
  back); deleting one hides it (RESTORE FACTORY). Names: 1 to 64 characters, none of
  `/ \ : * ? " < > |`, unique ignoring case. The DAW reads and writes the same files the
  same way (checked both ways with each program's own writer: each lists the other's
  presets, tags and favourites, and sets the same normalized values).
- **The plug-in remembers its preset** (`preset`, a persisted field saved with the
  session); the bar marks it • once a parameter leaves the preset's value.
- **The window grows by the bar** (80 panel units): 617 pixels tall at 1720 wide, was 577.
  **Since R12** the bar is the strip's left, and the window is 577 again.
- **The drawer takes the keyboard** while open (baseview's focus on opening and on a
  field): typing goes to the field with the caret (the search when none), the arrows step
  through the list setting each preset, Enter commits, Escape takes back an edit or
  closes, Tab moves between the search and the save fields. Keys with Command or Control
  stay the host's, and with the drawer shut every key is.
- **Drawn** with the strip's colours and the panel's typeface; the star and the arrows are
  shapes (the typeface has no such glyphs).
- **Tests:** `library::tests` (saved, found, tagged, made favourite, renamed, removed; a
  factory preset edited, renamed, reverted, deleted, restored; a file of the DAW's read, an
  unreadable one reported), `ca72-panel` `presets::tests` (fields edit at the caret, every
  control found where drawn, the caret placed by a click, the frames drawn), and the
  editor's (the bar opens the drawer over the panel, a row sets the parameters and is
  remembered, typing searches, the arrows and the bar's step, a chip filters, the wheel
  scrolls the list, SAVE… saves with tags, RENAME and TAGS in place, DELETE after asking,
  RESTORE, the host's shortcuts left to it).
- Checked on the Mac, 2026-10-02: clap-validator, pluginval at strictness 10 (its GUI tests
  skipped while Live was open) and Steinberg's validator pass; in Live the drawer takes the
  keyboard and the search works (the owner).

**Owner decisions, 2026-10-02, later** (trying it in Live): the drawer opens downward,
extending the plug-in's window; the strip's "NOT THE INSTRUMENT'S" read oddly: it says
CA-72; a double click on a preset chooses it and closes the drawer.

**Agent decisions:**
- The drawer opens below the bar: the editor asks the host to resize (as the grip does),
  its size counting the drawer while it is open (`drawer_height`: 845 panel units, 423
  pixels at 1720 wide), and the window follows at once (baseview on macOS reports no
  resize of its own making: R5). It slides down from under the bar; it shuts at once, the
  window shrinking back. A host that will not resize keeps the window and the drawer
  slides up over the panel's lower part as before; the bar's arrow points where the
  drawer goes. Tests: the window's height with the drawer and without, a double click, a
  host refusing to resize.

## R11. Ten POLY voices in real time
**Owner decisions, 2026-10-02.** Every preset should play ten POLY voices on the owner's
Mac without trouble, as a VST3 in Ableton Live; on Windows in Live, POLY had dropped out
constantly. The Mac (Apple M4 Pro) is the acceptance machine, Live's buffer there 256
samples: in Ableton Live 12 at 48 kHz, the VST3 with POLY on and VOICES at 10 plays each of
the 17 factory presets with ten-note chords (held, and changing every half second) for 60 s
with no dropout and no CPU overload, at 256 samples. Headless, each preset at ten voices
takes at most 60 % of the audio thread's period, worst block included. Shown both
measurements, the owner approved:
- **Worker threads of the plug-in's own, on by default**, reversing R7's "the plug-in starts
  no threads".
- **FEEDBACK's new preamplifier model** inside the loop (below): not the same samples as
  R8's, but within the circuit's own spread.

**Agent decisions, 2026-10-02** (not separately approved):

### Measuring
- `tests/preset_cost.rs` (the engine as a host calls it, as fast as it goes on one thread)
  and `tests/poly_bench.rs` (paced, a block a period on a promoted thread) as before, now
  playing their blocks through `Engine::render` between events, as the plug-in does.
  `poly_bench` also plays a factory preset (`CA72_PRESET`), shares the voices with workers
  (`CA72_WORKERS`, each block given the plug-in's deadline) and can put its own thread in a
  work interval as a CoreAudio IO thread is (`CA72_HOST_WG`). Both share the presets'
  controls and chords (`tests/common`).
- `tests/preset_render.rs` renders every factory preset at ten voices and compares it with
  a reference render: the same to the bit, or the difference's RMS and peak, the level and
  the largest third-octave band's change. Every change below was checked against the code
  before it this way.
- The Mac with Live closed, but not idle: other applications kept the load average at 4 to
  6 throughout (the window server among them). The same conditions before and after.

### Before (the Mac, 2026-10-02, the code before this record)
Each preset at ten voices, 256-frame blocks at 48 kHz (`preset_cost`, one thread, as fast
as it goes; the share of one core real time takes, and the worst block's share of the
period), and `poly_bench`'s own panel at ten voices (paced, 60 s):

| | Mean, of the period | p99.9 | Worst | Late blocks |
|---|---:|---:|---:|---:|
| 256 frames (5.33 ms) | 88.0 % | 135 % | 138 % | 1,706 of 11,250 |
| 512 frames (10.67 ms) | 88.1 % | 130 % | 135 % | 764 of 5,625 |

The strip's count: 6 voices at 256 frames, none with FEEDBACK on. `preset_cost` per preset
is in the first columns of the table under "After".

### FEEDBACK nearly free
- Inside the loop the circuit's preamplifier (R8) cost a voice four to eight times over:
  Undertow Growl took 694 % of a core at ten voices, Pulse Strut 349 %.
- Why Potato's plain model misbehaved in the loop (R8): mostly time. The circuit's four
  substeps a sample run between halfband resamplers that delay its output 44.5 samples; the
  plain model has none, so its loop is some 45 samples shorter and falls into another
  oscillation. Recorded inside the loop (the circuit's input, sample by sample) and replayed
  open-loop, the plain model's output differs from the circuit's by -2 to +5 dB (the
  error's power against the signal's); its equations run at four times the rate inside the
  circuit's own resamplers, by -19 to -36 dB (`crates/ca72/src/voice/loop_lab.rs`).
- **The model** (`preamp::Delayed`, `InLoop::Delayed`, the default in Potato with FEEDBACK
  on): the plain model's equations at twice the rate between sparse halfband resamplers,
  its input delayed so that its output comes 44.5 samples late, as the circuit's does; the
  input's coupling, C20 and the lamp at the sample rate. Built with the preamplifier, so
  switching FEEDBACK on allocates nothing. A load-dependent low clip (R57 against R46 through
  C20, which sets the circuit's lower limit) was tried and left out: no closer.
- **Proof.** Open-loop on the recorded inputs, FEEDBACK 3, 6 and 10, EXTERNAL INPUT VOLUME
  5 and 10, both FEEDBACK presets: the error -21 to -40 dB (plain: -2 to +5). In the loop
  the sound is chaotic (the circuit's own renders differ from one ANALOG seed to the next by
  up to 8 dB in a band), so the models were compared by their statistics over six seeds
  (`tests/feedback_models.rs`):

  | Preset, FEEDBACK, VOLUME | Level, dB: circuit | new | plain | Above 1 kHz, %: circuit | new | plain | Under 150 Hz, %: circuit | new | plain | Worst band, dB: new | plain |
  |---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
  | Pulse Strut, 3, 5 | -25.20 ±0.12 | -25.21 | -25.30 | 5.2 ±0.1 | 5.2 | 5.5 | 66.0 ±1.3 | 66.0 | 67.5 | -0.2 | -3.6 |
  | Pulse Strut, 3, 10 | -17.21 ±0.16 | -17.21 | -16.30 | 7.9 ±0.4 | 7.9 | 5.6 | 57.5 ±1.6 | 57.2 | 76.1 | -0.7 | -18.7 |
  | Pulse Strut, 6, 5 | -21.37 ±0.12 | -21.37 | -21.75 | 11.5 ±0.3 | 11.5 | 12.1 | 48.0 ±1.7 | 47.8 | 59.3 | -0.5 | +15.3 |
  | Pulse Strut, 6, 10 | -16.04 ±0.11 | -16.14 | -15.77 | 5.0 ±0.5 | 5.4 | 3.2 | 67.6 ±2.0 | 66.2 | 82.6 | -1.4 | -16.3 |
  | Pulse Strut, 10, 5 | -17.13 ±0.14 | -17.18 | -16.35 | 7.6 ±0.3 | 7.7 | 5.4 | 60.2 ±1.5 | 59.6 | 77.8 | -0.6 | -18.8 |
  | Pulse Strut, 10, 10 | -15.99 ±0.09 | -16.10 | -15.78 | 4.0 ±0.3 | 4.4 | 2.3 | 71.9 ±1.2 | 69.8 | 85.1 | -1.5 | -15.5 |
  | Undertow Growl, 3, 5 | -24.12 ±0.44 | -24.12 | -23.88 | 10.8 ±1.1 | 10.8 | 13.7 | 72.0 ±2.0 | 71.9 | 68.3 | +0.2 | -3.6 |
  | Undertow Growl, 3, 10 | -10.33 ±0.06 | -10.30 | -10.86 | 34.8 ±0.6 | 34.2 | 46.4 | 13.7 ±1.6 | 14.1 | 17.9 | +0.9 | -22.7 |
  | Undertow Growl, 6, 5 | -16.94 ±0.18 | -16.90 | -15.53 | 48.6 ±2.3 | 48.7 | 53.4 | 24.8 ±2.8 | 24.3 | 16.3 | +0.3 | +13.4 |
  | Undertow Growl, 6, 10 | -9.50 ±0.02 | -9.51 | -9.57 | 23.4 ±2.7 | 21.6 | 6.1 | 18.8 ±9.8 | 24.0 | 79.6 | +2.8 | -22.7 |
  | Undertow Growl, 10, 5 | -10.52 ±0.08 | -10.49 | -10.93 | 34.6 ±2.0 | 34.2 | 45.0 | 17.3 ±3.5 | 17.6 | 20.6 | +1.0 | -22.3 |
  | Undertow Growl, 10, 10 | -9.46 ±0.03 | -9.49 | -9.60 | 20.3 ±3.9 | 21.2 | 7.1 | 27.3 ±12.6 | 25.2 | 78.1 | -1.0 | -22.7 |

  (The circuit's mean over the six seeds and its spread, one standard deviation; the new
  model's and the plain one's means over the same seeds; the largest change of a
  third-octave band, the power over the seeds, among the bands within 40 dB of the
  strongest.) The OVERLOAD lamp's mean is the same for all three in every case (dark at
  FEEDBACK 3 and VOLUME 5, lit otherwise). Two of the circuit's own renders, seeds 1 and 2,
  differ by up to 8.2 dB in a band (Undertow Growl, FEEDBACK 6, VOLUME 10).
  At VOLUME 0 nothing reaches the preamplifier and the three are the same. R8's test passes
  as before: Potato against High Fidelity at 30 %, level 0.540 and 0.540, brightness 30.6 %
  and 28.7 % (R8 had 30.5 %).
- **Cost:** FEEDBACK now adds what the overdriven voice itself costs (its VCA's pairs, the
  filter, harder driven), about a fifth: Undertow Growl 100 % of a core at ten voices on one
  thread (84 % with FEEDBACK at 0, measured with this change), Pulse Strut 88 %.

### A voice cheaper, the same samples
- **A voice at a time.** `Engine::render` plays the samples between one event and the next:
  each POLY voice 128 samples in turn rather than every voice a sample at a time, its state
  (some 30 KB) then staying in the nearest cache, the voices summed in the same order. About
  3 % on the Mac; it is also the shape a block takes to share the voices among threads.
- **Less work, to the bit:** the noise generator not run while nothing hears it (its mixer
  channel off and the modulation mix's gain from it exactly zero, or neither modulation
  switch on; it resumes where it stopped, as an unheard oscillator does); the filter's
  Jacobian and output gradient kept from step to step (the entries always zero stay zero)
  and its first-order correction taken over the entries that are not; the bus
  conductance's resampler skipped while the conductance holds; ANALOG's per-sample
  constants worked out once a rate; the contours' transistor parameters looked up once a
  tick. 2 to 10 % (Wooden Mallet the most).
- **Measured and left out:** link-time optimisation (`lto = "fat"`, one codegen unit): 3 to
  4 % faster, but every test binary of the workspace would link that way too (R13: the
  bundles alone are, now); caching the
  filter's per-bias divisions and reading its two output tables with shared weights, and an
  oscillator's repeated exponential: no gain the measurement could see.
- **Where a voice's time goes** (Three Saw Slab): the filter about a third, the
  oscillators a fifth (the reverse sawtooth among them), the VCA a sixth, the contours a
  tenth; the noise, the keyboard, the VCA's bias and the control node a few percent each.
  Running the contours eight times less often saved 9 to 15 %. A key change costs a voice
  about a third more in its block: on a note the keyboard circuit's Newton solve after its
  contacts jump, on a release the contours' trigger section. No further change found here
  halves a voice without changing its sound; the threads below do the rest.

### Worker threads
- **How** (`crates/ca72-plugin/src/pool.rs`; safe Rust, as `ca72::threaded`): the host's
  audio thread and up to four workers share each run of samples between events. Each voice
  sits behind a mutex that the audio thread only ever tries; the engine keeps each voice's
  note, gate and age itself, so events never wait on a voice. A run is offered through
  atomics: one word holds its number, its voices and the next to take, so a voice is taken
  by a compare-and-swap on it and a worker late from one run can take nothing from the
  next. The audio thread takes voices too, until none is left (any voice no worker has
  started, it plays itself), then waits for the workers' voices, spinning, yielding, then
  napping, until the block's deadline, 0.75 of its period, and no longer: a voice not done
  by then is silent for that run and sits out until its worker lets it go; a change meant
  for it (its panel, a lifted key) waits for it. Each voice's samples go to its own buffer
  and the audio thread sums them in the voices' order, so the output is the same to the bit
  with any number of workers (`tests/workers.rs`; `preset_render.rs`, all 17). Nothing
  allocates on the audio thread (`tests/realtime.rs`).
- **When:** with POLY on, two voices or more sounding and a run of at least 8 samples
  (waking the workers takes microseconds). POLY off, the one voice stays on the host's
  thread.
- **Threads:** started when the plug-in is activated (not on the audio thread), a third of
  the processors less two, at most four (four on the Mac's 14 and on the i5-13600K's 20,
  two on 8, none below 5), made audio threads for the host's largest block
  (`ca72_rt::promote`: on macOS the time-constraint policy, on Linux `SCHED_FIFO` with the
  watchdog, on Windows time-critical priority). They spin 200 us after a run (the block's
  next run follows at once), then park; the audio thread wakes them for each run.
- **A work interval of their own (macOS).** Paced as audio is, a thread busy a third of each
  period ran about half as fast as one busy for most of it: the system's performance
  controller does not raise the clock for threads it does not know work to a deadline (on
  one paced thread a voice's 256 samples took 708 us with 3 voices sounding, 433 with 10).
  The workers therefore join an `os_workgroup` interval of their own
  (`AudioWorkIntervalCreate`; `ca72_rt::WorkInterval`), which the first of them opens at a
  block's first run with the block's period as its deadline and closes when the block's runs
  are done; a worker leaves it before it ends. A plug-in cannot join its host's audio
  workgroup: VST3 has no way to hand it over and nih-plug's CLAP wrapper none either; nor
  can it put the host's thread into one safely (a thread that ends while joined stops the
  process). How fast the host's own thread runs is the host's: in the bench, with its thread
  in an interval of its own (as a CoreAudio IO thread is), the worst blocks fall by about a
  third.
- **The strip's count** (**since R12** not shown, nor measured when the plug-in is
  activated; `engine::realtime_voices` stays for `poly_bench`) measures the engine itself
  with its workers, paced at the block's period on an audio thread of its own for a moment,
  as a host plays it (chords of as many notes as voices tried, a new one every few blocks,
  the voices still sounding taken): the most voices whose key-change blocks take, at their
  median, no more than 60 % of the period; ten tried first, then down from the estimate,
  about a fifth of a second each, once a process for each rate, block, FEEDBACK and number
  of workers. That thread is not in a workgroup, so the count does not count on the host's
  being in one. On the Mac: 10 at 256 and 1024 frames, FEEDBACK on too; 4 with the host's
  thread alone (R7's paced figure on the Linux reference machine was 4).
- **Tests:** a voice still held by a late worker sits out and the block is not held up
  for it; a worker past the deadline is not waited for; the same samples with and without
  workers; no allocation with workers, FEEDBACK switched, VOICES changed and POLY switched.

### After (the Mac, 2026-10-02)
**`preset_cost`** (each preset at ten voices, 256 frames, as fast as it goes; the share of
one core real time takes and the worst block's share of the period; before: the code before
this record, the circuit's preamplifier in FEEDBACK's loop; after: this record, one thread
and with four workers, which here are ordinary threads since the measurement is not paced):

| Preset | Before: one thread | worst block | After: one thread | worst block | Four workers | worst block |
|---|---:|---:|---:|---:|---:|---:|
| Bass | 67 % | 112 % | 59 % | 105 % | 14 % | 27 % |
| Lead | 75 % | 123 % | 68 % | 117 % | 16 % | 26 % |
| Three Saw Slab | 83 % | 131 % | 78 % | 123 % | 18 % | 30 % |
| Elastic Octaves | 72 % | 239 % | 67 % | 114 % | 15 % | 26 % |
| Pulse Strut | 349 % | 519 % | 88 % | 141 % | 21 % | 96 % |
| Undertow Growl | 694 % | 1040 % | 100 % | 208 % | 22 % | 34 % |
| Upright Pluck | 63 % | 110 % | 59 % | 213 % | 13 % | 25 % |
| Hollow Glider | 89 % | 140 % | 80 % | 127 % | 19 % | 30 % |
| Cruising Whistle | 80 % | 130 % | 74 % | 151 % | 17 % | 31 % |
| Stacked Fifths | 93 % | 141 % | 84 % | 200 % | 20 % | 78 % |
| Ringing Saw Line | 70 % | 119 % | 61 % | 123 % | 14 % | 25 % |
| Slow Horn Swell | 69 % | 279 % | 64 % | 112 % | 15 % | 87 % |
| Brass Tutti | 85 % | 134 % | 77 % | 154 % | 17 % | 29 % |
| Breath Flute | 67 % | 113 % | 62 % | 108 % | 14 % | 25 % |
| Wooden Mallet | 63 % | 102 % | 53 % | 92 % | 12 % | 21 % |
| Shoreline Wash | 73 % | 110 % | 68 % | 207 % | 15 % | 24 % |
| Slow Bow | 67 % | 125 % | 62 % | 114 % | 14 % | 26 % |

(Run as fast as it goes, its worst blocks include the other applications' turns: the
workers here are ordinary threads, so 78 to 96 % in three presets is that; the paced runs
below, on audio threads, are the measure.)

**`poly_bench`, its own panel** (ten voices, paced on a promoted thread, 60 s):

| | Mean | p99.9 | Worst | Late blocks |
|---|---:|---:|---:|---:|
| 256 frames, before | 88.0 % | 135 % | 138 % | 1,706 of 11,250 |
| 256 frames, one thread | 82.7 % | 132 % | 138 % | 781 of 11,250 |
| 256 frames, four workers | 33.2 % | 65 % | 75 % | 0 of 11,250 |
| 512 frames, before | 88.1 % | 130 % | 135 % | 764 of 5,625 |
| 512 frames, one thread | 81.6 % | 123 % | 127 % | 361 of 5,625 |
| 512 frames, four workers | 31.4 % | 57 % | 61 % | 0 of 5,625 |

**`poly_bench`, every preset** (ten voices, four workers, paced on a promoted thread: 30 s
at 256 and 512 frames with the bench's own thread outside any workgroup, and 20 s at 256
with it in a work interval of its own, as a CoreAudio IO thread is; shares of the period):

| Preset | 256: mean | p99.9 | worst | late | 512: mean | worst | late | 256, host in an interval: mean | worst |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Bass | 32 % | 68 % | 74 % | 0 of 5,625 | 31 % | 62 % | 0 of 2,812 | 25 % | 47 % |
| Lead | 38 % | 68 % | 75 % | 0 of 5,625 | 32 % | 60 % | 0 of 2,812 | 28 % | 49 % |
| Three Saw Slab | 42 % | 71 % | 93 % | 0 of 5,625 | 31 % | 59 % | 0 of 2,812 | 29 % | 47 % |
| Elastic Octaves | 37 % | 72 % | 82 % | 0 of 5,625 | 33 % | 62 % | 0 of 2,812 | 27 % | 51 % |
| Pulse Strut | 39 % | 75 % | 81 % | 0 of 5,625 | 31 % | 66 % | 0 of 2,812 | 31 % | 51 % |
| Undertow Growl | 41 % | 71 % | 79 % | 0 of 5,625 | 31 % | 58 % | 0 of 2,812 | 31 % | 50 % |
| Upright Pluck | 33 % | 66 % | 73 % | 0 of 5,625 | 30 % | 75 % | 0 of 2,812 | 22 % | 49 % |
| Hollow Glider | 42 % | 73 % | 75 % | 0 of 5,625 | 31 % | 59 % | 0 of 2,812 | 29 % | 49 % |
| Cruising Whistle | 40 % | 74 % | 82 % | 0 of 5,625 | 31 % | 56 % | 0 of 2,812 | 29 % | 48 % |
| Stacked Fifths | 43 % | 73 % | 76 % | 0 of 5,625 | 31 % | 56 % | 0 of 2,812 | 31 % | 49 % |
| Ringing Saw Line | 34 % | 66 % | 78 % | 0 of 5,625 | 30 % | 56 % | 0 of 2,812 | 25 % | 45 % |
| Slow Horn Swell | 37 % | 69 % | 84 % | 0 of 5,625 | 30 % | 58 % | 0 of 2,812 | 25 % | 49 % |
| Brass Tutti | 41 % | 73 % | 84 % | 0 of 5,625 | 31 % | 58 % | 0 of 2,812 | 29 % | 47 % |
| Breath Flute | 36 % | 68 % | 76 % | 0 of 5,625 | 29 % | 58 % | 0 of 2,812 | 26 % | 47 % |
| Wooden Mallet | 31 % | 59 % | 65 % | 0 of 5,625 | 27 % | 59 % | 0 of 2,812 | 21 % | 41 % |
| Shoreline Wash | 39 % | 68 % | 73 % | 0 of 5,625 | 30 % | 57 % | 0 of 2,812 | 25 % | 45 % |
| Slow Bow | 35 % | 68 % | 75 % | 0 of 5,625 | 31 % | 58 % | 0 of 2,812 | 24 % | 51 % |

No preset had a late block. With the host's thread in an interval every preset's worst
block is under 60 % of the period (41 to 51 %); with it outside one, the means stay under
half the period but the worst blocks, at the chord changes, reach 65 to 93 % at 256 frames
and 56 to 75 % at 512.

**The strip's count:** 10 voices at 256 and 1024 frames, FEEDBACK on too (6 and none
before); 4 with the host's thread alone.

**Old against new** (`preset_render`, every factory preset at ten voices, 4 s, against a
render of the code before): 15 the same to the bit; Pulse Strut and Undertow Growl, whose
FEEDBACK loop now runs the new preamplifier model: level +0.10 and -0.04 dB, the largest
third-octave band's change -0.91 dB at 1.26 kHz and +1.31 dB at 20 kHz; their difference
signals are about as loud as the renders (+0.4 and +2.5 dB), as two renders of a chaotic
loop are once they part (above).

**Evidence (the Mac, 2026-10-02, the release bundles of this version, Live closed):**
clap-validator 0.4.1, 37 passed, 0 failed, 7 skipped; pluginval 1.0.4 at strictness 10,
SUCCESS, its editor tests run this time; Steinberg's validator (SDK 3.8.1), 47 passed, 0
failed. The workspace's tests (155 passed, 0 failed, 20 by hand) and clippy pass on the
Mac. The bundles are installed for Live.

### Limitations
- The host's own thread: in a host whose audio threads are not in a workgroup, its share of
  the voices runs slower (paced and lightly loaded, the system clocks it down), and the
  worst blocks at a chord change rise to 65 to 93 % of the period at 256 frames (no block
  late in these runs); with it in one, 41 to 51 %. Whether Live 12's threads are is for the
  check in Live to show.
- Windows and Linux were not measured for this record (the Linux reference machine and the
  Windows one, an i5-13600K): the
  workers start there too (four on the i5's 20 processors), without a work interval.
- A worker held up past the deadline (a machine overloaded by other work) costs that voice
  its samples for the run: a gap in one voice rather than a dropout of all.
- VOICES still defaults to 4.

### For the DAW's instrument (its own copy of the model; porting left for later)
- The FEEDBACK finding and model, if its loop runs the circuit's preamplifier in Potato
  (`preamp::Delayed`, `InLoop`; the open-loop lab and the in-loop comparison).
- The bit-exact savings: the noise while unheard (`ControlPart`), the filter's sparse
  Jacobian and held conductance (`vcf.rs`), the character's constants (the DAW's character
  module), the contours' devices once a tick.
- The engine's runs between events and its threads belong to the DAW's own engine and pool,
  not to the model; the macOS work interval (`ca72_rt::WorkInterval`) is a candidate for
  the DAW's real-time threads.

## R12. One row under the panel; a preset holds POLY, VOICES, ANALOG and SPREAD
**Owner decisions, 2026-10-02**, trying R11's build in Live: the strip's real-time count
("10 in real time here ..."), which the owner found unhelpful and a waste of space, is
gone, and CA-72's label with it, the presets' selector brought up into the strip's row; the
presets' list scrolls with the mouse wheel and touch gestures (it did with neither); POLY,
VOICES, ANALOG and SPREAD are saved with a preset.

**Agent decisions, 2026-10-02** (not separately approved):
- **One row.** The strip's left (to 1110 panel units) is the presets' selector: its star,
  the previous preset, the name (680 units, the drawer's arrow inside), the next and SAVE…;
  then POLY and VOICES where the count was; ANALOG and SPREAD where they were. The bar's
  PRESET label went: the star, the arrows and SAVE… say what it is. The window is 80 panel
  units shorter (577 pixels tall at 1720 wide, as before R10's bar); the drawer opens below
  the strip. `ca72-panel`: `presets::bar_svg` draws the selector into the strip's frame
  (one renderer instead of two); `strip::hit` leaves the selector to `presets::bar_hit`.
  Tests: the row's parts do not overlap (each label measured in the strip's face), the
  selector's controls found where drawn, the window's heights.
- **No real-time count**: the strip shows none and VOICES is no longer marked; the plug-in
  no longer measures one when it is activated (half a second saved). R11's measurement stays
  as `engine::realtime_voices`, for `poly_bench`.
- **The list scrolls a row for each row's travel**: a wheel's lines as they come (on macOS a
  slow turn gives fractions of one), a trackpad's or a Magic Mouse's points as rows of the
  list at the window's size (a row is 62 panel units), the rest kept for the next event; a
  turn the other way starts again. Each such event is a few points, and the half-notch
  threshold before dropped every one. Test: `small_scrolls_add_up_to_rows`.
- **A preset sets POLY, VOICES, ANALOG and SPREAD** like every other parameter, to its value
  or the default (R10 left POLY and VOICES alone when a preset did not name them, and the
  factory presets named none of the four, so choosing one put ANALOG and SPREAD to 0). A
  preset saved already held all four. Every factory preset now names them: Brass Tutti
  POLY on, 8 voices, ANALOG 40 %, SPREAD 50 %; Wooden Mallet POLY on, 8, 20 %, 60 %; the
  other fifteen, documented mono patches, POLY off, 4 voices, ANALOG and SPREAD 0 (as they
  were). Test: `presets::tests`.
- **The DAW** carries the factory file byte for byte (R10); its copy now differs by these
  four keys in each preset, and its instrument's choosing of a preset by the same rule:
  both to port.

## R13. How much lighter ten voices can be
**Owner's question, 2026-10-02**, trying R11's build in Live: Live's CPU meter reaches
about 40 % with FEEDBACK and ten POLY voices. The owner asked whether more speed could be
had without losing audio quality, or large gains from a small concession, concerned that it
was too heavy for many people's computers. Two changes that keep the sound are built; the
concessions were measured and left for the owner to decide (any audible change is theirs).

**Agent decisions, 2026-10-02** (not separately approved):

### Measuring
- `preset_cost` (ten voices on one thread, as fast as it goes), the least of three 5 s
  runs, the builds interleaved. Live was open with the owner's Set, so a share of a core
  here is a few percent above R11's (Live closed): compare within a row.
- The sound against a render of the code before (`preset_render`, every factory preset at
  ten voices, 4 s), as in R11.

### The same sound, a little less work
- **The pairs' tanh, fast inside a tick.** A transistor pair's tanh anchor (`PairWarm`: the
  VCA's three pairs, the filter's in High Fidelity) takes `ulp::tanh_in_tick`: inside a High
  Fidelity or Potato part's tick `fast::tanh` (within 2e-14 of libm's, relatively), as
  `ulp::exp` already was; No Compromises and the factory calibration (outside a tick) keep
  libm's. A voice in Potato takes some 35,000 of them a second (Bass). Against the code
  before: 14 presets the same to the bit, Upright Pluck and Pulse Strut differ at -263 and
  -166 dB; Undertow Growl's FEEDBACK loop, chaotic, parts from the old render as any change
  of rounding makes it do (its difference as loud as the render, the level +0.04 dB, the
  largest third-octave band's change +1.3 dB at 20 Hz, inside R11's spread from one ANALOG
  seed to the next of up to 8 dB). 1 to 2 % faster.
- **The bundles link-time optimised:** `[profile.bundle]` (release, `lto = "fat"`, one
  codegen unit), built by `cargo xtask bundle ca72-plugin --profile bundle` in CI, the
  README and `scripts/validate.sh`; the tests stay on `release`, which links far faster.
  Every preset the same to the bit as `release`. 2 to 4 % faster. Its build output takes
  about half a gigabyte more under `target/bundle`.
- Together, a share of one core at ten voices:

  | Preset | Before | Fast tanh | And link-time optimised | Change |
  |---|---:|---:|---:|---:|
  | Undertow Growl | 100.1 % | 98.1 % | 94.5 % | -5.6 % |
  | Pulse Strut | 89.8 % | 88.4 % | 84.9 % | -5.5 % |
  | Three Saw Slab | 79.2 % | 78.6 % | 75.7 % | -4.4 % |
  | Brass Tutti | 76.6 % | 75.4 % | 72.5 % | -5.4 % |
  | Wooden Mallet | 53.5 % | 52.9 % | 51.2 % | -4.3 % |

- **Measured and not built:** the workers spinning 50 us rather than 200 after a run
  before they park. Four workers spin up to 0.8 ms a block, some 15 % of a core at 256
  frames that Live's meter does not show but other tracks' threads could use; whether the
  shorter spin makes a block late wants a paced run with Live closed.
- **Nothing more to the bit** that the measurement can see (R11 tried the filter's
  divisions and tables and the oscillators' exponentials). A large gain with the same
  samples would need the voices computed side by side, two to a NEON register (four to
  AVX2's), each operation the same in every lane, the solvers' iterations masked lane by
  lane: a rewrite of the voice's data and its solvers, its gain not measured.

### Concessions, measured (none built)
Each switched on alone in Potato for the measurement (and removed): its cost against the
same build without it (Undertow Growl, Three Saw Slab, Wooden Mallet, Brass Tutti), and
its sound against the reference render (17 presets):

| Concession | Cost | Sound |
|---|---:|---|
| The filter at the host's rate, not twice it | -12 to -23 % | the top octave 2 to 8 dB down in most presets; Cruising Whistle +3.5 dB at 12.7 kHz; FEEDBACK's loop another (Pulse Strut +15 dB at 630 Hz, Undertow Growl +24 dB at 20 Hz) |
| The filter's Newton solve one iteration a step, not two | -2 to -3 % (Undertow Growl -18 %) | the ordinary presets within 0.5 dB in every band; FEEDBACK's loop another (Undertow Growl +19 dB at 20 Hz) |
| The contours at a sixteenth of the rate, not an eighth | -2 to -8 % | attacks click: Bass +10 dB at 10 kHz, Upright Pluck +4 dB at 6.3 kHz |
| The keyboard circuit at a sixteenth of the rate | about -1 % | not compared |
| All of them | -22 to -38 % | all of the above |

So no small concession gives a large gain: the circuit's cheapest voice is 1.3 to 1.6
times cheaper and audibly another. A voice's cost is the circuit itself (R11: the filter a
third, the oscillators a fifth, the VCA a sixth). Five times cheaper or more would take
another model of the voice for POLY (the filter as a ladder of one-pole stages around one
nonlinearity, the oscillators as band-limited waveforms): another instrument than the
circuit's, and a second quality in the plug-in against R1's one; the owner's to decide.

### What ten voices ask of a machine
- On the M4 Pro a voice takes 5 to 10 % of a core (Wooden Mallet the least, Undertow
  Growl the most): ten voices about half a core's work to a core's, shared by the host's
  thread and the workers (R11: a third of the processors less two, at most four: none
  below 5 processors, one at 5 to 7, two at 8 to 10, three at 11 to 13). Live's meter shows
  the host's thread's block against its period, waiting for the workers' voices included:
  about a fifth of the work at ten voices with four workers, and its worst blocks.
- A machine of 8 processors (two workers) shares the voices three ways; one of 4 (none)
  plays them all on the host's thread, where ten voices of the heavier presets would need
  a core faster than the M4 Pro's (one of its cores holds five or six of them within 60 %
  of the period; a slower core fewer). Windows and Linux are still not measured.
- VOICES defaults to 4; Brass Tutti and Wooden Mallet set 8 (R12).

**Evidence (the Mac, 2026-10-02, the bundles of this version built with `--profile
bundle`, Live open):** clap-validator 0.4.1, 37 passed, 0 failed, 7 skipped; pluginval
1.0.4 at strictness 10, SUCCESS (its editor tests skipped while Live was open); Steinberg's
validator (SDK 3.8.1), 47 passed, 0 failed. The workspace's tests (157 passed, 0 failed, 20
by hand) and clippy pass. The bundles, with R12's changes, are installed for Live.

### For the DAW's instrument
- The pairs' fast tanh inside a tick (`ulp::tanh_in_tick`, `PairWarm`), if its copy of the
  model has the anchors.

## R14. ANALOG is ENTROPY
**Owner decisions, 2026-10-03:** ANALOG is renamed ENTROPY; its parameter id may change
too, as long as it is the same everywhere; and no compatibility with what was saved before
(below).

**Agent decisions, 2026-10-03** (not separately approved):
- **ENTROPY everywhere:** the strip's label, the parameter's name in a host ("Entropy") and
  its id (`entropy`), the presets' key (`entropy`, the factory file's seventeen included),
  the README, and the code's names (the engine's control and `ENTROPY_DEPTH` with its kin,
  the character's `entropy` figures, `crates/ca72/tests/entropy.rs`). What it does is
  unchanged. The strip's label fits before its slider (the row's parts measured as in
  R12).
- **No compatibility kept** (the owner, 2026-10-03, saw no need for it, since no one had
  used the plug-in outside testing): a session or a preset saved with ANALOG's `analog`
  opens with ENTROPY at its default, and a host's automation of ANALOG no longer reaches
  it.
- The records before this one keep the name ANALOG, as written then.
- **The DAW** reads the plug-in's presets by the same keys (R10): until its instrument
  takes the name too (its control, its key and its copy of the factory file), each
  program's presets hold the amount under a key the other does not know. To port, with
  R12's keys.

**Evidence (the Mac, 2026-10-03, the bundles of this version, Live open):** clap-validator
0.4.1, 37 passed, 0 failed, 7 skipped; pluginval 1.0.4 at strictness 10, SUCCESS (its editor
tests skipped while Live was open); Steinberg's validator, 47 passed, 0 failed. The
workspace's tests (157 passed, 0 failed, 20 by hand) and clippy pass. The bundles are
installed for Live.

## R15. Six factory presets: the launch film's drums and riser
**Owner decisions, 2026-10-03:** the agent made the CA-72's launch film with every sound on
the model, the drums included. The owner then asked for these sounds to be added to the
factory presets, in the DAW and in the VST3 and CLAP plug-ins, and for the film to make
clear that every sound is the CA-72.

**Agent decisions, 2026-10-03** (not separately approved):
- **Six presets after the seventeen, 23 in all:** Ladder Kick, Noise Snare, Closed Hat,
  Open Hat, Noise Crash and Pink Riser, tagged `drums` (with `kick`, `snare`, `hi-hat`,
  `cymbal`, `noise`, `percussive`) or `fx` and `riser`. Every control is named, as in the
  documented patches; POLY off, VOICES 4, ENTROPY and SPREAD 0 (R12). A preset is one
  monophonic instrument, so a kit takes one instance a drum.
- **Each works from the panel alone.** A preset holds neither the PITCH wheel nor
  automation, so the film's first kick (a triangle with the PITCH wheel swept 16 semitones
  down on each hit) and riser (CUTOFF automated over 8 s) were redesigned to do it
  themselves (measured on the model):
  - **Ladder Kick:** a triangle at 32' into a filter whose EMPHASIS (9.6) sits just under
    oscillation. A 0.2 FILTER DECAY sweeps the ring from about 540 Hz into the low end
    within 30 ms, over the triangle's 55 Hz on MIDI note 57. Longer, wider sweeps (EMPHASIS
    8 to 10, AMOUNT OF CONTOUR 6 to 8) lingered in the mids: their energy below 120 Hz was 8
    to 19 dB under the whole, against 1.7 dB here.
  - **Pink Riser:** FILTER ATTACK at 10 opens a resonant pink noise (EMPHASIS 7, CUTOFF
    -3.5, AMOUNT OF CONTOUR 10) over about 4.5 s, while the loudness swells (ATTACK 6).
  - The snare, the hats and the crash are the film's patches: noise with the filter open
    (a triangle at 7 for the snare's body, MIDI 54 about 185 Hz), told apart by DECAY.
- **The film was rendered again with these presets as they are** (no value changed, no
  automation), so its drums are these.
- **Levelled as the first seventeen:** MAIN OUTPUT VOLUME for about -18 LUFS momentary
  maximum on a phrase in each one's register. Noise Crash 9.7 (-17.9). The other five are
  under it at 10, quiet by nature: Ladder Kick -22.9 (LUFS weighs the lows lightly; a
  second triangle added 0.6 dB), Noise Snare -20.6, Closed Hat -20.7, Open Hat -20.8, Pink
  Riser -25.6.
- **The editor's test of the drawer over the panel** clicked "Lead" by name. With Closed
  Hat and Ladder Kick sorting before it, Lead left the list's first page and the test failed.
  It now clicks the first page's second row, whichever preset that is.
- **The DAW** carries the same six in its copy of the file (on a branch of its own),
  without R12's four keys, which it has not ported. Its golden renders record each.

**Evidence (the Mac, 2026-10-03, the bundles of this version, Live open):** clap-validator
0.4.1, 37 passed, 0 failed, 7 skipped; pluginval 1.0.4 at strictness 10, SUCCESS;
Steinberg's validator, 47 passed, 0 failed. The workspace's tests (157 passed, 0 failed, 20
by hand), clippy and rustfmt pass. Both installed bundles carry the 23 presets. They are
installed for Live, the ones they replaced kept aside.

## R16. Warped Pad, the IDM film's pad, as a factory preset
**Owner decision, 2026-10-03:** after the launch film's IDM version, the owner asked for
its pad to become a preset if it was not one already.

**Agent decisions, 2026-10-03** (not separately approved):
- **Warped Pad, the 24th preset:** two sawtooths a hair apart (oscillator 2 at +0.1), the
  filter opened slowly (CUTOFF -1.5, EMPHASIS 2, AMOUNT OF CONTOUR 2, ATTACK 5) and a slow
  vibrato from oscillator 3 at LO (MOD wheel 0.03). POLY is on with eight voices, ENTROPY 85
  and SPREAD 80; the drift is the point of it. The film automated CUTOFF and moved ENTROPY
  between 60 and 100; the preset holds the values of the film's last section.
- **Levelled as the others:** MAIN OUTPUT VOLUME 7.8, -18.0 LUFS momentary maximum on three
  held six-note chords.
- The DAW's copy names POLY, VOICES, ENTROPY and SPREAD for this preset, since the pad's
  sound depends on them. It is the first preset in that copy to name all four.

**Evidence (the Mac, 2026-10-03, the bundles of this version, Live open):** clap-validator
0.4.1, 37 passed, 0 failed, 7 skipped; pluginval 1.0.4 at strictness 10, SUCCESS;
Steinberg's validator, 47 passed, 0 failed. The workspace's tests (157 passed, 0 failed, 20
by hand) pass. Both installed bundles carry the 24 presets.

## R17. Installers, and a README that starts with them
**Owner's request, 2026-10-03**, preparing the open-source launch: installation and setup
instructions that are clear and concise, as easy as possible for people, perhaps with an
installer.

**Agent decisions, 2026-10-03** (not separately approved):
- **An installer for each platform**, made from the bundles by `scripts/package.sh` on the
  platform it runs on, and by CI on every run (artifacts `CA-72-<platform>-installer`):
  - **macOS:** `CA-72-<version>-macOS.pkg` (pkgbuild, productbuild), universal, macOS 11 or
    later. It installs `CA-72.vst3` and `CA-72.clap` for every user into
    `/Library/Audio/Plug-Ins/VST3` and `.../CLAP`, each format a choice; it asks for an
    administrator's password, as plug-in installers do. The bundles are not relocatable
    (Installer would otherwise put them wherever it finds a bundle of the same identifier,
    a build folder say) and not version-checked (any version installs over any other).
  - **Windows:** `CA-72-<version>-Windows-setup.exe` (Inno Setup 6,
    `scripts/installer/CA-72.iss`), 64-bit, Windows 10 or later. It installs into
    `Common Files\VST3` and `Common Files\CLAP`, each format a component, with an
    uninstaller in Settings' Apps and the licence and notices in `Program Files\Idle
    Foundry\CA-72`. Its AppId must never change.
  - **Linux:** `CA-72-<version>-Linux-x86_64.tar.gz` with `install.sh`: for the user into
    `~/.vst3` and `~/.clap`, `--system` into `/usr/lib/vst3` and `/usr/lib/clap`,
    `--uninstall`. It warns when X11's client libraries are missing.
- **Unsigned for now**, as R4 waits for the owner's Developer ID. The README says how to
  open them past Gatekeeper and SmartScreen once. Files an installer writes carry no
  quarantine flag, so the README's `xattr` step is gone. `package.sh` signs and notarises
  when given identities and a notarytool profile (`CA72_MACOS_SIGN_APP`,
  `CA72_MACOS_SIGN_INSTALLER`, `CA72_NOTARY_PROFILE`); CI does not yet, which needs the
  certificates as secrets.
- **The bundles' Info.plist on macOS:** nih-plug's bundler writes the identifier
  `com.nih-plug.ca72-plugin` and version 1.0.0. The installer's copies carry
  `com.idlefoundry.ca-72.vst3` and `.clap`, the crate's version and macOS 11 as their
  minimum, and are signed again (ad hoc).
- **macOS 11 is the minimum.** The audio workgroups (R11) are macOS 11's and are linked as
  required, while the toolchain's default for the Intel slice is 10.12: it would have
  failed to load on 10.12 to 10.15. `.cargo/config.toml` sets `MACOSX_DEPLOYMENT_TARGET`
  to 11.0 (checked: both slices say 11.0).
- **Linux's bundles are built on Ubuntu 22.04** in CI (24.04 before), so that they load
  where the C library is glibc 2.35 or later.
- **Third-party notices.** The plug-ins' binaries include some 190 crates (MIT,
  Apache-2.0, BSD, ISC, Zlib and others) whose licences ask for their notices to go with
  every copy. `scripts/notices.py` writes `THIRD-PARTY-NOTICES.txt` from Cargo.lock for
  every platform released: each crate's own licence files, MIT's where a crate offers it
  among others, the standard MIT text with its authors where a crate ships none, and the
  font's licence. CI checks that it is current. Every installer carries it and LICENSE
  beside the plug-ins and inside each bundle that has room.
- **A release:** a tag `v<version>` runs CI and drafts a GitHub release of the three
  installers, their SHA-256 sums and the notices, for the owner to publish.
- **The README** opens with downloading and running the installer, the warnings while
  unsigned, the folders, and uninstalling. Building from source lists each platform's
  prerequisites (rustup; Xcode's command line tools, Visual Studio's Build Tools, or
  Linux's packages for Debian and Ubuntu, Fedora and Arch), cloning, how long the build
  and tests take, and `validate.sh`'s own needs.

**Evidence (2026-10-03).** The README's Building and Testing followed on the three
reference machines, from a fresh clone from GitHub with an empty cargo cache:

| | Mac (M4 Pro, macOS 27) | Linux (Ryzen 7 7800X3D) | Windows 11 (i5-13600K) |
|---|---|---|---|
| Bundles (`--profile bundle`) | 93 s | 35 s | 69 s |
| `cargo test --workspace` | 157 passed, 0 failed (164 s) | 158 passed, 0 failed (470 s) | 157 passed, 0 failed (218 s) |
| Standalone built | yes | yes | yes |
| clap-validator 0.4.1 | 37 passed, 7 skipped | 37 passed, 7 skipped | 36 passed, 8 skipped (one runs on Unix only) |
| pluginval 1.0.4, strictness 10 | SUCCESS, editor tests skipped (Live open) | SUCCESS, editor tests skipped (no display) | SUCCESS, editor tests included |
| Steinberg's validator, SDK 3.8.1 | 47 passed | 47 passed | not built: the SDK needs Visual Studio, and this machine builds with MinGW |
| Installed copies validated | yes | yes | CLAP yes |

On Windows, pluginval's editor test crashed when run over SSH (no desktop) and passed in
the desktop session. The installers, made by `package.sh` from this branch:
- **macOS:** the `.pkg`'s payload is the staged bundles to the byte, each bundle's
  signature valid within it, both slices macOS 11.0; its bundles pass clap-validator (37
  passed), pluginval at strictness 10 (SUCCESS, editor tests skipped while Live was open)
  and Steinberg's validator (47 passed). Not installed on the Mac (that needs an
  administrator, and the owner's own build in `~/Library` was in use by Live), and the
  Intel slice not run (no Rosetta).
- **Linux:** unpacked and `./install.sh` run, the installed copies pass clap-validator and
  Steinberg's validator; installing again, `--uninstall` and `--help` behave.
- **Windows** (Inno Setup 6.7.3 installed for it, with the owner's approval): the setup
  program installed silently (exit 0) into `Common Files\VST3` and `\CLAP` with the licence,
  the notices and an uninstaller in `Program Files\Idle Foundry\CA-72`, and an entry
  "CA-72 0.1.0, Idle Foundry" in Settings' Apps; the installed copies pass clap-validator
  and pluginval at strictness 10; it installs again over itself (VST3 only); its
  uninstaller removes everything it wrote, the folders it made included.

## R18. Before the release: a review's findings fixed, the name plate fixed, a new home
**Owner decisions, 2026-10-03.** Preparing the open-source launch, the owner asked for a
review of the code and an install test on the three reference machines, and for every
issue the review found to be fixed; and for R1's name plate to stop turning over. The
CA-72's public home is the Idle Foundry organisation, `github.com/idlefoundry/ca-72`. R17's
installers were approved, and the 24 presets (R15, R16), the installers and these fixes go
to `main` together.

**Agent decisions, 2026-10-03** (not separately approved), by part:

### Three factory presets, as the owner changed them
**Owner decision, 2026-10-03:** the owner saved three factory presets anew from the panel
and asked for the changes to be made permanent. In `sounds/presets.toml`, to the values the
controls show:
- **Slow Horn Swell:** POLY on, VOICES 10, ENTROPY 25 % (saved as 24.96).
- **Undertow Growl:** FEEDBACK 3.8 (saved as 3.8022) and MAIN OUTPUT VOLUME 9.17 (9.167),
  so no longer levelled as R15's presets were.
- **Wooden Mallet:** POLY off (R12 had it on with eight voices; VOICES 8, ENTROPY 20 and
  SPREAD 60 stay for when POLY is switched on).
- **Upright Pluck** (asked later the same day): the three oscillators' VOLUME at 10 (8
  before). Only oscillator 1 sounds in it, now louder into the mixer.

### The name plate
- It no longer turns over (R1's "Model DEEZ" is gone, with its click and its animation). It
  always shows CA-72 and its maker, at the width it had (261 units), so the panel is the
  same to the pixel.

### The voices on the audio thread
- **Exports and freezes wait for every voice.** The deadline (R11) applies only in real time:
  the processing mode is read each block (nih-plug's `ProcessContext::process_mode`, below),
  since CLAP may switch to offline without initialising the plug-in again. A voice that
  misses a run keeps its level for the silence test, so it is not freed and its tail is not
  cut.
- **After the deadline the block's later runs play on the host's thread**, rather than being
  handed to workers and lost: the block is late rather than voices silent.
- **A sample that is not finite** on the side chain is read as 0. A voice whose output goes
  non-finite is silent until a clean voice takes its place: a spare for each voice, made off
  the audio thread when the plug-in is prepared and made again on nih-plug's background
  thread after use. The output is never NaN. (Ten spare voices an instance: memory, no
  work.)
- **All Sound Off (CC 120) silences at once:** a 5 ms fade, every sounding voice replaced by a
  clean one, then POWER's 10 ms fade in. All notes off (CC 123) still releases the notes.
- **A state loaded into a running instance** (CLAP and VST3 initialise the plug-in again
  for it) keeps its workers when their count and period are unchanged, so a host's preset
  change no longer drops out; a new seed draws ENTROPY's characters again, so a session
  plays as it did when saved. A session saved before R14 opens with ENTROPY at its default.
- A retrigger due for a voice that did not play a run is kept for the next.

### The plug-in's threads
- **Every instance in a process shares one budget of workers** (R11's count, at most four):
  an instance that finds none plays its voices on the host's thread. An instance with POLY
  off still takes its share when activated (starting workers when POLY turns on would mean
  starting them off the audio thread on demand; not done).
- **A late worker cannot touch a later run:** a worker that finds its run closed leaves the
  voice alone (that voice does not advance for the run), and each voice's mark is its own.
- **Workers take the host thread's flush-to-zero bits** for each run, so a voice's samples
  are the same on any thread. DAZ is not forced where the host has not set it, for the same
  reason.
- **Windows:** workers join MMCSS's "Pro Audio" task at critical priority (time-critical
  priority if that fails), and the host's thread spins rather than sleeping while it waits.
- **Linux:** the watchdog (R11) demotes only the threads it promoted, never the host's or
  other plug-ins'; its messages begin `ca72:`, and the real-time limit's message prints at
  most once. Its threads park while no promoted thread lives.
- Nits: no Mach port leaked a worker; a worker that fails to start stops the others; the
  lab's threads are `ca72-*`.

### The presets' library (R10)
- **Names:** `factory` (the overlay's file) and Windows' device names (CON, NUL, COM1 and
  so on) are refused on every system, in any case; names are one preset regardless of case
  on every file system.
- **Writes go to the preset's own file**, which keeps its name unless no other file has the
  new one; a name read from a file is checked as one typed (`../x` or an absolute path is
  reported, not listed).
- **An overlay that cannot be read is left as it is,** and marking a factory preset reports
  why; the overlay is edited in place, keeping keys this version does not know.
- **An edit stays with its preset** (by name and origin) while the list filters, refreshes or
  loses it; a preset gone meanwhile is reported.
- **The library is read again only when its files change**, and search and tags work on what
  was read, off the disk.
- **MIDI BEND RANGE is the player's,** not the sound's: choosing a preset that does not name
  it leaves it as it is, and the plug-in's own saves no longer write it (a file that names
  it still sets it).
- **The factory file is `sounds/presets.toml`,** its comments without trademarks or the
  DAW's name (R1: the file is inside every bundle); Ladder Kick is "a booming electronic
  kick". R10's "byte for byte" with the DAW's copy no longer holds for the comments.

### The editor (R5, R6, R10)
- **Linux:** the drawer no longer takes the keyboard by asking for focus (baseview's X11
  window does not implement it and panicked, closing the editor). X11 gives the editor the
  keys while the pointer is over it.
- **Windows:** when the drawer shuts it gives the keyboard back to the window that had it.
- **Scale:** on Windows and Linux the editor uses the host's scale, or exactly 1 when the
  host gives none (Live), so the drawing, the window and the pointer agree (at 150 % the
  pointer had landed at two thirds of its place).
- **Linux's first size** is the monitor the pointer is on, else the primary (RandR, through
  x11rb), less the desktop's panels; not the whole X screen.
- **The drawer opens over the panel** when the window with it below would not fit on the
  screen, or when the host refuses to resize (VST3 now reports a refusal on macOS and
  Windows; below).
- **Gestures never nest:** a wheel's gesture, a drag or a slider is ended before any other
  begins, when the window loses focus, and when the editor closes; on Windows a drag whose
  mouse capture was lost ends, and the capture is released.
- The editor reopens at its own size (not the drawer's); the grip asks the host for a size
  at most once a frame; names with characters XML forbids no longer blank the drawer;
  without a parent window or a display no window opens, and softbuffer's failures stop the
  drawing rather than the host. The approved-image test prints its margin (mean 1.245,
  99th percentile 26, against 1.5 and 32, the same on Apple silicon and x86-64).
- `docs/panel.png` is drawn again, with the strip under the panel.

### nih-plug (`third_party/nih-plug/PATCHES.md`, changes 4 to 8)
- A VST3 host's refusal to resize reaches the editor; auxiliary buses are bounded by the
  host's own counts (two read one past the end of the host's array); a side chain's missing
  channels are as long as the block; VST3's buffer configuration carries the processing mode
  just set; and `ProcessContext::process_mode()`.

### Building, testing, the repository
- `scripts/validate.sh` fetches the validators with curl (no GitHub CLI), checked against
  their SHA-256, and `CA72_SKIP_GUI_TESTS=1` leaves out pluginval's editor tests.
- `scripts/fetch-sources.sh` finds its manifest again and fails when a source cannot be
  fetched or does not match.
- The circuit tests ask for ngspice 47, the reference measurements' version, and skip (or,
  with `CA72_REQUIRE_NGSPICE`, fail) with another.
- Timing tests: the two measurements of what a machine plays are run by hand; the engine's
  deadline tests no longer depend on a worker being scheduled within milliseconds.
- CI builds the standalone. LICENSE is the GPL's canonical text. CONTRIBUTING.md and
  SECURITY.md. The repository's URL, the plug-in's (shown by hosts) and the installers'
  are `github.com/idlefoundry/ca-72`.

**Evidence (2026-10-03).**
- **The three reference machines,** from this release's code before it was published:
  the Mac (Apple M4 Pro, macOS 27), 201 tests passed, 0 failed (22 run by hand), clippy,
  rustfmt and the notices' check, the universal bundles and the `.pkg`, clap-validator 37
  passed (7 skipped), pluginval at strictness 10 SUCCESS (editor tests skipped while Live
  was open), Steinberg's validator 47 passed; the Linux reference machine (Ryzen 7 7800X3D),
  202 passed, 0 failed, the tarball, the same validators (editor tests skipped: no
  display); the Windows machine (i5-13600K, Windows 11, the GNU toolchain), 200 passed, 0
  failed, the setup program and the standalone.
- **CI** (macOS 15, Windows Server 2025, Ubuntu 22.04): every job green, with ngspice 47
  required on Linux and the three validators on each system.
- **The installers CI made, each installed on a machine without the CA-72:**
  - **Linux:** `install.sh` into `~/.vst3` and `~/.clap`; the plug-in needs glibc 2.35 at
    most (Ubuntu 22.04's); the installed copies pass clap-validator (37 passed), pluginval
    at strictness 10 (SUCCESS, editor tests skipped) and Steinberg's validator (47 passed);
    `--uninstall` leaves nothing. The owner then played it in Bitwig Studio, CLAP and VST3.
  - **Windows:** the downloaded setup program, unsigned, met SmartScreen's warning, as the
    README says; allowed, it installed (exit 0), "CA-72 0.1.0, Idle Foundry" in Settings'
    Apps; the installed copies pass clap-validator (36 passed, 8 skipped: one test runs on
    Unix only) and pluginval at strictness 10 with its editor tests (SUCCESS, in the
    desktop session); its uninstaller leaves nothing, the folders it made included.
  - **macOS:** Gatekeeper would not open the downloaded package until Open Anyway in
    Privacy & Security, as the README says; the owner installed it. In
    `/Library/Audio/Plug-Ins`: universal, 0.1.0, macOS 11, `com.idlefoundry.ca-72.vst3` and
    `.clap`, signatures valid, no quarantine flag, the licence and notices inside; they pass
    clap-validator (37 passed), pluginval at strictness 10 with its editor tests (SUCCESS)
    and Steinberg's validator (47 passed).

## R19. Two editor fixes after the installers were tried
**The owner's reports, 2026-10-03,** trying the installed plug-in: in Bitwig Studio on the
Linux reference machine, CLAP, the editor opened black until the pointer moved over it; on
macOS in Live, the presets' drawer, opened below the strip, did not shut the way it opened:
it jumped up over the panel and slid down off it, and when that was fixed, it flickered at
the moment it had shut.

**Agent decisions, 2026-10-03** (the owner confirmed the first three fixes in the hosts they were found in):
- **Linux: the frame shown again every quarter second.** The editor drew only when its frame
  changed (R5), the first frame once. X11 drops what is drawn to a window not yet on screen,
  and Bitwig maps the editor's window after that first frame; without a compositor it also
  drops what a covering window hid; baseview reports neither. On Linux the last frame is now
  shown again, unchanged, every 250 ms (a copy, no rendering). macOS and Windows keep a
  window's pixels and are unchanged.
- **The drawer shuts as it opened.** Shutting below the strip had shrunk the window at once,
  so the drawer's slide (still running) was drawn as the over-the-panel one (R10, R18). Now
  the window keeps its height while the drawer slides back up under the strip (140 ms), and
  shrinks when it has; the over-the-panel drawer is unchanged.
- **The window shrinks in one frame.** The host was asked for the shorter window while a
  frame was drawn, and the editor's view followed only in the next: on macOS, for that
  frame, the tall view sat in the short window with its top cut off. The view now follows in
  the same frame, before anything is shown.
- **The width is measured once an opening.** Until the grip sets a width, the editor's was
  fitted to the screen afresh at each of the host's questions, and the drawer asks again as
  it opens and shuts; on Linux the screen is the monitor under the pointer (R18), so the
  width, and the panel's scale, could change with the drawer. It is now fitted as the editor
  opens and kept until it closes.
- **A worker lets a voice go before marking it played.** R18's fencing of late workers had
  the mark stored while the voice's lock was still held: the caller's thread could see the
  mark, close the run, take the voice in the next and fail its `try_lock`, the voice then
  silent for that run. CI on macOS caught it as a FEEDBACK preset's samples differing between
  one thread and four (`tests/workers.rs`); the lock is now released first.

**Evidence (2026-10-03):** the plug-in's library tests (65 passed, among them
`the_drawer_shuts_the_way_it_opened`, `a_frame_is_shown_again_on_linux_only` and
`the_width_stays_while_the_drawer_opens_and_shuts`) and clippy on
macOS and Linux, clippy for Windows; the owner, in Bitwig on Linux (CLAP) and in Live on
macOS, with the builds of these changes.

## R20. Developer ID signing and notarization
**Owner's request, 2026-10-03:** set up Apple Developer ID for CA-72 so people installing
it on a Mac are recognized as receiving software from a trusted developer.

- Developer ID Application and Developer ID Installer certificates were issued for
  Idle Foundry Ltd., team `3JA8JUZ36W`, with the owner's approval. Both identities and
  private keys are installed in this Mac's login Keychain; encrypted backups and the
  local release configuration are outside the repository. Notarization credentials
  were entered by the owner and stored as the `CA72_NOTARY` Keychain profile.
- A universal 0.1.0 installer from main was signed, accepted by Apple's notary service,
  stapled and validated. Both payload bundles pass strict signature verification and
  contain x86_64 and arm64; their signatures include hardened runtime and a trusted
  timestamp. Gatekeeper accepts the installer as `Notarized Developer ID`, including a
  copy carrying the download quarantine attribute. It also opens normally in Installer.
- `CA72_REQUIRE_NOTARIZATION=1` makes public Mac packaging require both Developer ID
  identities and a notarization profile. Partial signing configuration is rejected.
  Apple must return `Accepted`; the ticket, installer signature and Gatekeeper policy
  are checked before the final distributable is replaced. A missing-profile failure
  was verified to preserve the previous installer.
- `docs/macos-release.md` records the reproducible local release procedure. Hosted CI
  signing secrets are not configured by this change; its unsigned macOS artifacts must
  not be published as the trusted installer. No signing private key or password is
  committed to the repository.

## R21. POLY's workers only while POLY is on; the copyright holder
**Owner decisions, 2026-10-03.** Of R18's note that an instance with POLY off still took its
share of the workers: it "should be fixed, absolutely". The copyright holder is Idle Foundry
Ltd., the company that holds the Developer ID (R20); the brand in hosts stays Idle Foundry.

**Agent decisions, 2026-10-03** (not separately approved):
- **An instance holds workers only while its POLY is on.** At activation it starts them only
  if POLY is on. When POLY is switched on, the audio thread asks for them and plays every
  voice itself until they come; nih-plug's background thread (as for the spare voices, R18)
  starts the pool and hands it over through a slot the audio thread only tries to lock.
  When POLY is switched off, or the plug-in is deactivated, the pool is given back and
  stopped on the background thread, its workers returned to the budget. Nothing is
  allocated, waited for or started on the audio thread; the sound is the same either way
  (R11). A state load at the same rate and period keeps running workers (R19).
- Still first come, first served among instances with POLY on: one that found none plays on
  the host's thread and asks again only when its POLY is next switched on (no polling).
- **The copyright line** "Copyright © 2026 Idle Foundry Ltd." is in the README, the notices,
  the macOS bundles' Info.plist and the Windows installer (whose publisher is now Idle
  Foundry Ltd.).

**Evidence (2026-10-03, the Mac):** the plug-in's tests (`ca72-plugin`: the library's 72, among
them `an_instance_with_poly_off_leaves_its_workers_to_one_with_it_on` and
`the_workers_follow_poly_through_the_plug_ins_life`; the real-time test with POLY switched,
nothing allocated on the audio thread), `ca72`'s `threaded` and `rt_alloc`, rustfmt and
clippy (and clippy for Windows).

## R22. URW Gothic for the lettering
**Owner decision, 2026-10-03:** shown the panel lettered in TeX Gyre Adventor, URW Gothic and
Nimbus Sans side by side, the owner chose URW Gothic.

**Agent decisions, 2026-10-03** (not separately approved):
- **Why change.** TeX Gyre Adventor's GUST Font License is the LaTeX Project Public
  License 1.3c, which the FSF counts as incompatible with the GNU GPL (some modified
  versions must carry, or point to, the unmodified original), and the plug-in builds its
  font into its binaries. URW Gothic, the font TeX Gyre Adventor was made from, is under the
  GNU AGPL 3.0 with a font exception; the GPL 3.0 (section 13) lets a work be combined with
  one under the AGPL 3.0, so the binaries are one work under compatible licences.
- **The same letters.** The drawn panel differs in 0.3 % of its pixels, at the letters'
  edges; the approved-image test gives a mean difference of 1.304 and a 99th percentile of
  28 (TeX Gyre Adventor's: 1.245 and 26; the limits 1.5 and 32). Book is the regular, Demi
  the bold.
- `third_party/urw-gothic/`: the two fonts unmodified, from
  `ArtifexSoftware/urw-base35-fonts` at tag `20200910`, with its licence, exception and
  readme, the hashes in `VENDORED.md`; the notices carry the licence and the exception.
  R2's "TeX Gyre Adventor" stands as the record of that time; `docs/panel.png` is drawn
  again.

## R23. The plug-in's own helper thread, so that a host may unload it at once
**Found 2026-10-03,** in the first CI run of the repository as published: on Windows,
clap-validator's `param-fuzz-modulation` test crashed (`0xc0000005`). On the Windows
reference machine it crashed in 167 of 200 runs with the code before R21 (workers started at
activation), and about once in 300 with R21.

**The cause (gdb).** The validator's main thread was unloading the plug-in's library while
nih-plug's shared background thread still ran a task of ours inside it (rebuilding the spare
voices, R18; starting or stopping POLY's workers, R21), the voice workers alive beside it.
nih-plug's background thread keeps the plug-in alive while it runs a task for it, so a host's
destroy during one returned at once and left the plug-in, its pools and their workers
running; Windows unmaps a library as soon as it is unloaded. macOS and Linux keep a library
mapped while threads of it live (on Linux, glibc's record of their thread-local
destructors), so only Windows crashed.

**Agent decisions, 2026-10-03** (not separately approved):
- **The plug-in gives nih-plug's background thread no work.** A helper thread of its own,
  `ca72-helper`, started at the first activation, rebuilds the spares and starts and stops
  POLY's workers; the audio thread asks it with an atomic and an unpark, allocating nothing
  and waiting for nothing. Dropping the plug-in stops and joins the helper, then the pools
  and their workers: when the host's destroy returns, no thread of the plug-in runs. A
  rebuild stops before its next voice when told to stop, so a destroy waits at most for one
  voice to be built.
- Linux's watchdog threads (R11, R18) still live for the process once started. glibc keeps
  the library loaded while they live, so a host's unload there is deferred, not a crash; left
  as it is.

**Evidence (2026-10-03):** on the Windows reference machine (the GNU toolchain), with the
helper: `param-fuzz-modulation` 0 crashes in 300 runs, and 0 in 100 runs of a build whose
helper was made to stall 1.5 s a round regardless of a stop (the worst case for a destroy;
those runs took about 1.5 s longer each, the destroy waiting); the full clap-validator, 36
passed, 0 failed. On the Linux reference machine: 0 failures in 100 runs, the full
clap-validator 37 passed, clippy, and the plug-in's tests on 16 processors and on 3 (as CI's).
On the Mac: the plug-in's tests (among them
`a_plug_in_dropped_while_its_helper_works_leaves_no_thread_running`), rustfmt and clippy, and
clippy for Windows. GitHub's CI could not run: the organisation's Actions minutes for private
repositories were spent.

## R24. 0.1.0's Windows and Linux installers built on the reference machines
**2026-10-03.** The tag's CI could not run (R23: the organisation's Actions minutes for
private repositories were spent), so the draft release had only the Mac's installer. The
owner chose to build the other two on the reference machines from the tagged commit rather
than make the repository public early for CI's free minutes.

**Agent decisions, 2026-10-03** (the route approved by the owner, the means not separately):
- **Windows** with the GNU toolchain (Rust 1.97.1, `x86_64-pc-windows-gnu`) and Inno Setup
  6.7.3: `cargo xtask bundle ca72-plugin --profile bundle`, then `scripts/package.sh`. The
  plug-in imports only Windows's own libraries (the universal C runtime among them, as on any
  Windows 10 or 11), none of MinGW's.
- **Linux** linked by Zig 0.17.0 through cargo-zigbuild 0.23.4 against glibc 2.35, the floor
  CI's Ubuntu 22.04 gives (the machine's own glibc, 2.43, would have raised it): the wrapper
  `cargo zigbuild --target x86_64-unknown-linux-gnu.2.35` makes is given to the bundler as
  `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER`, with `RUSTFLAGS="-L native=<dir>"` where
  `<dir>` holds links to the system's libX11, libX11-xcb and libxcb only, then
  `cargo xtask bundle ca72-plugin --profile bundle --target x86_64-unknown-linux-gnu` and
  `scripts/package.sh`. The plug-in's newest glibc symbol is `GLIBC_2.35`; it needs libX11,
  libX11-xcb, libxcb, libm, libc and nothing else.
- CI stays the way releases are built; this was for 0.1.0 only.
- **The release job's checksums** now take CI's line for a file over the draft's earlier one,
  so a re-run that replaces an installer replaces its checksum too (before, `sort -u` kept
  the earlier line).

**Evidence.** Each installer was validated and installed on its system before it went into
the release, built from the release's tag: on Windows clap-validator and pluginval at
strictness 10 (editor tests skipped: no desktop over SSH; Steinberg's validator needs Visual
Studio), a silent install (the plug-ins in Common Files, the Apps entry "CA-72 0.1.0, Idle
Foundry Ltd."), clap-validator on the installed CLAP and a silent uninstall leaving nothing;
on Linux clap-validator, pluginval at strictness 10 (editor tests skipped) and Steinberg's
validator, and `install.sh` into an empty home. Their checksums are the release's
`SHA256SUMS.txt`.

## R25. The review before going public
**2026-10-04.** A last review of the repository and the release before it was made public
(the owner: "if not, give it one last review"), by two agents, one on the documents and one
on licensing and provenance, checked against the code. Nothing blocked; these were fixed.

**Agent decisions, 2026-10-04** (not separately approved):
- **The Rust standard library's notice.** std, core, alloc and what the standard library
  bundles are linked into every Rust binary, under MIT or Apache-2.0; the notices now carry
  the Rust project's MIT notice, for the Rust that `rust-toolchain.toml` names.
- **The sources of the crates taken from git.** Cargo.lock takes seven sources from git
  repositories of others (vst3-sys, GPL-3.0, among them, from a branch), which could vanish;
  the rest are on crates.io, which keeps every version. `scripts/git-sources.sh` packs the
  seven at their pinned commits into `CA-72-<version>-git-sources.tar.gz`, which the release
  job puts into each release (0.1.0's was made on the Mac the same way); the notices say so
  and give each git crate's repository and commit.
- **Credits:** the fonts' own copyright line and the licence they are built in under (the
  AGPL 3.0, R22) in the notices; `circuits/models/ua741.lib` credits TI's PSpice uA741
  macromodel, whose layout it follows (its values are computed from the data sheet); the
  trademark notice names Moog as well.
- **Documents:** the README's install steps for Linux, uninstalling on each system, building
  universal bundles (the two targets to add), the voice POLY takes ("the oldest of those let
  go", as the code does, not the one released longest ago); a sentence about the DAW the
  model came from removed; CONTRIBUTING's commands as CI runs them, and the preset
  comparison's procedure; SECURITY's link; `docs/macos-release.md` written as a maintainer's
  procedure rather than one machine's notes; two crates' descriptions; the ngspice version
  Homebrew must have. R17's "some 190 crates" was too many: the notices list 159.
- **0.1.0 rebuilt.** The notices are inside every installer, so 0.1.0's three installers were
  built again from the corrected tree, and the tag v0.1.0, not yet public, moved to it.

## R26. Windows: the host no longer aborts as the presets' drawer opens
**The report, 2026-10-05.** A developer tried the VST3 on Windows 11 in Sonar and in a DAW of
their own: it loaded, but opening the presets' drawer ended the host. Their recording, in Sonar:
the window grows for the drawer, the new part stays white, and some 7 s later Sonar is gone,
with no dialog.

**The cause,** found on the Windows reference machine with 0.1.0's released VST3 (its
installer checked against `SHA256SUMS.txt`) in a test host written for it, which answers
`IPlugFrame::resizeView` in several ways. baseview borrows the editor's window handler (a
`RefCell`) for each event; a `WM_SIZE` that reached the window while the handler was still
handling the press borrowed it again: a panic inside the window procedure, which cannot
unwind, so the process aborted (exception 0xC0000409, fail fast; Windows Error Reporting
names `CA-72.vst3`). Two ways led there:
- **The drawer taking the keyboard.** The press asked the host to grow the window and baseview
  to follow (a deferred task); `take_keys` then called baseview's `focus()`, `SetFocus`, whose
  focus messages reached the window from within the handler, and baseview ran its deferred
  tasks for them: the resize, and so the `WM_SIZE`. Only where the editor's window did not
  already have the keyboard: REAPER 7.82 gives a plug-in's window the keyboard as it is
  clicked, and its drawer opened and shut normally (with real clicks, three launches).
- **A host resizing the editor's window from within `resizeView`.** Then the `WM_SIZE` came
  from the host, whatever had the keyboard. The grip, which asks from the editor's frame, did
  the same in such a host.

The test host aborted 3 times in 3 for each host behaviour that agrees to the resize (five
behaviours, with and without the processing active, posted and real clicks), and never where
the host refused it (the drawer then opens over the panel). Which way Sonar takes was not
seen (it needs an account the owner does not have); the symptoms and the second DAW fit
either. The same panic was fixed upstream for `focus()` alone
([baseview#252](https://github.com/RustAudio/baseview/pull/252), 2026-05-25), after the
revision the editor took.

**Agent decisions, 2026-10-05** (the owner approved making the fix and asked that both ways
work; not separately approved otherwise):
- **baseview vendored and patched** in `third_party/baseview`, at the commit the editor took
  from git, as nih-plug is (`third_party/baseview/PATCHES.md`): an event that arrives while
  the handler is busy waits and is given to it once its call returns, the deferred tasks run
  only then, and `focus()` is deferred (#252). Not upstream's master: its Windows backend and
  handler interface were rewritten, a port of the editor. Not #252 alone: it leaves the second
  way (the tests below abort with it alone). A review of the patch (a second agent) found that
  closing the window freed its state while baseview's window procedure still read it (as
  upstream did); each call of it now keeps the state until it returns.
- **Tests in a real window** (Windows only, `editor.rs`, `windows_window_tests`): the editor's
  window in a window of the test's, off the screen, in a host that grows its own window, or
  resizes the editor's as well from within the request: the drawer opens below the strip and
  shuts, and the grip resizes. They aborted the test process before the patch. The test's
  window has no taskbar button and is not brought to the front, and the tests count in pixels
  whatever the screen's scale (each thread per-monitor aware). The editor's window now takes its
  presets' library from its caller, so that these tests read an empty folder of theirs rather
  than the user's.
- **The git sources.** baseview at 9a0b42c is no longer taken from git, so
  `CA-72-<version>-git-sources.tar.gz` leaves it out (it is in this repository);
  nih-plug's standalone still takes baseview at 579130e from git. The notices name its
  repository.

**Evidence (2026-10-05, the Windows reference machine, the GNU toolchain as R24, a screen
at 200 %):** the bundle as it ships (`--profile bundle`) in the test host: the drawer opened
under nine conditions, and opened, took the keyboard, shut and reopened under seven host
behaviours; the grip resized under three (0.1.0 aborted under two of them); the editor
opened, closed and opened again; no abort, panic or error report in any. Before the review's
changes the patch had passed the same, the drawer runs 3 times in 3 each. In REAPER 7.82 (the
bundle loaded from its own folder) the drawer opened below the strip with real clicks, three
launches. `windows_window_tests` 3 passed; each aborted with baseview's Windows files as
upstream had them, and the two where the host resizes the editor's window aborted with #252's
change alone. `cargo test --workspace`: 217 passed, 0 failed (22 ignored, run by hand).
Clippy with `-D warnings` on the workspace, all targets, with
`--features ca72-plugin/standalone` (`assert_process_allocs` does not build for the GNU
target; CI's `--all-features` on MSVC not run here); `cargo fmt --all -- --check`;
`scripts/notices.py --check`. Not tried: Sonar, the reporter's DAW, the CLAP in a host (the
same editor), macOS and Linux (whose code is unchanged: `src/macos/window.rs` only as rustfmt
orders it).

## R27. An update check, when asked
**Owner decisions, 2026-10-06.** The owner asked whether the plug-in could look for new
releases on GitHub, or let its user look. Of three ways offered (a link to the releases'
page; a check made when the user clicks; a check made by itself, at most daily, that could
be turned off), the owner chose the second, with a button that downloads the installer for
the user's system. Installing it from within the plug-in was weighed and not made: the
plug-in would be replacing its own files while the host has them loaded (locked, on
Windows), both installers ask for an administrator anyway, and a plug-in that ran what it
downloaded would let whoever controlled a release run code on every user's machine (the
Windows installer is unsigned, and the release's checksums come from the same release).

**Agent decisions, 2026-10-06** (not separately approved):
- **Where:** the presets' drawer's tools, before ×: the version (`CA-72 0.1.0`) and CHECK FOR
  UPDATES. The strip's row is full (R12), and the panel is the instrument's.
- **Through the system's curl,** not a network library linked in: Windows' own
  `System32\curl.exe` (Windows 10 1803 on), macOS's `/usr/bin/curl`, elsewhere the one on the
  path. It asks `api.github.com/repos/idlefoundry/ca-72/releases/latest`, which names only
  published releases (no drafts or pre-releases): HTTPS only (redirects too), 20 s and 1 MB at
  most, as `CA-72/<version>`, nothing else sent; on Windows without a console window.
  serde_json, which nih-plug already links, reads the answer: the notices are unchanged.
- **No thread waits for it** (R23): curl writes into a file the editor makes afresh in the
  temporary folder (never one already there, nor through a link), and the editor looks each
  frame whether curl has finished. Closing the editor ends curl and removes the file; past
  30 s the check fails. A curl gone with its exit unknown (a host that reaps its children
  itself, ignoring SIGCHLD) has its answer read all the same and judged by itself; curl
  writes none on an HTTP error.
- **What it shows:** CHECKING…; `<version> IS UP TO DATE` (this release or a later one) and
  CHECK FOR UPDATES again; `<new> IS OUT (THIS IS <this>)` in the accent, and DOWNLOAD; after
  DOWNLOAD, CLOSE THE DAW, THEN INSTALL IT; COULD NOT CHECK (no curl, no answer, an HTTP
  error, an answer without a version) and RELEASES PAGE; NO BROWSER WOULD OPEN. A version is
  three numbers, compared in order.
- **DOWNLOAD opens the installer in the browser,** which downloads it: the release's
  `CA-72-<version>-Windows-setup.exe`, `-macOS.pkg` or `-Linux-x86_64.tar.gz`, as the README
  lists them; the release's page where it has none for the system, or the system has none
  (Linux on ARM). Windows' shell (`ShellExecuteW`) opens it, macOS's `open`, Linux's
  `xdg-open`, and only an address under `https://github.com/idlefoundry/ca-72/releases`,
  whatever the answer says. An `open` or `xdg-open` that exits in failure (with no display,
  `xdg-open` exits 3 and opens nothing) turns the message to NO BROWSER WOULD OPEN.
- **SECURITY.md** no longer says the plug-in opens no network connections: it says when it
  connects and what it sends. The README says how to update, and the check's limits (curl,
  no Windows proxy settings).
- **Tests:** `update::tests` (versions; each system's installer found; addresses elsewhere
  refused; answers without a version; every scene fits the drawer whole; a newer release's
  DOWNLOAD and what follows, and this release up to date and checked again, each through a
  stand-in for curl printing a canned answer; curl missing or failing; an opener failing
  once started; the editor closed mid-check ending curl at once and leaving no file; and,
  run by hand, GitHub asked, the releases' page opened in the browser, and the latest
  release's installer for the system downloaded by it), `ca72-panel`'s `presets::tests`
  (the button found where drawn, and not where there is none), and the editor's
  `the_drawer_checks_for_updates`.

**Evidence (2026-10-06):** on Windows 11 (the GNU toolchain as R24), macOS (the Mac, Apple
silicon) and Linux (an x86-64 desktop running Hyprland), the tests of `ca72-plugin` and
`ca72-panel` all passed, R26's real-window tests among them on Windows, and clippy with
`-D warnings` passed on both crates and all targets (on Windows the whole workspace with
`--features ca72-plugin/standalone`); `cargo fmt --all -- --check` and
`scripts/notices.py --check` on Windows. On each system, by hand, the system's own curl
found 0.1.0 up to date, and the browser (Chrome on Windows and macOS, Chromium on Linux)
opened the releases' page and downloaded 0.1.0's installer for the system, its SHA-256 the
release's `SHA256SUMS.txt` gives (the `.pkg` signed and notarised). A build calling itself
0.0.9, so that 0.1.0 was newer:
- **REAPER 7.79 on Linux** (a portable copy scanning only the build's folder; the clicks
  sent to the editor's X11 window): the drawer opened below the strip, the window growing;
  CHECK FOR UPDATES, then CHECKING…, then `0.1.0 IS OUT (THIS IS 0.0.9)` and DOWNLOAD;
  DOWNLOAD had Chromium download the Linux installer (its SHA-256 the release's) and said
  CLOSE THE DAW, THEN INSTALL IT; no process of the check's was left, nor its file.
- **The standalone on Windows** (started without a console, as a host is; the clicks posted
  to the editor's window; a screen at 200 %): the same, `System32\curl.exe` run by it with
  the arguments above, no console window appearing, its file removed; DOWNLOAD had Chrome
  download the Windows installer (15,419,949 bytes, the release's); closed, it exited.

Not tried: a host on macOS or Windows (the Mac's Live and Bitwig need clicks this testing
could not make there), an HTTP proxy, a release whose installer is missing.
