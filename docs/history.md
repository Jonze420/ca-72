# How the model was made: its decision record

The circuit model in this repository was developed from 2026-09-27 to 2026-10-01 inside a
digital audio workstation (DAW), as one of its instruments, and then moved here to be
released on its own. This file is that DAW's record of the decisions about the model: the
section on the model itself (its circuit lab, its quality modes and its performance work),
and four later entries that concern it. The code and the derivation documents
(`docs/circuit/`) refer to it as `history.md`.

*Lightly edited for publication.* The DAW's internal name, its labels for this work and for
its own decisions, its crates' and files' names and the machines' names are replaced by
descriptions, and the owner's replies are given in indirect speech. What moved to this
repository is named as it is here: the model's crate is `ca72`, the circuit lab and its
command `ca72-lab`, the derivation documents `docs/circuit/`. Otherwise the entries are as
they were written.

They describe the model as a device of that DAW, so they also mention parts of it that are
not here: its engine, its device catalogue and patch library, its command bus, CLI and MCP
server, its GUI's panel, and its design and other decisions, which are in its own record,
not this one. "The owner" is the person who commissioned the work. Only the Potato quality
mode is released in the plug-in (`README.md`); No Compromises and High Fidelity remain in
the model and the lab.

## A circuit-derived Minimoog Model D (the owner's request)

**Owner's request, 2026-09-28:** a complete, playable software Minimoog Model D of about
1972-73 with the second-generation ("old") oscillator board (CA3046 arrays), derived from
its circuit, component models and calibration documents rather than sampled or voiced by
ear; offline circuit simulation as a development stage; every approximation labelled; the
owner verifies the audio against real Model D recordings, then performance is benchmarked.

**Owner's answers, 2026-09-28:**
- **Where:** on the Linux reference machine (Ryzen 7 7800X3D), on a branch of its own in
  the DAW's repository, started from the Mac's main line, in new files as far as possible;
  merged into the main line when the DAW's milestone then in progress is done.
- **Tools:** ngspice and numpy/scipy/matplotlib installed in user space (no sudo, no system
  packages): ngspice 47 from its source release into `~/.local/opt/ngspice-47`; a Python
  venv under `target/`.
- **Hardware reference:** existing recordings, revision unknown: comparisons with them
  are qualitative.

**Agent decisions, 2026-09-28** (plan in `docs/circuit/plan.md`):
- **Three concerns, three places:** the specification (`docs/circuit/`,
  `circuits/boards/`, `circuits/models/`), the offline reference (`ca72-spice` runs
  ngspice; `ca72-lab` builds benches, calibrates and measures), the real-time models
  (`ca72`, with no ngspice in its dependencies; then modules in the DAW's patch library, so
  the instrument is one of its patch devices).
- **ngspice 47 is the reference:** independent of our code, mature device models and
  convergence aids; too slow for real time. Tests that need it say `SKIPPED` without it
  unless `CA72_REQUIRE_NGSPICE` is set; simulator errors are errors, never partial
  results (a probe test shows a missing model and a floating node both fail).
- **Sources are not committed:** their URLs and hashes are
  (`docs/circuit/sources/manifest.tsv`, 234 files); `scripts/fetch-sources.sh`
  fetches them. fantasyjackpalance.com asks that its scans not be re-hosted, and vendor
  SPICE models may not be redistributed (Central Semiconductor's say so).
- **Device models are written here from data sheets** (`mm-devices.lib`, a Boyle
  macromodel for the 741), with their provenance and confidence
  (`docs/circuit/components.md`); vendor models are cross-checks only.
- **The configuration is defined by drawings, board by board**
  (`docs/circuit/manifest.md`): board 1 is Figure 9-3, which is the 1972 drawing 08-001
  after factory kit 94-001 (prescribed in July 1973 for boards above serial 1300); the
  as-built values are recorded. Two choices wait on the owner (manifest Q1, Q2).
- **Where the drawings do not settle a value, the assumption is explicit and tested**
  (`docs/circuit/assumptions.md`): the 1st-edition range switch string (10 ohm steps,
  -5 V sensed at the 2' end, LO five octaves below 32'); the control inputs at rest (pitch
  wheel in its detent at +6.12 V, dwg 1449 and Folkman; TUNE at +7.52 V, Figure 9-17).
  With those, the factory calibration lands the trimmers inside their ranges; with all
  inputs at 0 V it could not (1.5 octaves short), which is how the resting voltages were
  found.
- **Both factory tuning procedures run as tests:** Folkman's (July 1973: 2', 440/3520 Hz,
  octave trim 2' to 8') and Norlin's replacement Table 5-3 (4', 220/1760 Hz, 32').
  They agree: R11 184 ohm, R8's wiper 0.135/0.140, octave step 0.2989/0.2987 V; the octave
  step is the keyboard's 1.0176 V per octave (8.48 mA through 10 ohm per key, dwg 1436)
  carried through R43, as it should be.

**Owner's answers, 2026-09-28 (after the oscillator increment):**
- **Board 1's default is Figure 9-3**, the board after factory kit 94-001 (the factory's
  1973 standard for boards above serial 1300); the as-built 1972 values (dwg 08-001) stay
  a selectable variant.
- **Board 4's default is Figure 9-11's board with Folkman's July 1973 values** for the
  VCA's R2 (160K), R8 and R28 (4.7 ohm) and the A-440 feed R40 (10K): inferred to be a
  1973 unit's values after serial 2000 (not documented as such). The 1970-71 drawings'
  values (47K, 27 ohm, 1K) and Figure 9-11's (82K, 8.2 ohm, 22K) stay selectable.

**Agent decisions, 2026-09-28 (the filter increment; board4.md, numerics.md):**
- **The filter's loop is solved whole, without delays:** nine states (the ladder's four
  capacitors, the output coupling, C10, nt, b5, C27) in one implicit trapezoidal step with
  Newton's method, so no feedback path gains a sample of delay. Measured need: leaving out
  nt (C9, R6) or b5 (C24) put the response 1 dB off below 50 Hz with emphasis.
- **Device effects the ladder needs are derived from the model library at the bias
  current:** base currents (each stage's current, the input pair's input resistance),
  series resistance, high injection (IKF), junction and diffusion capacitance. Each was
  found by removing it in ngspice and measuring the difference (up to 1.5 % of gain and
  5 cents at high cutoff); A12 records the balanced-pair approximation.
- **The gain recovery amplifier is its ngspice DC transfer, its capacitances left out**
  (A11): up to 4 cents and 2.5 dB on +25 dB peaks at 11 kHz. Reason: the TIS97's
  capacitances are generic values (B4-3), and a dynamic output stage costs four more
  nonlinear unknowns per step; revisit if the owner's comparison points there.
- **Prewarping the ladder's corner up to 16 kHz**, measured against none, 8, 12 kHz and
  unlimited (numerics.md). **4x oversampling** for now; 8x is shown to converge (0.17 dB at
  20 kHz) and the choice is the benchmarks' (stage 7).
- **The filter's converter is tabulated, not solved per sample:** its circuit solution
  (full Gummel-Poon Q26 and Q28, Q28's collector on the ladder's tail) takes 0.5 ms, so
  ln I0 and Q26's base current are tabulated against the control node's voltage, which
  the node's resistors set as a Norton source (the table does not depend on which
  controls feed the node). Rebuilt when the temperature or the RANGE trim changes (0.6 s).
- **The VCA's trims are set by the factory's balance procedure, run on the model**
  (`VcaCircuit::calibrated`), as the oscillator's are: at mid-travel the circuit leaks
  EXT. LOUDNESS into the output at 0.067 V/V.
- **The VCA's bias is split by speed:** Q18 and Q21 follow the contour within a sample
  (directly, or through the chain's node nn) and are solved every sample; Q1 and the
  pairs' base-current ratios follow the chain's capacitors and are solved every 1/3000 s
  and interpolated. Measured against solving everything every sample (numerics.md).

**Agent decisions, 2026-09-28 (board 2):**
- **Figure 9-7 transcribed with Figure 9-17's and the parts list's panel values:** ATTACK
  and DECAY 1M audio rheostats, SUSTAIN 5K linear across +10 V, AMOUNT OF CONTOUR 5K
  linear (R17 through R21 in the parts list; Figure 9-17's scan reads "56"), GLIDE 5M
  (No. 1 taper). The DECAY switch follows the manual's text, not Figure 9-12 (B2-2).
- **The contour generators' flip-flops are latches in the real-time model** (their
  transitions take microseconds in ngspice); everything else is solved from the circuit
  every sample, the latch's reset threshold derived from its devices.

**Agent decisions, 2026-09-28 (the first voice; voice.md):**
- **A voice before the instrument is complete (v0).** The models so far are wired as the
  instrument wires them (oscillator 1, the mixer's bus, the filter and its control node,
  the VCA, the contours) and played from patch files by `ca72-lab play`. Reason: the models
  are heard together early, and a whole-voice test catches what the per-part tests
  cannot. The first render found a missed first trigger that no part test covered.
- **Lowest-note priority and single triggering are the circuit's**, not a choice: the key
  string's lowest closed contact sets its voltage, and the trigger bus stays at +10 V while
  any key is held.
- **The voice's output is the main output's voltage across 10K, 5 V as 1.0**, a fixed
  scale rather than one set per patch, so levels compare between patches and with a
  recording once one level is matched.
- **The contours run at half the output rate, interpolated** (assumptions.md A17): at
  24 kHz they stay within 86 mV of ngspice with trigger edges within 0.19 ms; at 12 kHz the
  fastest attack was 183 mV off. The choice is revisited with the benchmarks (stage 7).
- **The contours' trigger transistors are solved to convergence** (up to 60 damped Newton
  steps, not 8). With 8, a trigger at a low rate converged to the wrong state and the
  first note was silent.
- **Interim, until each is modelled:** the pitch comes from the key string's voltage
  without board 2's keyboard circuit (no GLIDE, no hold offsets); oscillators 2 and 3, the
  noise, the external input and the A-440 are not in the voice, and their mixer channels
  load the bus switched off.

**Agent decisions, 2026-09-28 (the keyboard circuit; board2.md, numerics.md):**
- **The keyboard circuit is solved as its netlist**, by a small nodal solver in
  `ca72` (`mna.rs`: ngspice's device equations, backward Euler, Newton with
  SPICE's limiting), not reduced by hand. Reason: its behaviour is a feedback loop through
  two JFETs and three transistors that switches between tracking, slewing and holding,
  and each reduced form tried would have had to be checked against the circuit anyway.
  It costs nothing while the circuit is still (it is stepped in blocks), about 0.2 of real
  time in a busy passage; the solver serves the next such circuits (oscillator 3's control
  stage, board 3). Its results: every key within 0.1 uV of ngspice, glides within 0.2 mV.
- **The key string is part of the circuit**: the lowest and highest keys held both join
  the pitch bus to it, so holding two keys shorts the string between them, as it does.
  The current source is a Thevenin source fitted to its own operating points.
- **The key contacts' resistance and order are assumptions** (A18): 0.1 ohm, and a
  released key's pitch contact opening 2 ms after its trigger contact (the service
  manual requires only the order). Measured: two keys held put the lower 1.1 cents sharp
  at 0.1 ohm; releasing both contacts together would hold the note about 6 cents sharp.
- **The oscillators' factory tuning plays its keys through the keyboard circuit**, as the
  procedure is done on the instrument; the circuit's 0.3 % scale error and its offset are
  absorbed as they are there.
- **Oscillators 2 and 3 as Figure 9-3 draws them, checked by the manual's own figures**:
  the FREQUENCY controls' travel (14-17 semitones), OSC. 3 CONTROL's wide range and
  the LO range's 2-5 s clicks (5.36) all come out of the transcription in ngspice, which
  is how the drawing's harder readings (IC8's supply connections, R163/R156 as the buses'
  pull-ups, Q37's feedback) were settled. The FREQUENCY pots' clockwise ends follow from
  the pitch rising clockwise (the drawings do not mark them).
- **The reference circuit's op-amp offsets are part of the real-time model** where no
  tuning absorbs them: IC8's (0.188 mV), the summers' (18.6 uV) and the -5 V line's
  (19 uV), measured in the lab's 741 macromodel by a test. Without them oscillator 3 with
  OSC. 3 CONTROL off was 2.7 cents off ngspice; a real unit's 741s are off by more, at
  random.
- **Oscillator 3's reverse sawtooth is solved as its circuit** (the nodal solver, C10 a
  state), only while its sawtooth or reverse sawtooth is heard.
- **The oscillators' core model is left as fitted (to 4.2 kHz)** for now: -1.25 cents at
  5.6 kHz (A20); a refit waits on the owner's comparison or the benchmarks.
- **GLIDE is an audio-taper rheostat** (5M): Figure 9-17 labels it "R2 5M AUDIO" (the
  parts list calls it "No. 1 Taper"); its law is the generic one (A9). Its resistance
  rises with the knob, so it is the pot's counterclockwise end and wiper.

**Agent decisions, 2026-09-28 (board 3 and the wheels; board3.md, assumptions A1,
A21-A24). The owner approved the scope, the headphone amplifier left out:**
- **The headphone amplifier (circuit No. 11) is not modelled**, at the owner's direction.
- **The regulators are transcribed and measured in ngspice, not modelled in real time.**
  Trimmed as 5.6 does, each rail is within 48 milliohm from 10 Hz to 100 kHz and takes the
  rectifier's 120 Hz ripple 78 dB down: about 0.3 mV on a rail, under 0.4 cent at any
  pitch input. The rails stay ideal (A1). The MPS-U05/U55 pass transistors' models are
  generic (B3-3); at half their gain the impedance is 72 milliohm.
- **The noise generator is its circuit's small-signal model**, linearised at its
  operating point by the nodal solver and discretised by the trapezoidal rule at 4x, driven
  by seeded Gaussian noise. Reason: its transistors carry microvolts to millivolts, so it
  is linear; the model's response is the circuit's (within 0.001 dB of ngspice), not a
  fitted pinking filter. Its loads change in place (the outputs are AC coupled), at most
  every 64 samples, keeping its state.
- **The noise's level is the factory's calibration** (A23): the white 6 dB under
  oscillator 1's triangle at the output, both channels at VOLUME 4, over 20 Hz to 20 kHz
  with the filter open. VOLUME 4 is Table 5-3's, and the only setting under which 5.27's
  requirement that both white and pink fall in its window can be met: at VOLUME 10, R50
  puts the pink 7 dB under the white. The pink's level is then the circuit's: -6.02 dB.
- **The modulation mix amplifier is its DC transfer**, solved from its circuit over
  MODULATION MIX's travel, with the output stage's two regimes (Q7 sourcing, or off with
  R30 alone; A24). It is linear to 0.05 % and flat to 100 kHz in ngspice. It uses the last
  sample's oscillator 3.
- **The switches' off states ground their lines** (A21); **the wheels' laws** are the
  drawings' values with a generic taper for the MODULATION wheel (A22). The pitch wheel's
  +-2 V gives 16.1 semitones of travel (5.35: 13-17).
- **A patch's controls can move while it plays** (`"moves"` in `ca72-lab play`), and every
  key is checked by one setter, so a misspelt or out-of-range key is refused whether it
  sets the panel or moves it.
- **Found and fixed**: the pink network was transcribed as two shunts where Figure 9-8
  draws C3 and R8 in series. The voice's check against 5.27 caught it (the pink sat 12 dB
  under the white).
- **Interim**: the noise generator runs whether or not it is heard (about 0.1 of real
  time); left for the benchmarks.

**Agent decisions, 2026-09-29 (the external input, the A-440, the filter's calibration, the
DAW device, the panel and performance). The owner asked for all of these (the external
input and the A-440, the filter's calibration and the DAW's patch device; a 1:1 panel GUI
adjudicated by a critic agent; then a 100 times faster voice with no loss of quality or
latency); the choices below are the agent's:**
- **The external preamplifier and overload lamp are solved as their circuit** (the nodal
  solver), the preamplifier by the trapezoidal rule at four substeps with the halfband
  resamplers (within 0.03 dB of ngspice to 20 kHz). The solver gains the theta method; its
  default stays backward Euler, so the keyboard circuit and the reverse sawtooth are
  unchanged. The jack's voltage is the input sample times 5 V (A25); in the DAW the side
  chain is the jack.
- **The A-440 plays its circuit's unloaded waveform at 440 Hz** (A26): as transcribed and
  loaded as drawn it cannot be trimmed to 440 Hz (B4-7), which contradicts the manual's
  procedure and its description of Q4 as a buffer.
- **The filter is calibrated by Folkman's procedure run on the model**, and the trims are
  kept as constants (`filter_cal::FACTORY`) that a test re-derives. "Filter Scale" as
  written diverges on the model; the state it aims at is solved directly.
- **The noise's calibration moves to VOLUME 10** (A23): at VOLUME 10 the model's triangle
  meets 5.8's "1 +- 3dB" (+0.8 dB) and Folkman's "-5dB maximum" reads as the noise at its
  maximum.
- **The Minimoog is a patch device, and the patch runtime gains keyboard modules**
  (`Module::keyboard`, `Module::key`, both defaulted so no other module changes): the
  device runs one instance of such a module, gives it every note at its frame and neither
  resets nor frees it. Reason: the Minimoog's lowest-note priority and single triggering
  are its keyboard circuit's, which needs every key; the runtime's per-voice notes (voices
  1 is last-note) cannot give them. A new engine device kind was the alternative; it would
  bypass the DAW's patch devices and every place that knows them.
- **Its parameters are the panel's in the panel's units** (43), plus MAIN OUTPUT VOLUME as
  its pot into the 10K load. The voice is built once per sample rate and copied (its
  calibration is the circuit's, not the panel's); each device seeds its own noise; the
  factory tuning uses `libm`, as the DAW's engine does; a key press and a noise load change
  no longer allocate.
- **The panel** (in the DAW's GUI, built by a builder agent and adjudicated
  by a critic agent against photographs of real Model D panels): an SVG recreation shown in
  a `minimoog` device's card (recognised by its 43 parameters and a `synth.minimoog` node),
  every one of the 43 parameters operated from its control and checked by a scripted GUI
  session; a button in the card's head switches to the generic parameter view. Its
  colours and typeface are the GUI's tokens, the same in
  every theme (it is a picture of an object). The Moog logo is not reproduced: the name
  board carries "minimoog" in plain type. POWER is the device's bypass (its lamp lit while
  it plays). The critic's reviews (five rounds, the last a MATCH) set the knobs' form and
  the selectors' dial geometry from the photographs, the PITCH wheel's detent without a
  spring, TUNE's parameter to its printed dial (-2.5..+2.5), the legends' width (the
  typeface stretched 9 %, the section titles not), the left hand controller's wheel slots,
  a plain jewel for OVERLOAD and rounded rocker paddles; the GUI session checks that no two
  of the panel's texts come within 2 units and no title is squeezed (both regressions the
  critic found once); the instrument is fitted in the pane by
  default with a Zoom toggle to its legible size (a view state, not in the document). Parts the device does not model (PHONES and its volume, the left hand
  controller's jacks) are drawn dimmed with a "not modelled" tooltip; no keyboard is drawn
  (there is no API to play notes live from the GUI; notes come from clips and MIDI). The
  OVERLOAD lamp is drawn but stays unlit: the device has no way yet to report it to the GUI.
  The GUI sessions' runner now takes the DAW's binary from the environment or
  `CARGO_TARGET_DIR`.
- **A patcher fix found on the way**: a palette entry added by double click always went to
  one point of the canvas, on top of (or, by its random ID, under) whatever was there; with
  `io.notes` grown by an earlier change the DAW's patcher session failed about half the
  time. It now goes
  to the free spot nearest that point (a port's reach from every node), or right of them
  all with the view panned to it. The DAW's full GUI suite passes.
- **Performance: "no loss" is measured, not assumed.** Every scenario must render within
  1e-7 of full scale of a recording made before any change, every ngspice comparison must
  keep its figures, latency must not change (`ca72-lab perf`). Kept: oscillators nothing
  hears are not computed; the converters' solves reused on exactly repeated arguments;
  contour parts at rest not solved again (A28); the contour sections' decay by an analytic
  Jacobian; the filter's pairs warm-started. Result: 1.6 times (1.4-2.5 by scenario),
  within 2.2e-9 (-173 dBFS). **The 100 times asked for is not reached**: the rest is the
  models' own work (numerics.md, "Performance", says what further exact work could give and
  what would change the sound).
- **Performance, a second pass** (the same night; numerics.md, "Second pass"; agent's
  decisions, not yet seen by the owner). A `profile` feature times the voice part by part
  (`ca72-lab perf`; off by default, compiled out). It found solver waste rather than model
  work: the series junction's solve bisected away from roots it had found (13 to 55
  iterations where 1 or 2 do); contour roots probed the far ends of their brackets;
  transistor and root slopes taken by differences. Fixed, with exact reformulations: the
  filter's Newton system by its structure and its step's end by a first-order update,
  Gummel-Poon's base charge in closed form (more exact than the six fixed-point passes it
  replaces), the pairs' tanh from exact anchors by the addition formula, the noise model's
  rows interleaved, the nodal solver's elimination over its nonzeros only. Result: 3.8
  times the first recording's speed over all seven scenarios (2.5 to 5.6), within 2.6e-8
  of full scale (-152 dBFS; the most where the self-oscillating filter starts, which
  amplifies rounding), every ngspice figure kept, latency unchanged; the Minimoog's golden
  renders re-recorded (their last bits). Rejected: a chord Newton step in the filter (its
  error could accumulate in self-oscillation). **Still not 100 times**, and not reachable
  without changing the output: what exact work is left is worth perhaps 10 to 15 %; the
  external input's preamplifier (24 us a sample, 1.5 times real time with it on) needs its
  own solver. Going further means changing the models or relaxing "no loss" to an audible
  threshold: the owner's decision.
- **The Minimoog live: real-time priority, and the voice on three threads** (2026-09-29;
  approved by the owner provided the audio quality was unaffected). Playing a demo project
  through the EVO4 showed that live the worst period decides, not the average: the keyboard
  circuit's eight substeps a sample after a key changes pitch took up to 17 ms of a 21.3 ms
  period, the voice up to 32 ms. The owner granted real-time priority on the Linux
  reference machine (`realtime-privileges`, the `realtime` group); it removed scheduling
  jitter but not the overruns. The voice was then split into its keys, keyboard circuit,
  control part (contours and noise) and audio path, and in the DAW the keyboard circuit and
  the control part run on threads of their own ahead of the audio path
  (`crates/ca72/src/threaded.rs`, safe Rust). The samples are the same to the bit (a test
  over notes, GLIDE, panel changes, the external input and the A-440 in blocks of every
  length; the golden renders unchanged; the real-time safety test covers the workers).
  Worst period on the bass line 29.6 to 23.0 ms; live, two Minimoogs at 2048-frame blocks
  miss about one period a minute, at 1024 frames about one in twelve. Open: the keyboard
  circuit, now the slowest part, is what a 1024-frame block needs faster; the GUI opens the
  stream at 128 frames with no setting for the block.
- **Three quality modes** (2026-09-29, the owner's direction and answers). The Minimoog gets
  three modes, a device parameter switchable while it plays (the agent's assumption, to
  confirm; renders may always use No Compromises):
  - **No Compromises**: perfect fidelity, meaning every render within 1e-7 of full scale
    of the model as it stands (below a 24-bit output's last bit; the rule the performance
    passes used), and every ngspice comparison keeping its figures. Every possible
    optimisation short of that, to make it playable end to end (the external input, the
    CV inputs and S-TRIG included): at least one instance in real time on the Linux
    reference machine at 256-frame blocks (5.3 ms) in the worst case (GUI open, every
    parameter automated).
  - **High Fidelity**: compromises outside human hearing, proven by measurement: the
    difference from No Compromises below -120 dBFS on every scenario, pitch within 0.01
    cent, timing within a sample.
  - **Potato**: compromises allowed, fidelity kept as high as possible (measured against No
    Compromises); a hundred instances at once on that machine, each fully automated.
  Order (the owner's choice): the owner compares the voice with recordings of real
  instruments first, while the agent builds a worst-case benchmark (the worst block, live
  and offline, as a pass/fail check) and the missing model parts (CV inputs, S-TRIG, the
  OVERLOAD lamp to the GUI); then No Compromises, High Fidelity, Potato.
- **The Minimoog's rear jacks as signal inputs** (2026-09-29, the owner's choice). The
  oscillator, filter and loudness control inputs and EXT. S-TRIG reach the device sample by
  sample, as its external audio input does, not as parameters driven at the control grid: a
  DC-coupled interface input or any DAW signal drives them exactly, audio-rate CV included.
  This needs an addition to the DAW (its design had CV in only as a modulation source). The
  agent's conventions, to confirm: a signal of 1.0 is 10 V (the DAW's hardware CV scale,
  `volts_per_unit`); an input with nothing connected is an empty jack, its normal contact
  in place (the filter's input grounded, the loudness input tied to +10 V through 33K, the
  oscillators' external bus held by its 33K pull-up, S-TRIG open); a plugged source is
  ideal (no output impedance); S-TRIG is closed while its signal is above 0.5. Done in the
  model the same day (voice.md, "The rear jacks"): each jack checked against ngspice; with
  EXT. LOUDNESS plugged the VCA's bias is solved every sample instead of at 3 kHz (a
  tremolo's sidebands 8 dB closer to the circuit); the patch runtime's inputs a module may
  have raised from 8 to 16 (`MAX_INPUTS`: the Minimoog now has 9). Still to do: feeding a
  device those signals from outside it (the addition to the DAW).
- **No Compromises, first pass: the nodal solver** (2026-09-29; the agent's decisions under
  the owner's rule, every render within 1e-7 of full scale). Newton's starting point
  extrapolated from the last three accepted points, a step's charges taken from its last
  load, one exponential for a transistor's `exp` and `expm1`: renders within 2.9e-9 of full
  scale of the reference (the model as it then stood), every ngspice comparison kept. The
  stamps' positions worked out once and the elimination recorded and replayed:
  bit-identical. The worst case's load in turn 3.52 to 2.37 times real time. The golden
  renders re-recorded (the three Minimoog devices'). numerics.md, "No Compromises";
  `ca72-lab solvers` and the `worst` scenario of `ca72-lab perf` added to measure them.
  Next: the voice's audio path split across threads (the preamplifier ahead of it, the
  filter and VCA behind).
- **No Compromises: renders judged against their own spread under rounding** (2026-09-29;
  the owner's choice of a rounding-noise test, asked after a change of rounding was found
  to cross 1e-7 in the worst case's load). Each recording carries twins: the same render
  with every elementary function's result an ulp up, then down (`ca72::ulp`, the `twins`
  feature). A render whose twins part by more than 1e-7 may differ from the reference by as
  much as they do in each quarter second; every other render by 1e-7 (`ca72-lab perf check`
  fails otherwise). The probes showed the parting comes from the solvers' stopping
  decisions, not the circuit's dynamics (numerics.md, "No Compromises"). The owner also let
  the agent stop the demo project's playback, which had been running for hours and loading
  the machine (2026-09-29).
- **The VCA where EXT. LOUDNESS overdrives Q21: fixed** (found and fixed 2026-09-29; the
  first of the two items the owner then chose). Between about 6.2 and 7 V at the jack (with
  an ideal source; the contour anywhere) the real-time VCA's bias solve did not settle: its
  passes took each pair's drop at the last pass's tail current, whose slope (Vt over the
  current) grows without bound as the pair cuts off, and the iterates circled. Its tails
  were then off by up to 1,360 %, its gain up to 140 dB (+24 to +84 dB where the circuit is
  shut). Each tail (Q18, Q21, Q1) is now solved together with the pair it feeds, the pair's
  junction an unknown and Kirchhoff's current law at its emitters closing the system
  (`devices::solve_tail_pair`: Newton's method on three junctions with the devices' own
  slopes, the step shortened as a whole to SPICE-like limits and halved while the residual
  does not fall), and the passes run until the tails agree to 1e-12. A new comparison
  (`the_loudness_jack_overdrive_matches_the_circuit`: the contour at -0.35, 0, 2 and 5 V,
  the jack from 5 to 9 V) has the tails within 0.1 %, the output at rest within 0.1 mV, the
  gain within 0.06 dB where the VCA is on and both off where the circuit shuts it, every
  solve settled. In the normal range the agreement is the same (the tails 0.006 to 0.008 %,
  J3's gain +0.059 to +0.062 dB): the pair's base current now comes from its own junction,
  as the circuit's does, and the renders move by at most 7e-6 of full scale there (-103
  dBFS), by up to 0.29 in the worst case's overdrive bursts. The reference for No
  Compromises moves to this model. The junction solver's per-junction clamps, which had
  turned its step's direction, now shorten the step as a whole. The worst case's twins now
  part by 6.4e-8, not 5.5e-2: that amplifier was this defect, and the worst case is judged
  by 1e-7 like the others.
- **No Compromises, second pass: the keyboard's solver, exactly** (2026-09-29; the second
  of the two items the owner chose: the keyboard circuit made faster without changing its
  results). Kept, each bit-identical (every scenario of `ca72-lab perf check` identical in
  every sample): the recorded elimination replayed from a flat list of its additions; a
  depletion charge's constants (two `pow`s beyond FC*VJ, which every forward-biased
  junction took at every load) worked out once per part; a step's charges accepted without
  copying parts. The worst case in turn goes from 2.39 to 2.13 times real time; threaded at
  256 frames from 23 % of blocks late to 3 to 4 %. Tried and dropped: products by
  reciprocals in place of divisions (outside the rule: the worst case moved 1.29e-7,
  perf-screech 1.18e-7 in a quarter second), a constant Jacobian base and a column-at-once
  replay (no gain). Found: the scheduler's placement of the voice's threads matters (pinned
  each to a core of its own, 2.2 to 2.5 % late; two to a core, 8 %), and with the threads
  apart the late blocks are the back's. The VCA's bias solve (every sample while EXT.
  LOUDNESS is plugged) is 90 % of its time and reads nothing of the signal path, so it can
  run ahead on a thread of its own; then the keyboard's heaviest blocks (14 to 21 us a
  sample in turn) are what is left. `ca72-lab solvers` reports each elimination worked out
  again and the solves by Newton iterations; `MM_PIN` pins `ca72-lab worst`'s threads
  (measurement only: the DAW does not pin its threads). The kernel setting for `samply` is
  not needed after all (a sampler preloaded into the bench did the profiling). numerics.md,
  "No Compromises".
- **The VCA's bias worked out ahead of the audio path** (2026-09-29; the agent's, under the
  owner's No Compromises goal). The VCA's bias (its tails, which follow the loudness
  contour and EXT. LOUDNESS, and the chain's states) reads nothing of the signal path, so
  the voice's control part (the contours' thread) works it out and hands the back twenty
  values a sample (`Vca::control` and `Vca::signal`; `Vca::tick` does both, as before).
  Bit-identical to the reference and, threaded, to the voice in turn. The threaded worst
  case at 256 frames: 2.3 to 2.9 % of blocks late, 0.6 to 0.75 % with the threads pinned;
  at 512, 2.5 % and 0.2 %. Left: the keyboard's heaviest blocks and the bias's spikes
  (numerics.md).
- **The VCA's bias on a thread of its own: six threads** (2026-09-29; the agent's, under
  the owner's No Compromises goal). The contours' thread feeds the bias's sample by sample
  (a worker may now feed another; if the feeder stops, the fed one stops waiting); the
  caller takes the bias and hands it to the back with the front's outputs. Bit-identical.
  The worst case at 512 frames runs in real time (30 s, no block late, the worst 98 % of
  its time); at 256 frames 0.13 to 0.34 % of blocks are late, each the keyboard's
  heaviest (up to 24 us a sample). A sample a stopped worker did not give is now silent
  and counted: before, the back ran on stand-in inputs and its output was heard
  (`a_stopped_worker_leaves_silence_not_a_wait`).
- **A helper thread for the keyboard circuit: built, measured slower, dropped**
  (2026-09-29; the owner chose to try it for the last 0.5 % of late 256-frame blocks).
  Within each Newton iteration a helper evaluated some of the circuit's parts (keeping
  their state) while the keyboard's thread evaluated the rest, which then added every
  part's stamps in their order. Bit-identical, and slower. In the bench at eight substeps a
  sample, against 13.1 us a sample: a helper for the transistors 13.6, for the transistors
  and diffusion charges 13.7, with the diodes 14.7, for every junction 16.0. In the
  threaded worst case at 256 frames it gave 97 late blocks against 38 without (pinned, 40
  against 31). A round trip between two cores carrying the voltages out and the stamps back
  costs about 240 ns here (measured: 95 ns with nothing carried). That is as much as the
  evaluation it moves off the keyboard's thread, and the stamps then take a second pass.
  The code was kept aside, not committed.
- **No Compromises' optimisation stops here** (2026-09-29; the owner judged it optimised
  enough). It stands at the worst case in real time at 512-frame
  blocks (1 to 4 late blocks of 11,250 over 2 min) and 99.5 % of 256-frame blocks on
  time (all the late ones the keyboard circuit's heaviest), every render identical to the
  reference. Lookahead (the threads ahead of the audio path a block ahead) is left
  unbuilt, as an option.
- **A second agent's review: Newton's bookkeeping trimmed; LTO and the native CPU target
  dropped** (2026-09-29; the owner passed the attempts list,
  `docs/circuit/no-compromises-attempts.md`, to another agent and chose its first idea and
  a trial of LTO). Kept, bit-identical: the residual's maximum only when a solve fails, the
  limiting started and the charges accepted only for the parts that have them, the history
  voltages swapped instead of copied. About 7 % off the keyboard at eight substeps a
  sample; in turn 2.01 times real time. Threaded at 256 frames the late blocks fall a
  little (clean 60 s runs: 13 and 21 to 8 and 19), not to none. LTO gave under 1 % and
  `target-cpu=native` was slower overall; neither is adopted. The review's other ideas
  are listed in the attempts file.
- **High Fidelity: how it is measured, and its speed** (2026-09-29, the owner's answers).
  Each limit is measured on its own path: pitch (the keyboard's
  control voltage and the oscillators' frequency) within 0.01 cent of No Compromises at
  every moment, allowing up to a sample of shift; note and contour events within a sample;
  the audio within -120 dBFS of No Compromises when both modes get the same pitch. A
  plain subtraction would count the lasting, inaudible phase offset any change to the
  keyboard's stepping leaves (around -40 dB), which would have ruled out changing the
  real-time bottleneck. Speed: one voice in real time at
  256-frame blocks under the worst-case load, no block late over 2 minutes; Potato covers
  many instances.
- **High Fidelity, first pass** (2026-09-29; the agent's, under the owner's limits). The
  quality mode is the panel's `quality` and the Minimoog's `quality` parameter (three
  choices, Potato among them so a saved value keeps its meaning: Potato runs as High
  Fidelity until it is built), switchable while it plays; the catalogue's Minimoog exposes
  it last, so every other entity keeps its ID (the preset's text re-recorded: its own ID
  and the new exposed parameter). `ca72-lab hifi` measures a mode on each path
  (`ca72_lab::quality`; a test runs it on the worst case). In High Fidelity the
  keyboard's, the preamplifier's and the lamp driver's Newton tolerances are 1e-5 V (from
  1e-9): pitch within 4e-8 cent, the audio within -147 dBFS, the heaviest blocks' iterations
  about 35 % fewer. Fewer keyboard substeps crossed the pitch limit (up to 8 cents at key
  changes, 0.7 cent while GLIDE moves) and were dropped. The threaded worst case at 256
  frames over 120 s: paced as a callback, 0 and 2 late blocks of 22,500; back to back 1 to
  2, the worst of them the system's (every thread's own work light). Not yet the goal (no
  late block over 2 minutes); the contour generator is next. `ca72-lab worst` takes
  `--quality` and `--paced`.
- **High Fidelity meets its speed goal** (2026-09-29; the agent's). The contour
  generator's solvers stop at 1e-9 V in High Fidelity (from 1e-12 and 1e-11; the contours
  within 1.3e-14 V of No Compromises'). Paced as an audio callback, the worst case at 256
  frames ran five times 120 s without a late block (the slowest at 95 to 99 % of its time);
  back to back, 1 late block in 120 s. The margin is thin: the keyboard circuit's heaviest
  blocks are the slowest thread.
- **Potato before High Fidelity's margin; its block size** (2026-09-29, the owner's
  answers). Potato comes next, as the owner asked; High Fidelity's thin margin waits, since
  Potato's cheaper parts, each measured by `ca72-lab hifi`, may give it more than tuning
  would. Potato's hundred voices run in real time at 256-frame blocks (the owner's choice),
  each under the worst case's load.
- **Potato, first stage: the circuit models at cheaper settings; noted as potential for
  High Fidelity** (2026-09-29; the agent's stage, the owner's note). In Potato the keyboard
  circuit steps once a sample, the contours run at an eighth of the rate, the VCA's bias is
  solved every 16 samples (again at once when EXT. LOUDNESS moves 2 mV), the preamplifier
  steps once a sample, the oscillators run at the sample rate and the filter at twice it,
  each switchable while it plays (resamplers prepared beforehand: no allocation). About 21
  us a sample in the worst case, from 37: not enough for a hundred voices (about 2), so
  explicit models follow. The owner noted this stage as potential for High Fidelity: each
  setting to be measured on its own against High Fidelity's limits (numerics.md,
  "Potato"). `ca72-lab crowd` runs many voices as a DAW's engine would; `ca72-lab hifi
  --quality potato` reports, not judges.
- **Potato, second stage: tables from the circuit models** (2026-09-29; the agent's). The
  noise stepped once a sample; the VCA's bias from a table of the settled full solve with
  its slopes against the chain's slow states (made once a process: the first voice builds
  about 0.5 s slower); each oscillator's converter from a table in the one drive its
  current depends on (within 0.001 cent of the full solve). About 12.6 us a sample in the
  worst case (37 in High Fidelity). The oscillators' pitch probe corrected: it was their
  input current, it is now the converter's output, which the frequency follows (High
  Fidelity's oscillator figures now 6e-7 cent at worst; its verdict unchanged).
- **Potato's preamplifier as its paths** (2026-09-29; the agent's): filters, the loop with
  C26's charge, the clip and the lamp from the circuit's parts, its input resistance and
  open-loop gain fitted to the circuit's gain (numerics.md, "Potato"). 4.2 to 0.1 us a
  sample; the external input's scenario -57 dB from No Compromises by spectra.
- **Potato's filter: its bias from a table, its pairs taken at once** (2026-09-29; the
  agent's). The ladder's bias from a table on 1024 points; each pair's tanh with its series
  drop folded into its thermal voltage; at most two Newton iterations to 1e-4 V. The filter
  4.2 to 1.4 us a sample, its fidelity unchanged; one iteration, or the sample rate, lets
  a self-oscillating filter stray, so twice the rate and two iterations are kept
  (numerics.md, "Potato").
- **Potato's keyboard at an eighth of the rate** (2026-09-29; offered 8 or 4, the owner
  kept 8). One step of 167 us every 8 samples, contacts read at the step, the
  output interpolated: contacts wait up to 0.15 ms, a pitch change ramps over 0.17 ms, a
  held pitch released mid-glide can be 2 cents off. 4.7 to 3.9 us a sample in the worst
  case (numerics.md, "Potato").
- **Potato's reverse sawtooth stage for its transistor alone** (2026-09-29; the agent's).
  The same equations, charges included, solved for Q37's two junction voltages instead of
  node by node: within 1.7 uV of the circuit; the worst case's mean block 0.99 to 0.93 ms.
- **Potato's goal: about 50 voices, not 100** (2026-09-29; the owner lowered the goal
  to about 50 voices). Still 256-frame blocks under the worst case's load; when it is
  met, an audio example for the owner.
- **Potato: the noise without its transistors' series resistances, sparse resamplers,
  knob laws and device parameters kept** (2026-09-29; the agent's). The noise's model 34 to
  22 states (within 0.008 dB of the circuit's); the filter's twice-rate resamplers' zero
  taps skipped (Potato's only: in the others the taps are 1e-17, not zero); the knob laws
  and the contours' temperature-scaled transistor parameters taken again only when they
  change (every mode; No Compromises unchanged to the bit). The worst case's mean block 0.93
  to 0.81 ms. `ca72-lab worst --features count` counts elementary functions by call site.
- **Potato: the VCA's signal path by Newton steps, the contours' spikes** (2026-09-29; the
  agent's). The output pair's Early effect by Newton steps from the last sample's split
  (within 0.15 uV of the full path), the pairs to 1e-9; the contours' trigger woken only by
  its own inputs, the dump node warm-started with its slope. One voice's worst block 2.56
  to 1.84 ms; 50 voices still 736 of 5625 blocks late.
- **The crowd measured as the engine's pool runs tracks; shared tables; the contours'
  transistors limited as the nodal solver's** (2026-09-29; the agent's). `ca72-lab crowd
  --pool` claims voices from one counter as the DAW's engine pool does (the fixed
  assignment kept for comparison). The tables every voice has the same of made once a
  process (every mode; the same values). In Potato: the filter's second iteration finished
  to first order, its elimination by reciprocals; Q20 and Q12 in the contours warm-started
  within the tick and limited by pnjlim. 50 voices in the pool on 16 threads: none late in
  30 s; on 15, 8 of 5625.
- **Potato's goal met: 50 voices** (2026-09-29; the agent's measure of the owner's goal).
  50 voices as the engine's pool, 256-frame blocks, each under the worst case's load, 120 s
  paced on 15 threads: none of 22500 blocks late (worst 4.96 ms of 5.33). The last steps:
  the keyboard's extrapolated starts limited to 50 mV and its steps to 16 iterations
  (halved beyond); Potato's own exponential and tanh (`fast`: within 2 ulp and 2e-14 of
  libm's, twice as fast, the same on every platform), used by every exponential inside a
  Potato part's tick through a scope (`ulp::PotatoScope`), the other modes libm's to the
  bit. An audio example for the owner follows.
- **The device runs Potato's voice in turn** (2026-09-30; the owner approved fixing the
  device). The threaded voice in Potato runs its parts on the caller's thread, the
  workers parked, so the engine's pool spreads many instances; No Compromises and High
  Fidelity keep their workers. 50 Potato Minimoogs in the engine, 256-frame blocks, 120 s
  paced with a real-time audio thread: none of 22500 blocks late (worst 4.78 ms). Memory is
  the same in every mode (about 8.4 MB a process, 7.2 MB of it Potato's VCA table, and 410
  KB a voice); trimming it was offered and not chosen.
- **The DAW's main line has the Minimoog; High Fidelity takes Potato's exact-enough parts;
  the panel's QUALITY selector** (2026-09-30; the owner approved the three steps). The
  main line merged into the branch and fast-forwarded to it, after the DAW's full CI
  passed; that first needed fixes the branch had been failing unseen: an unlisted `unsafe`
  test, a debug-only bracket check in the contours, the Minimoog GUI session's counts and
  a font-loading race in its legend placement. High
  Fidelity takes the fast exponential, the contours' lean solves, the VCA's signal path by
  Newton steps and the filter's reciprocals, each measured within its limits; not the
  reverse sawtooth stage (-115.8 dBFS in perf-screech), the keyboard's limited
  extrapolation (slower there) or the sparse resamplers (little, off the bottleneck).
  Under load it went from 18-124 late blocks a minute to none (numerics.md, "High
  Fidelity candidates"). The panel's QUALITY selector sits on the name board, not part of
  the instrument.
- **No Compromises' reference moved: its solves converge** (2026-09-30; the owner chose to
  fix it and re-record the reference). No Compromises' contours stopped Q20's and Q12's
  solves short in every scenario with notes (20 to 447 times a render: steps held too short
  to arrive), and the decay step twice in perf-ext. A correctness fix, not a compromise, so
  the rounding rule's reference moves: pnjlim for those transistors in every mode, 100
  iterations for the decay step, every solve that should converge counted (`unconverged`)
  and required to (a test in all three modes; `ca72-lab hifi`, `perf check` and `perf
  record`). No Compromises moved -101 dBFS at most (perf-screech), -164 to -270 elsewhere;
  its speed the same; the reference re-recorded, the old one kept.
- **A MIDI keyboard's wheels play the Minimoog** (2026-09-30; one of the items 1, 2, 3 and
  7 of the agent's list of what was overlooked, which the owner chose; the form the
  agent's). Live input carried notes only (an earlier limit of the DAW's), so a keyboard's
  pitch bend and modulation wheel did nothing.
  - **The engine delivers a MIDI input's channel controllers**: pitch bend (MIDI 1.0's 14
    bits or 2.0's 32, as -1..1) and control changes (0..1), from the channels the track's
    input takes, while it monitors, by frame (`ControlEvent`, `Controller`;
    `Device::controls`, called before a block when any arrived, for every device but note
    effects; within `MAX_CONTROL_EVENTS`, 256 a block, allocating nothing). When
    monitoring stops, each channel that sent one gets control change 121 (MIDI's Reset
    All Controllers), and a swap hands the channels over.
  - **A patch device passes them to its keyboard module** (`Module::control`) at the grid
    line at or after their frame, as it applies parameter changes. Other patch
    devices and plugins ignore them for now.
  - **The Minimoog maps them without configuration**, as a keyboard's wheels: the bend
    adds to the PITCH wheel's position (clamped to its travel), and control change 1 takes
    the MODULATION wheel as far as the greater of the two. Its parameters stay the
    panel's; the device reports the bend and the wheel as the indicators `pitch_bend` (0.5
    centred) and `modulation`, and the panel draws its wheels where the keyboard leaves
    them.
  - **The simulated studio's keyboard player** (`[[players]]`) gains `bends` and
    `controls`; the GUI suite's Minimoog session gets a studio of its own with one.
  - **Checked:** an engine test (channels filtered, MIDI 1.0 and 2.0, the reset on
    disarming and after a swap); the Minimoog through the API (a bend fully up and the
    wheel fully forward from MIDI play the panel's wheels there to the sample, and half a
    bend down from the wheel fully up is the wheel halfway); the DAW's server on the
    simulated studio from the CLI (the keyboard player's A3 on the track's meter, the bend and the
    wheel arriving, centred again when let go, reset when monitoring stops); the real-time
    safety test with bends and control changes among the notes; the GUI (the panel's wheels
    drawn at the keyboard's, their parameters unmoved, back when monitoring stops).
  - **Still open:** MIDI learn, so a controller cannot yet move any other
    parameter; channel pressure and polyphonic aftertouch are not delivered; controllers
    are played, not recorded; plugins do not receive them.
- **The pitch wheel's range, and a MIDI bend range** (2026-09-30; the owner asked whether
  the wheel's minor sixth was right, and chose to add a MIDI bend range). The
  wheel travels about 8.07 semitones each way (16.1 end to end): its +-2 V (Figure 9-2)
  enters the oscillators' summer through R12 150K against the keyboard's and the control
  jack's 51.1K, at the calibrated 0.984 octaves a volt; nothing trims it, and the service
  manual's check 5.35 accepts 13 to 17 semitones end to end, so real instruments spanned
  about +-6.5 to +-8.5 by tolerance. It stays so on the panel. A MIDI keyboard's bend now
  moves the wheel by the device's `midi_bend_range` (semitones, 0 to the whole travel,
  default 2 as most MIDI instruments bend), shown as MIDI BEND on the rear strip; the voice
  test holds `modulation::PITCH_WHEEL_SEMITONES` (8.07) to the measured travel within 0.01
  semitone. Checked through the API: at the top of the range a bend plays the panel's wheel
  to the sample; at the default a full bend plays the wheel 2 semitones' worth up (within
  3e-5, the f32 values meeting). The MODULATION wheel doing nothing in the owner's demo was
  the instrument's behaviour: with OSC. MODULATION and FILTER MODULATION both off it has no
  destination (the demo was then set for vibrato at the owner's request).
- **The rear panel drawn after the operation manual** (2026-09-30; the owner asked whether
  the real rear panel is known, and chose to draw it from the manual). The original
  operation manual names the rear connector strip's MAIN OUTPUT (HIGH and LOW level), EXT.
  SIGNAL INPUT, the control inputs for the oscillators' pitch, the filter and the loudness
  (pedals, joystick, sequencer) and the TRIGGER INPUT for an S-trigger (a two-pin
  Cinch-Jones socket, per a technician's forum thread), and its sheet 70-019 draws the
  back panel's ten trimmer holes, A to K. No photograph of the rear was found on Wikimedia
  Commons (the only ones are sellers' and forums', not ours to use), so the strip's order
  and spacing are the drawing's own and its labels the manual's words. The Rear strip is now
  that drawing: the trimmers and MAIN OUTPUT drawn, dimmed and not operable (the
  model is calibrated by the factory procedure; the device plays into its track), the five
  jacks that take a source drawn with a plug and its source's name when plugged, a click on
  one going to its source list below, where MIDI BEND sits too. Checked in the GUI session
  (five jacks, the trimmers A to K, a jack drawn plugged once its source is picked).
- **Automation overridden by hand, as Ableton Live does it** (2026-09-30; the owner found
  automated controls locked and asked for the industry standard; of Live and Bitwig's
  override, Pro Tools' Read (moved, then taken back by the automation) and Logic's Read
  (locked), the owner chose an override that lasts until re-enabled). Before, an automated
  control was locked in the GUI, and a value set from the bus, CLI or MCP was not heard (the
  lane always won).
  - **A `set` of an automated target's value overrides it**: every lane on that target
    (the same entity and field, however the path is written) gets `overridden = true` in
    the same transaction, so one undo takes the value and the override back together. The
    engine does not play an overridden lane: the target keeps the value set, playing or
    stopped. The field is optional (absent: played), so old projects stay valid.
  - **`automation.reenable {lanes? | target?}`** plays them again (all overridden lanes
    when given nothing); unsetting the field does the same. `list` and the `arrangement`
    query report a lane's `overridden`; `device.params` reports `overridden` for a
    parameter whose automation is overridden (and it is then not `driven`).
  - **The GUI**: automated or modulated controls are marked and operable (the Minimoog's
    knobs, the generic sliders; a modulated one's move sets the value the modulation moves
    about); an overridden one is marked (dashed on the panel, struck through in the generic
    view, where a ↺ hands that parameter back); the transport bar's Re-Enable is lit while
    any lane is overridden and hands every lane back; an overridden lane is drawn grey in
    the arrangement.
  - **Checked:** through the API (a track's gain automated to silence, set by hand: heard;
    one undo: silent again; overridden again and re-enabled: silent, the value kept; a
    device parameter's flags); in a GUI session (a slider moved overrides and lights
    Re-Enable; the row's ↺ and the transport's Re-Enable hand it back); in the Minimoog's
    session (an automated knob turns and is marked until re-enabled).
  - **Not done:** Live also re-enables a clip's automation when the clip is launched again;
    launched clips play no automation yet, so there is nothing to re-enable there.
    Writing automation (Touch, Latch) is not built: until it is, every move overrides.
    The mixer's faders override their automation when moved but do not show it.
- **A switch flips at a click anywhere on it** (2026-09-30; the owner's direction: every
  switch to toggle wherever it is clicked). The Minimoog panel's
  rockers and its POWER switch pressed the end clicked, so a click on the end already
  pressed did nothing; now any click flips them (Space and Enter as before). Checked in
  the GUI session: OSC. 3 CONTROL flipped by a click on its pressed end, its other end and
  its middle; POWER by the same end twice.
- **Several voices at once stalled the machine: bounded waits, a real-time watchdog, the
  workers following the audio thread** (2026-09-30; found by the agent's benchmark, which
  froze the owner's machine until it was reset; the fixes the agent's, not yet approved).
  Measuring two and three No Compromises voices at once (the owner's item 7), the kernel
  logged an RCU stall with a Minimoog worker spinning on the CPU, and the machine stopped
  answering. The cause: every part of the threaded voice runs at `SCHED_FIFO` 70 and
  waited for the others by spinning without end. With more such threads than CPUs, a
  thread spinning on a CPU where the thread it waits for is queued keeps that thread off
  it for good; and on this machine real-time throttling is off (`sched_rt_runtime_us`
  equal to its period, the owner's setting, left alone), so nothing stops them, and the
  kernel's interrupt threads (`SCHED_FIFO` 50) and RCU thread (1) starve under them.
  - **No wait spins for good** (`threaded::Backoff`): it spins 20 us, yields to the threads
    queued on its CPU until 200 us, then sleeps 20 us between looks. The idle wait between
    passes does the same before it parks. The lab's benchmark threads use it too.
  - **A real-time watchdog** (the DAW's real-time threads, now `ca72_rt`; Linux): with the
    first thread promoted, a thread at ordinary priority on each CPU beats every 50 ms and
    a thread above the audio threads (the highest real-time priority allowed, at most 99)
    looks every 100 ms. When a CPU's beat is 500 ms old while one of the process's
    real-time threads runs there, every real-time thread of the process is made ordinary,
    counted (`rt::demotions`) and said on stderr. It covers any cause (an overloaded
    engine, a plugin helper, a bug), and the threads PipeWire runs in the process too.
  - **The voice's workers follow the thread that plays it** (`Threaded::following`, Linux):
    real-time only while it is, so an offline render's, the golden tests' and an unpaced
    benchmark's stay ordinary (before, they held their CPUs at real-time priority for a
    whole render), and they follow a demotion. `ca72-lab worst` and `crowd` take real-time
    priority only with `--paced`; the DAW's engine crowd benchmark (with a block size of
    its own now) paces its warm-up.
  - **Checked without risk to the machine** (each test holds one CPU, at most for
    seconds): a real-time thread spinning on one CPU is made ordinary after 500 ms
    (a test of the watchdog); the voice with all six threads at real-time priority
    on one CPU plays 512 samples in about 10 ms with no demotion, where the old waits held
    the CPU until the watchdog stepped in at 500 ms and took 652 ms
    (`crates/ca72/tests/one_cpu.rs`). No kernel stalls since.
  - **Several voices, measured after the fix** (the engine at 256-frame blocks, 20 s each,
    the Linux reference machine, no demotions): High Fidelity 2, 3 and 4 voices 0 blocks
    late (before: 1 and 28 for 2 and 3); No Compromises 1 voice 2 to 50 late (as before, within the machine's
    noise: alternating runs with the old waits gave 15 to 130), 2 voices 150 (4 %), 3
    voices 376 (10 %). So: one No Compromises voice, or four High Fidelity voices, in real
    time at 256 frames on this machine; Potato about 50.
  - **Still open:** the engine's own overload (any heavy project) is guarded only by the
    watchdog, which makes the audio threads ordinary rather than shedding load; whether and
    when to make them real-time again is not decided (the threads it demoted stay
    ordinary; new audio threads, as when the transport is opened again, are promoted).
- **The Minimoog's rear panel in the GUI** (2026-09-30; another of the items the owner
  chose; the form the agent's). The external input (the side chain) and the rear jacks (the
  device's `inputs`) could be set only with `set` from the bus, CLI or MCP, so from the GUI
  the OVERLOAD lamp and the jacks were out of reach. The Minimoog's card gets a Rear button
  that shows a strip of five jacks, with either view:
  EXTERNAL INPUT, OSC. CONTROL, FILTER CONTROL, EXT. LOUDNESS and EXT. S-TRIG, each with a
  source list (empty, another track, or for the four control jacks an audio or CV input
  port), the channel (a track's L or R, a port's channels) and, for a track, the tap
  (after its devices or after its fader). Each choice is one command: `set` of the
  device's `side` or `inputs/<name>`, `unset` to empty it. The device's own track and the
  master are not offered. A device whose patch lacks a jack (made before the jacks
  existed) shows it disabled, with how to get it. It is a row of labelled jacks, not the
  rear panel's own layout. The panel's fit now sizes it across from its own box, so the
  card's header can no longer shrink it (it did, with the new button). Checked in the GUI
  session: the strip shows on request with its five jacks, EXTERNAL INPUT picks the Tone
  track (the device's own not offered) and the OVERLOAD lamp then lights through it,
  FILTER CONTROL takes the Tone track's right channel after its fader, EXT. LOUDNESS a CV
  input's second channel (the external input offers no ports), a jack emptied, and a
  jack set from the API shows in the strip.
- **The rear panel beside the left hand controller** (2026-09-30; the owner's request, with
  a screenshot of the card; the form the agent's, not yet approved by the owner). With the
  Minimoog's panel shown, the rear panel's drawing sat under the whole panel, below the
  left hand controller, and the space right of the controller (where the keyboard would be)
  was empty. The panel now holds the drawing there, in its own drawing: level with the
  controller's face and as tall (348 panel units, so 1874 across), 20 units right of the
  controller's wooden block, fitted and zoomed with the panel (the panel's `hold`). The
  jacks' lists and MIDI BEND stay below the panel. With the generic view there is no panel,
  and the drawing goes back above the lists (the rear panel's `home`). The Rear button
  shows and hides it as before. The GUI session checks that the drawing is right of the
  controller, level with its face and as tall, and inside the panel; that it is above the
  lists with the generic view; and that it is beside the controller again after. The
  panel's text-spacing check leaves the rear drawing's text out (its spacing is the
  drawing's own). Evidence: the DAW's Minimoog GUI session passed 5 runs of 5 and its full
  GUI suite passed (24 sessions, 236 checks). In the sessions' 1600x1000 page the panel is
  fitted at 598 x 265 px and the drawing is 325 x 60 px beside the controller. A screenshot
  of the card showed it there, with the lists below.

- **A device's named signal inputs: the rear jacks fed from outside** (2026-09-29; the
  addition to the DAW the owner chose; its form the agent's, not yet seen by the owner). A
  device gets a field `inputs`, by input name: `{track, tap, channel}` (one channel of
  another track's audio, after its devices or after its fader, as a side chain is) or
  `{port, channel}` (a channel of a logical audio or CV input port; 1.0 is 10 V at a
  DC-coupled channel). A patch's named inputs are its top-level `io.inlet` nodes (the names
  `patch.describe` already lists among its `inputs`): those the device's `inputs` feed are
  kept when it compiles, the others dropped with their connections, so what they would feed
  keeps its own default; for the Minimoog, an empty jack. The `minimoog` preset now has
  four: `osc_cv`, `filter_cv`, `loudness_cv`, `s_trig`. The engine carries a track source
  as a side chain's input (the source first whatever the order, a loop an error, aligned to
  the listening track's input as the side chain is, not to the device's place in its chain)
  and reads a port's channel from the block's inputs (not compensated for the interface's
  input latency, as the `cv-in` device is not; a channel not on this machine is silent,
  with a warning). `Device::inlet_names` and `Device::inlet` in the engine; set with `set
  <device>/inputs/<name> {…}` (the bus, the CLI and MCP). Checked: an engine test (one
  channel of a track before and after its fader, an input the device lacks, a loop, a
  port's channel present and absent), the Minimoog through the API (+3 V at the filter's
  jack opens the filter, 0 V at EXT. LOUDNESS shuts the VCA, unplugged again the same
  render to the bit), the CLI and MCP, the real-time safety test playing a Minimoog with a
  jack fed. A patch made from the preset before this has no inlets (projects keep their own
  copy): its device must be added again, or the inlets added to its patch, to use the
  jacks. This extends the DAW's design, which had CV input only as a modulation source.
- **Devices report indicators; the OVERLOAD lamp lit** (2026-09-29; the agent's decisions,
  not yet seen by the owner). A device may now report up to eight indicator levels, 0 to 1
  (a lamp, a meter): `Device::indicator_names` and `indicators` in the engine, the same on
  a patch module. The live transport publishes every device's each period (up to 64
  devices), apart from the voice watch, and `transport.indicators {device}` reads them
  (from the CLI and MCP too; `transport.voices` carries them as well). The Minimoog reports
  `overload`: the lamp's current over its full brightness's, its peak held and falling with
  a 50 ms time constant so that a reader polling now and then sees a flash (the lamp's
  thermal lag is not modelled, A27; the samples are unchanged). The panel reads it ten
  times a second while it shows and lights the jewel by it. Checked from the API, the CLI,
  MCP and the GUI: lit at full EXTERNAL INPUT VOLUME with a tone at the input, out at 0.
- **The VCA's fast attacks corrected: a click at every fast attack removed** (2026-09-29;
  the agent's decision, not yet seen by the owner). Extending the thump's check to fast
  attacks (the contour rising in 1 and 0.3 ms, as short ATTACK settings make it) showed the
  model off the circuit by 446 and 2125 mV against its 118 mV thump: its slow bias,
  interpolated a block behind, left the tails' two Newton steps far from convergence. In
  the voice that was a click at the start of every fast attack (0.125 of full scale in one
  sample on the bass line). Now the bias is solved at once whenever the contour has moved
  2 mV since its last solve, and taken whole, and the tails' Newton steps run to
  convergence: within 1.3 mV at every rise. Renders change at attacks and releases (up to
  0.2 of full scale in an attack's first samples, the same again 20 ms later); the golden
  renders re-recorded; 2 % slower. **This moves the No Compromises reference**: "within 1e-7 of
  today's model" now means of the corrected model. What it cost: EXT. LOUDNESS's tremolo
  at 1 kHz was 35.5 dB below the circuit's output and is now 27.9 dB (5 to 100 Hz
  unchanged, 36.7 to 41.2 dB). The old figure was an accident, the same lagging tails
  cancelling part of the model's step error: ngspice without the transistors' charges gives
  the same figures, and the model oversampled converges to 35.2 dB at 8 times
  (numerics.md, "Real-time VCAs"; `tremolo_step_budget`). That test's budget at 1 kHz is
  now 27.5 dB, the rest still 33. Open: find the VCA's first-order step error and remove
  it (part of No Compromises).
- **A bug found on the way: an instrument's side chain never arrived.** A patch device's
  `io.side` was fed only on effect chains; on an instrument (the Minimoog, whose EXTERNAL
  INPUT is the side chain, voice.md) it stayed silent. Each sounding instance now gets the
  side chain's block (the DAW's patch device); a test plays a tone track into the
  Minimoog's external input with its oscillators off and hears the tone's pitch (219.98 Hz
  against 219.97 Hz alone, 38 dB above the input switched off).

## The worker locks made before the audio thread takes them (macOS)
**Agent decision, 2026-09-30** (a fault the merge showed; for the owner's review). The
first macOS build of the merged main line (the Mac's work merged into the Linux machine's)
failed the DAW's real-time safety test: 5 allocations on audio threads with the Minimoog
playing, none on Linux. Each is the first `try_lock` of a Minimoog worker's `Mutex`: on
macOS the standard library makes a mutex's pthread lock on first use (64 bytes), where
Linux's futex needs none. Each worker now takes its lock once when it is made, off the
audio thread. The test passes on macOS (0 allocations in every mode); nothing changes on
Linux.

The Minimoog's own allocation test (`crates/ca72/tests/rt_alloc.rs`) failed on macOS too,
not by the voice: it counted every thread while armed, and the test harness's thread,
printing the file's other test's result, allocated meanwhile. Its one-thread test now
counts its own thread only; the threaded test still counts every thread (its workers').

## The Minimoog panel's font bundled: TeX Gyre Adventor
**Owner decision, 2026-09-30** (choosing the agent's recommendation among three, with its
download approved). The merged main line's first macOS GUI run failed the Minimoog panel's
text-clearance check: its font list (`--font-mm`: URW Gothic, Avant Garde Gothic, Century
Gothic, Futura) gave URW Gothic on Linux, where the panel was drawn and checked, and
Futura on the Mac, whose wider letters made "KEYBOARD CONTROL" touch "32'" (-1.6 units).

- **Now:** TeX Gyre Adventor 2.501 (GUST e-foundry; GUST Font License, the LaTeX Project
  Public License 1.3c or later), regular and bold, vendored in
  `third_party/tex-gyre-adventor` (`VENDORED.md`: source, version, hash) and copied into
  the built page; `--font-mm` is `'TeX Gyre Adventor', 'URW Gothic', sans-serif`. It is
  based on URW Gothic L (its bold on URW Gothic's Demi, the panel's weight 600), so the
  panel keeps its letters on Linux and gets them on every other platform.
- **Why not URW Gothic itself:** its current release is AGPL-3.0-only (its font exception
  covers PostScript and PDF only), while the DAW's licence was still open.
- **Evidence (the Mac, 2026-09-30):** the DAW's Minimoog GUI session passes: every
  legend clears its tick by 6 units or more (the closest 6.7), no two of the panel's 227
  texts touch. Linux checks it when this main line is fetched there.

## A circuit model's budget is its own: the Minimoog's default mode within 60 % of a block
**Owner decision, 2026-09-30** (choosing the agent's recommendation among three). The
merged main's first full CI on the Mac failed the benchmark gate on one device: the
Minimoog, timed like every first-party instrument (eight voices of chords on
128-frame blocks at 48 kHz), took 1.31 ms at p99 in its default quality mode, No
Compromises, against the instruments' 0.40 ms (15 % of the block, the DAW's rule for its
instruments). Its other modes, measured the same way: High Fidelity 1.18 ms, Potato 0.16
ms. The Linux reference machine had not timed it yet (its results predate the device).

- **Now:** a circuit model is timed twice. In its default mode it is
  `device/circuit/<name>`, budgeted at 1.6 ms p99 (60 % of the block) on every reference
  machine; in its cheapest mode it is `device/instrument/<name>-<mode>` (the Minimoog's
  `minimoog-potato`), under the instruments' budget like the rest. Every other instrument
  keeps the 15 % rule. A list in the DAW's benchmarks names them.
- **On the Mac (2026-09-30):** `device/circuit/minimoog` 1.31 ms, `device/instrument/minimoog-potato`
  0.16 ms. The failed run's own records (under the old name) were not kept.

## The Minimoog's panel as the plug-in's: its printed ticks corrected, softer knobs, no cheeks, the left hand controller beside the panel
**Owner decision, 2026-10-01.** The owner asked for the circuit-derived Minimoog to be
released as an open-source plug-in, in a repository of its own (not the DAW's), and for
its panel to be fixed there: the owner found the ticks misplaced, the knobs' reflections
distracting and the wood ending short. From the agent's mock-ups the owner chose the
layout with the left hand controller in a column beside the panel, and then asked for the
DAW's own panel to be fixed to look the same. The DAW's GUI now draws that panel.

The model, its circuit lab, the netlists and the derivation documents were copied into
this repository (the plug-in CA-72, by Idle Foundry, GPL-3.0-or-later). The DAW keeps its
own copy: nothing in it depends on this repository, and changes do not pass between them
unless copied.

- **The ticks, measured again** against the face-on photograph (Commons, "Minimoog
  panel.jpg") and a second one, each tick found as a blob about the drawn centre:
  - FREQUENCY (oscillators 2 and 3): a tick a semitone, 20 degrees apart, -7 at -140 and
    7 at 140 (they were spread evenly over 150 each way), and an end mark past each end at
    156 (not drawn before); every number on its tick.
  - ATTACK and DECAY TIME: 13 marks, at -150, -120, -105, -90, -60, -30, 0, 30, 49, 67, 86,
    108 and 148. There were 14: a mark at -15 the photograph does not have (a knob's
    highlight read as a tick), and the seconds' at 50, 70, 90, 110 and 150. "5" and "10"
    are on their marks.
  - RANGE and WAVEFORM: six positions 33.5 degrees apart (16.75, 50.25 and 83.75 each
    way; they were 32 apart).
  - The 0 to 10 dials, TUNE and CUTOFF were right: eleven ticks, 30 degrees apart.
- **The knobs:** the aluminium caps are flat, lightly shaded from the upper left, their
  two wedges at 16 % rather than 55 %; the highlight on the black skirts is a third as
  strong.
- **The wood:** no cheeks. The top strip and the name board run the whole width, their
  ends square.
- **The left hand controller** stands in a wooden column, 330 units wide, at the panel's
  left and as tall as it: GLIDE and DECAY at its top, the wheels below at 1.75 times their
  size. With no row under the name board, the drawing is 3438 by 1057 units (it was 3252
  by 1441).
- **The rear panel's drawing is back above the jacks' lists**, under the panel, as it was
  before 2026-09-30 put it beside the controller (above, "The rear panel beside the left
  hand controller"): with the controller in the column, there is no space beside it. The
  panel's `hold` and the rear panel's `home` are gone; the drawing stays in the rear
  panel's section. This is the agent's consequence of the owner's choice, not separately
  approved.
- **The DAW's own parts stay:** the "minimoog" plate, QUALITY on the name board, the rear
  panel and MIDI BEND. The plug-in's plate, its name and its maker are this repository's,
  not the DAW's.
- **Not changed: FREQUENCY's value.** The parameter is -7 to 7 over the knob's whole
  travel, while the dial prints 7 at 140 degrees (so 7.5 at the ends). The sound follows
  the knob's position and is unaffected, but at the printed 7 the value shown is 6.53.
  Making the range -7.5 to 7.5 (the printed units, as TUNE's are) would move the
  catalogue's Minimoog presets slightly, so it waits for the owner.

**Evidence (the Mac, 2026-10-01):** the DAW's GUI type-checks, and its Minimoog GUI
session passes. Its new check: every knob's 20 dials have
the photograph's ticks (FREQUENCY's 17, the TIME dials' 13, the rest 11), with the
selectors' pointers at their new angles. The rest of its checks pass unchanged, among
them: every legend clears its tick by 6.7 units or more; no two of the 227 texts touch
(the nearest 2.2); the rear drawing is under the panel, above the lists, with either
view. In the sessions' 1600 by 1000 page the panel is fitted at 863 by 265 px (598 by
265 before). Only that session draws the panel.
