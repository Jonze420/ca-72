# The voice

`crates/ca72/src/voice.rs` wires together the real-time models tested so far, the
way the instrument wires them. It is how the models are first heard together. It is not
yet the whole instrument (see "Not yet in the voice"). The patch device of the DAW the
model was developed in comes later (plan stage 6).

## Version 0 (v0)

| Part | Model | Rate |
|---|---|---|
| Keyboard | Board 2's keyboard circuit (board2.md), solved as its netlist: 44 keys, F to C (MIDI 41 to 84). The lowest and highest keys held join the pitch bus to the string, so the lowest sounds (lowest-note priority). The trigger contact stays closed while any key is held, so a legato key does not retrigger (single triggering). Then the hold, GLIDE and the output's load (the three oscillators' keyboard inputs to -5 V and the filter's to its control node, A19). A released key's pitch contact opens 2 ms after its trigger contact (A18) | per sample while it moves, else in blocks |
| Oscillators 1, 2, 3 | board1.md, each tuned by Folkman's 1973 procedure on the model when the voice is built (oscillators 2 and 3 with FREQUENCY centred and OSC. 3 CONTROL on), their keys played through the keyboard circuit. Oscillator 2's FREQUENCY on IC4's + input; oscillator 3's FREQUENCY through its control stage IC8, OSC. 3 CONTROL switching its inputs, and its reverse sawtooth (Q37) solved as its circuit | 4x oversampled; Q37 at the output rate while heard |
| WAVEFORM switches | dwg 1448: the sawtooth, triangle and rectangle outputs as Thevenin sources (board 1's output dividers). The second position is the shark tooth on oscillators 1 and 2 (sawtooth and triangle through R030 47K and R031 10K) and the reverse sawtooth on oscillator 3, whose sawtooth R171 loads; the rectangle's width comes from the switch's second pole (0, -1.5 and -2.5 V) | |
| Mixer | Each oscillator's VOLUME (25K linear) and 33K into the filter's input bus, as a Norton source, and the noise's (25K linear and R48 11K); switched off, a channel's series resistor only loads the bus, as does the external input's (33K) | |
| Noise | board3.md: the noise generator's small-signal model, its outputs loaded as the NOISE switch (and R50 at PINK), the mixer's VOLUME and MODULATION MIX load them; its level the factory's calibration, done when the voice is built (A23) | 4x oversampled |
| Modulation | board3.md: MODULATION MIX (oscillator 3's switch output and the pink or red noise), the modulation mix amplifier's transfer, R57 and the MODULATION wheel; OSCILLATOR MODULATION puts the line on the oscillators' MOD bus, FILTER MODULATION on the filter's control node through R52 33K (off, each is grounded, A21). Oscillator 3's switch output is loaded by its mixer channel and R23 | output rate; oscillator 3 a sample behind |
| Wheels | The pitch wheel on the oscillators' pitch bus (+6.12 V +-2 V, A22); the MODULATION wheel's resistance on the line (A22) | |
| External input | board4.md: EXTERNAL INPUT VOLUME (R9, 1M audio) ahead of the preamplifier (circuit No. 12, solved as its circuit), SW10 and R46 33K onto the bus; the OVERLOAD lamp's driver (`Voice::overload`). The jack's voltage is the input sample times 5 V (A25) | the preamplifier at 4x (trapezoidal), while SW10 is on |
| A-440 | board4.md: its circuit's waveform (ngspice) at 440 Hz into the VCA's R40 and C8 (A26, B4-7) | output rate |
| TUNE | From its centre (where the factory's tuning leaves it), R1's wiper into each oscillator | |
| Filter | board4.md, calibrated by Folkman's procedure (`filter_cal.rs`: R39, R49, R73). The control node: CUTOFF (5K linear across ±10 V) through R55 200K, KEYBOARD CONTROL 1 and 2 through R53 300K and R54 150K, AMOUNT OF CONTOUR (5K linear from the filter contour) through R74 47K, the modulation line through R52 33K, and the external control's R51 100K (its jack empty: grounded). EMPHASIS is R14, a 50K reverse-audio rheostat | 4x oversampled |
| VCA | board4.md, balanced by the factory procedure, on the loudness contour | output rate (bias per sample and at 3 kHz) |
| Contours | board2.md, on the front panel's ATTACK and DECAY (1M audio rheostats) and SUSTAIN (5K linear), with the DECAY switch | half the output rate, interpolated (A17) |

The pots' tapers are generic audio and reverse-audio laws (A9; GLIDE is "R2 5M AUDIO" on
Figure 9-17). The panel's knobs run 0..1 for its 0..10 (CUTOFF's -5..+5 as 0..1); GLIDE's switch
on the left hand controller is `glide_on`.

**Output scale.** `Voice::tick` returns the main output's voltage across a 10K load, with
5 V as 1.0. The scale is fixed, not set per patch, so levels compare between patches and
with a recording once one level is matched. A full-volume sawtooth peaks near 1.4 V
(0.28).

**Trigger delay.** A key's contours start about 8 ms after the press. This is the
circuit's (board2.md, B2-3): C7 rests near -10 V and must charge past Q20's threshold
before the trigger fires. A unit whose Q20 emitter-base junction breaks down would rest
higher and fire sooner. This is a candidate for the owner's comparison.

## Tests

`crates/ca72/tests/voice.rs` (no ngspice needed):

- `a_key_sounds_at_its_pitch_and_releases`: the output is silent before a key
  (1.6e-9 RMS). The attack starts 8.1 ms after the press (the circuit's trigger delay).
  The low A on 8' is 110.008 Hz (+0.13 cent). The release's tail keeps the pitch
  (+0.3 cent). 0.6 s after the release, with LOUDNESS DECAY at 1.5, the level is 5.3e-4
  against 0.114 held. This test caught the missed first trigger: the contours at 12 kHz
  with Q20's and Q12's solves limited to 8 Newton steps left the first note silent.
  Reinstating that makes it fail.
- `the_lowest_key_sounds_and_contours_are_not_retriggered`: A3 at 220.01 Hz, then A2
  held under it at 110.08 Hz. E3 added above changes nothing (110.07 Hz). A2 released
  leaves E3 at 164.92 Hz. With two keys held the lower is about 1.1 cents sharp: the
  string's current through the two contacts (A18). The loudness contour stays at its
  sustain (4.480 V) throughout. A note outside the keyboard is ignored.
- `glide_moves_the_keyboard_voltage_between_keys`: legato from A2 to A3. With GLIDE off
  the keyboard voltage arrives within 1 ms of A2's pitch contact opening. At GLIDE 5 (500K)
  it rises 0.49, 0.85, 1.34 V at 10, 30 and 60 ms, and arrives.

- `oscillators_2_and_3_play_through_their_controls`: oscillator 2 alone plays the low A at
  109.999 Hz (-0.02 cent), +8.23 semitones with FREQUENCY at 10; oscillator 3 with OSC. 3
  CONTROL off holds its pitch whatever the key (232.68 Hz under A2 and A3); its reverse
  sawtooth reaches the mixer inverted against its sawtooth (correlation -1.000).

- `modulation_and_the_wheels_meet_the_service_manual`: with oscillator 3's low square
  on the MOD bus and the wheel fully forward, oscillator 1 swings 18.0 semitones (5.37: 13
  to 23); on the filter the corner rises 33.8 times (5.19: at least 5.45); the pitch
  wheel's travel is 16.1 semitones (5.35: 13 to 17).
- `the_noise_sits_under_the_triangle_as_the_factory_set_it`: at the output, both channels
  at VOLUME 4, white noise -6.39 dB and pink -6.02 dB against oscillator 1's triangle
  (5.8 and 5.27: -6, each within 3 dB). The pink tilts 15.1 dB over five octaves, the
  white -0.5.
- `oscillator_3s_frequency_and_wide_range_meet_the_service_manual`: FREQUENCY spans 16.8
  semitones (5.35: 14 to 17); on LO with OSC. 3 CONTROL off it clicks every 4.95 s at its
  minimum (5.36: 2 to 5 s) and overlaps 32' at its maximum.

- `the_a440_and_the_external_input_meet_the_service_manual`: the A-440 at 440.00 Hz and
  -5.3 dB (5.7: -8 +- 2, B4-7); with -30 dB at 1 kHz into the external input the OVERLOAD
  lamp lights at VOLUME 9.5 while the output's distortion is the filter's soft overdrive
  (4.4 %), before the preamplifier clips (5.26).
- `filter_cal.rs`: Folkman's filter procedure run again against the committed trims, and
  the service manual's 5.13, 5.15 and 5.18 on the calibrated filter.

Every voice test also fails on a failed solve in the keyboard circuit or the reverse
sawtooth stage.

The keyboard circuit, the contour generators and each other part are tested against
ngspice in their own tests (`keyboard_realtime.rs`, `contour_realtime.rs`, ...).

## Rendering

```
cargo run -p ca72-lab --release --bin ca72-lab -- play crates/ca72-lab/patches/v0-bass.json v0-bass.wav
```

A patch is JSON: `{"panel": {...}, "notes": [[start s, length s, MIDI note], ...],
"moves": [[time s, key, value], ...], "seconds": total}`. The panel's keys are `Panel`'s
fields: the contour knobs as `filter_attack` .. `loudness_sustain`, `glide` and
`glide_on`, `osc3_control`, the noise as `noise_on`, `noise_volume` and `noise` (`"white"`
or `"pink"`), `mod_mix`, `osc_mod`, `filter_mod`, `pitch_wheel` (-1..1, 0 its detent),
`mod_wheel`, `ext_on`, `ext_volume`, `a440` and `tune` (-1..1), and each oscillator's as `osc1_range` .. `osc3_freq`: a range is `"lo"`,
`"32"` .. `"2"`, a waveform `"triangle"`, `"sharktooth"` (oscillators 1 and 2),
`"reverse"` (oscillator 3), `"sawtooth"`, `"square"`, `"wide"` or `"narrow"`. Knobs run
0..1. `"input": {"hz", "amp"}` puts a sine at the EXTERNAL INPUT jack (as a fraction of
its 5 V). A key left out stays at its default. A move sets any key at its time (the wheels,
a sweep, a switch). `ca72-lab` refuses an unknown key, a wrong type, a knob out of its
range or a malformed note rather than playing a default. The output is a mono 32-bit
float WAV at 48 kHz on the scale above.

`patches/v0-board3.json` plays board 3: the pitch wheel up and back (A), oscillator 3 on
LO as vibrato brought in by the MODULATION wheel (B), the same on the filter (C), then
noise in the mixer and on the filter, and pink noise alone (D).

Cost on the Linux reference machine (release build): building the voice takes about 0.6 s
(the keyboard circuit's settled output for the tuning's keys, three oscillators' factory
tuning, the filter converter's table, the VCA's balance, the noise's calibration, the
modulation mix's table); the DAW device builds it once per sample rate and copies it
(`Voice::prototype`). Rendering runs at 0.40 (idle) to 0.74 times real time depending on
the patch, 1.5 with the external input on (its preamplifier is the dearest part; release
build, the same machine): numerics.md, "Performance", has the profile, what was optimised
and why it stops at 3.8 times.

## In the DAW

The catalogue device `minimoog` (in the DAW's patch library: module `synth.minimoog`,
catalogue entry and two variants, Bass and Lead) is the voice with 43 parameters in the
panel's own units (knobs 0..10, CUTOFF -5..+5, TUNE -2.5..+2.5 as its dial is printed,
FREQUENCY -7..+7, the wheels, every switch; ATTACK and DECAY as their knobs' positions
0..10, not the times their dials print), plus MAIN OUTPUT VOLUME (R20, 5K audio, into the
output's 10K load) and its switch. The patch runtime's keyboard modules
(`Module::keyboard`, history.md) give it one instance that takes every note, so the
keyboard circuit gives lowest-note priority and single triggering as on the instrument. The
side chain is the EXTERNAL INPUT. Each device seeds its own noise. The device reports the
OVERLOAD lamp as an indicator, `overload` (0 dark, 1 fully lit; its peak held and falling
over 50 ms): `transport.indicators` on the bus, the DAW's CLI and MCP tools, and the
panel's jewel.

In the GUI the device's card shows a recreation of the Model D's front panel and left hand
controller: knobs turn by vertical drag (Shift for fine, the mouse wheel to nudge, a double
click for the default), RANGE and WAVEFORM step by click or drag, a switch flips at a click
anywhere on it (the owner's direction, 2026-09-30; it was pressed at the end clicked), the
PITCH wheel settles into its centre detent when let go near it (no spring, as on the
instrument). The whole instrument is fitted in the pane, with a Zoom button to its legible
size; a button in the card's head switches to the generic parameter view. A scripted
session of the DAW's GUI operates every one of the 43 parameters from its control and
checks through the API that it alone changed.

Reachable as every patch device is: the bus (`patch.from_preset {preset: "minimoog"}`,
`device.add`, `set <device>/<param>`), the DAW's CLI (adding the device to a track from its
preset, setting, listing its parameters and the library's devices) and MCP. A test of the
DAW's API adds, sets and renders it through the bus and checks lowest-note priority (A2
alone 110.01 Hz, with A3 held 110.08 Hz, A3 alone 220.02 Hz). It allocates nothing on the
audio thread: the DAW's real-time safety test plays it with an LFO on its cutoff and noise
volume in every threading mode, and `ca72/tests/rt_alloc.rs` counts the voice's own
allocations.

**The rear jacks** (2026-09-29; A5). `Voice::tick_jacks` takes, each sample, the EXTERNAL
INPUT and the four rear jacks (`Jacks`: `None` an empty jack, its normal contact in
place): the oscillators' control input holds their external bus (8A; the circuit's scale
0.984 octaves a volt, within 0.14 cent of ngspice from -4 to +4 V, `expo_realtime.rs`), the
filter's feeds the control node through R51 100K (within 0.01 cent of ngspice from -8 to +4
V, 0.49 cent at +8 V where Q28 saturates, `vcf_control.rs`), EXT. LOUDNESS drives Q21's
tail through J3 (the gain within 0.09 dB of ngspice from 1 to 6 V; its useful range is
about 0 to 5.5 V, the gain peaking at +2 dB at 5 V; from about 6 V an ideal source
overdrives Q21 and cuts the second pair off, shutting the VCA by about 6.6 V, as the circuit
does: tails within 0.1 % and gain within 0.06 dB of ngspice through it, to 9 V; a tremolo at 5 to 100 Hz within
36.7 dB of the circuit and at 1 kHz within 27.9 dB, the VCA's step, with its bias solved
every sample, which the voice does while the jack is plugged, `vca_realtime.rs`), and EXT. S-TRIG shorted triggers the contours
(edges within 0.04 ms and contours within 32 mV of ngspice, alone or pulsed during a held
key, `contour_realtime.rs`). In the DAW the device's module takes them as inputs
`osc_cv`, `filter_cv`, `loudness_cv` (1.0 = 10 V) and `s_trig` (closed above 0.5); nothing
connected is an empty jack. The samples with every jack empty are the same as before. The
`minimoog` preset makes them the device's named inputs, fed through its `inputs` field from
a track's audio or an input port's channel, sample by sample: `set <device>/inputs/filter_cv
'{"track": "tr_…"}'`, or `{"port": "pp_…", "channel": 3}` for a DC-coupled interface
input (history.md, "A device's named signal inputs").

**On five threads** (2026-09-29, for playing live). The voice is six parts (`voice.rs`):
the keys' contacts, the keyboard circuit, the control part (the contour generators and
the noise source), and the audio path's three: the external input's preamplifier (with
the OVERLOAD lamp's driver), the front (the modulation line, the oscillators, the mixer,
the filter's control node) and the back (the filter, the VCA, the A-440). The keyboard
circuit, the control part and the preamplifier depend only on the keys, the panel and the
external input, so in the DAW each runs on a thread of its own a few samples ahead
(`ca72/src/threaded.rs`); the front runs on the caller's thread, taking their
outputs sample by sample as they come, and the back on a thread of its own a few samples
behind it, taking the front's. Each part does what `Voice::tick_jacks` has it do, in the
same order, so the samples are the same to the bit (`tests/threaded.rs`, over notes,
GLIDE, panel changes, the external input, the A-440 and the rear jacks, in blocks of 1 to
513 samples). What changes is a period's worst case, which is the slowest part's instead
of the parts' sum (numerics.md, "No Compromises"). The workers are audio threads
(real-time priority where the system allows, held to the no-allocation rule by the
Minimoog's and the DAW's tests); they spin 0.3 ms for the next block and then park. Safe
Rust: the parts behind mutexes the audio thread only tries, the outputs, and the back's
inputs, through atomics.

## Not yet in the voice

- **The headphone amplifier** (not to be modelled: the owner's direction, 2026-09-28).
- **The mixer's loading on the oscillators' outputs** (the channels load the bus only).
- **Velocity**: the Model D has none; a note's velocity is ignored.
- The rails are ideal (A1; board3.md measures why).
