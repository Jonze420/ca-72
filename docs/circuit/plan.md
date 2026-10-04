# Plan: a circuit-derived Minimoog Model D

The owner's request of 2026-09-28: a complete, playable software Minimoog Model D of about
1972-73, derived from its circuit, component models and calibration documents, not
sampled and not voiced by ear. Decisions are recorded in `docs/history.md`.

## Three concerns, kept apart

| Concern | Where | What |
|---|---|---|
| A. Circuit specification | `docs/circuit/`, `circuits/boards/`, `circuits/models/` | What is modelled: sources, revision manifest, transcribed netlists, device models with provenance |
| B. Offline reference | `crates/ca72-spice`, `crates/ca72-lab` (`ca72-lab`) | ngspice 47 runs of the netlists: operating points, calibration, waveforms, reference data |
| C. Real-time implementation | `crates/ca72`, then modules in the DAW's patch library | Models derived from the circuit, tested against B; the instrument as a device of the DAW it was developed in |

Why ngspice for B: a mature, open, batch-scriptable circuit simulator with Gummel-Poon
BJTs, JFETs and diodes, adaptive time steps and convergence aids; independent of our own
code, so agreement between B and C means something. Its cost (seconds per note) rules it
out for C. Our own nodal solver may later serve C for stages where a reduced model is not
accurate enough; ngspice stays the reference.

## Stages

Each stage ends with tests that pass and its evidence recorded (in the DAW's reports).

0. **Sources and specification** (started): source index, revision manifest, board 1
   transcription. *Accept when* every board 1 part traces to a drawing and the parts list,
   and the open items are listed. **Done for board 1's one oscillator.**
1. **Board 1 reference**: electrical plausibility against the drawing's annotated voltages;
   both factory tuning procedures pass; tracking over every key and range measured; the
   solver's tolerance, time step and method shown not to change the results; waveforms
   stored as reference data. *Accept when* the tests run on the Linux reference machine.
   **Done for oscillator 1** (2026-09-28).
2. **Real-time oscillator**: a reduced model derived from the board 1 circuit (the
   exponential converter's static and dynamic behaviour, the sawtooth core with its reset,
   the shapers as functions of the buffer's voltage), band-limited. *Accept when* after the
   same factory calibration it agrees with ngspice within 1 cent on every key and range,
   its waveforms agree within stated limits, and aliasing is below a stated floor.
   **Done for oscillator 1** (2026-09-28): 0.088 cent on 32'..2', harmonics within 1 %,
   aliasing -70 dB at 4 kHz (board1.md). **Oscillators 2 and 3 too** (2026-09-28): their
   FREQUENCY controls, OSC. 3 CONTROL, IC8 and the reverse sawtooth transcribed, the
   manual's ranges met in ngspice, the real-time models within 0.035 cent (0.29 with OSC.
   3 CONTROL off) below 4.5 kHz.
3. **Board 4 reference and real-time model**: mixer inputs, filter (input pair, ladder,
   gain recovery amplifier, emphasis), VCAs and output. *Accept when* cutoff tracking,
   resonance, self-oscillation and level-dependent distortion agree with ngspice.
   **The filter's ladder, coupling networks, output pair and emphasis done** (2026-09-28;
   board4.md): small-signal within 0.25 dB to 10 kHz, self-oscillation within 3 cents and
   0.04 dB, drive distortion within 0.33 dB. The exponential converter and control node
   too: within 0.1 cent over CUTOFF and temperature. The VCAs and output stage too
   (tails, balance procedure, gain, thump, distortion). The mixer's loading remains.
4. **Board 2**: contour generators (attack, decay, sustain, the DECAY switch) and the
   keyboard's track-and-hold with glide. **Contour generators done** (2026-09-28;
   board2.md): within 27..72 mV of ngspice over five scenarios, peaks within 11 mV,
   trigger edges within 0.15 ms. **The keyboard circuit too** (2026-09-28): solved as its
   netlist; every key's output within 0.1 uV of ngspice, glides within 0.2 mV, slews with
   GLIDE off within 2 us.
5. **Board 3 and the controllers**: noise, modulation mix, oscillator 3 as a modulator,
   pitch and modulation wheels; the rails' regulators if they matter (A1). **Done**
   (2026-09-28; board3.md): the noise generator's small-signal model within 0.001 dB of
   ngspice and its spectrum within 0.42 dB; the modulation mix's transfer to four digits,
   its line within 1.7 mV (17 mV at the clip, A24); on the voice the manual's 5.19, 5.27,
   5.35, 5.36 and 5.37 met. The regulators transcribed and measured: under 48 milliohm and
   78 dB of ripple rejection, so the rails stay ideal (A1). The headphone amplifier is left
   out (the owner's direction).
6. **The instrument in the DAW**: one of the DAW's patch devices, of circuit-section
   modules; a monophonic voice mode (lowest-note priority, single triggering), MIDI mapping
   of keys, wheels and panel controls, reachable from the command bus, CLI and MCP; renders
   of one-, two- and three-oscillator patches. **Voice v0** (2026-09-28; voice.md): the
   keyboard circuit, the three oscillators, the mixer's bus, the filter with its control
   node, the VCA and the contours wired as the instrument, played from patch files by
   `ca72-lab play`; tested for its first trigger, pitch, release, lowest-note priority,
   single triggering and GLIDE. Board 3 and the wheels joined it (2026-09-28), with timed
   moves of any control in a patch. **The external input, the A-440 and the filter's
   factory calibration** (2026-09-29; board4.md). **The patch device** (2026-09-29;
   voice.md, "In the DAW"): `minimoog`, 43 parameters, reachable from the bus, CLI and MCP,
   one keyboard-taking instance (lowest-note priority), the side chain as the external
   input. **The panel** (the owner's request, 2026-09-29): a 1:1 recreation of the Model
   D's front panel and left hand controller as its editor, every control wired to its
   parameter, adjudicated by a critic agent: done (five rounds, the critic's verdict MATCH;
   left out: the Moog trademark on the name board, the owner's call).
7. **Validation and performance**: the owner's recordings (qualitative); numerical and
   aliasing studies; benchmarks (after the owner has checked the sound). **Performance**
   (2026-09-29, at the owner's request): two passes, 3.8 times faster within 2.6e-8 of the
   unoptimised renders (a first pass 1.6 times, a second from the profile); the owner's 100
   times not reached and not reachable without changing the output (numerics.md,
   "Performance").
8. **Quality modes** (the owner's direction, 2026-09-29; history.md, "Three quality
   modes"): No Compromises (within 1e-7 of full scale; one instance in real time at
   256-frame blocks in the worst case, the external input and CV included), High Fidelity
   (the difference below -120 dBFS, pitch within 0.01 cent), Potato (a hundred fully
   automated instances). First: the owner's comparison with recordings, a worst-case
   benchmark, the CV inputs, S-TRIG and the OVERLOAD lamp. Done (2026-09-29): the
   benchmark (`ca72-lab worst`), the rear jacks in the voice and the device, the OVERLOAD
   lamp reported and lit on the panel, the jacks fed from outside the device (a track or
   an input port's channel, the device's `inputs`).

## Tooling

- `ca72-lab calibrate [folkman|norlin]`: runs a factory tuning procedure on board 1's
  oscillator in ngspice.
- `ca72-lab play <patch.json> <out.wav>`: the voice plays a patch's notes (voice.md).
- `scripts/fetch-sources.sh`: the sources, into a local cache.
- ngspice 47 built in user space (`~/.local/opt/ngspice-47`, docs/circuit/README.md).
