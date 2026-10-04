# Board 1: the oscillator board ("old" board, serial numbers below 10175)

Netlist: [`circuits/boards/board1-vco.lib`](../../circuits/boards/board1-vco.lib) (one
oscillator; oscillator 1's reference designators). Sources are listed in
[sources.md](sources.md) by their tags (S-F93 and so on).

## Which drawing

Two drawings document this board's circuit:

| Tag | Drawing | Date | Expo arrays | Q7/Q18/Q31 | R69/R105/R141 | R78/R106/R128 | C3/C5/C7 |
|---|---|---|---|---|---|---|---|
| S-F93 | Service manual Figure 9-3, "Oscillator printed circuit board No. 1 schematic diagram (serial numbers below 10175)" | undated (manual c. 1978-80) | "3046" | E402 | 15K | 2K plus 220 ohm and .01 in series across it ("resistance capacitance network", Table 7-5) | 100pf |
| S-08001 | Moog dwg 08-001 "Mini D oscillator PC brd assy schematic" (Williamsville, N.Y.) | 4-10-72 | SG3821 | hand-lettered, unclear | 6.8K | 2K alone | 47pf |

Both are the second revision of the board (CA3046 or SG3821, Silicon General's drop-in; board
photos show an SG3821N dated 7337 and a CA3046 dated 1977). S-F93 is S-08001 after factory
kit 94-001 ("to improve tracking and pitch stability", prescribed in Folkman's field service
manual of July 1973 for boards above serial 1300): R69/R105/R141 6.8K to 15K, the RC network
across R78/R106/R128, C3/C5/C7 47 to 100 pF, R181 56K to 51K, R170 and R162 to 1 %
(manifest.md lists every difference). Table 7-5 (S-T75) agrees with S-F93 on every value
checked; the synthfool copy carries a hand note "SG 3821" beside IC2/IC7. The netlist
follows S-F93; whether the model should be the as-built board is the owner's question Q1.

## How the oscillator works (from the drawing, checked in ngspice)

Voltages are oscillator 1's, measured in the first ngspice runs (2026-09-28) and compared
with the drawing's annotations.

1. **Control summer IC1** (741) is supplied from GND and -10 V, its + input at -5 V (R10 to
   the -5 V line), so its summing junction sits at -5 V. Inputs arrive through R12 150K
   (bend), R21 560K (tune), R27 51.1K (keyboard), R32 51.1K (mod), R38 51.1K (ext), R43 15K
   (range switch). R11 (1K WW rheostat, the rear-panel "range" trimpot of Table 5-3) and
   R37 11.8K draw a fixed current to -10 V. R20 (1K, 1 W, "temperature compensating")
   is the feedback. Keyboard gain: R20/R27 = 19.6 mV per volt at IC1's output.
2. **Divider** R9 33, R8 100 WW (the "scale" trimpot), R19 1K from IC1's output to -5 V:
   0.883..0.971 of it reaches the reference transistor's base, 17.3..19.0 mV per octave
   (kT/q ln 2 is 17.8 mV at 25 C).
3. **Exponential converter**, IC2 (3046): Q1 (pins 1, 2, 3) is the reference transistor,
   its base on R8's wiper; Q2 (pins 5, 4, 3) is the exponential transistor, its base at
   -5 V through R7 100 (C2 100pF between the bases). IC3 (741) holds Q1's collector at
   the "-4V" line (R50 39K to GND: 99 uA reference current) by driving the tail transistor
   Q5 (pins 14, 12, 13; pin 13 is Q5's emitter and the substrate) through R69 15K and the
   R78 network. Q2's collector pulls current out of the timing capacitor C1.
   R42 160K feeds the tail's emitter voltage (which rises with the exponential current)
   back to the summing junction: the drawing's text says it compensates the flattening
   caused by the transistors' base resistance.
4. **Core**: C1 (0.01 uF polystyrene, GND to the ramp node) is discharged linearly by
   Q2; Q10 (2N4402, emitter at GND) resets it to 0 V. The ramp runs 0 to -3.97 V.
5. **Ramp buffer**: a discrete amplifier with an E402 JFET pair (Q7) in, a 2N4058 pair
   (Q9, Q8; tail R45 from +10 V) and a 2N3392 output stage (Q3, R15 2.2K to +10 V);
   feedback R24 10.7K with R25 15K to -10 V and R4 33.2K to +10 V:
   Vbuf = 2.036 Vramp + 3.91 V (drawing: "+4V" to "-4V"; ngspice: +3.96 to -4.17 V).
   Compensation: C3 100pF from the PNP pair's emitters to the output, C* 10pF (etched on
   the board) across R24.
6. **Reset Schmitt trigger** Q5, Q6 (2N4402, emitters at GND; Table 7-5 says 2N3906): Q5's
   base is R30 10.7K from the buffer, R16 33.2K from +10 V, R17 10K from the R6/R26 node.
   Q6 off lets current flow from GND through Q10's base and R41 910, R6 56, R26 1000 to
   -10 V, turning Q10 on. Hand analysis: reset starts at a ramp of -3.84 V and is released
   when the ramp has risen past about -1.6 V (drawing: the R6/R26 node swings -0.7 V /
   -5 V).
7. **Sawtooth** out (9B): R34 4300 from the buffer, R33 4700 to GND: +2.07..-2.18 V
   unloaded.
8. **Triangle**: Q2 (2N3392, R3 10.7K to +10 V, R29 10K to -10 V, R14 150K to -10 V)
   saturates for a positive buffer (its collector follows the base, less a diode) and
   inverts for a negative one; the fold's bottom is -0.45 V at the collector (drawing:
   "+3.3V" / "-0.5V"). Q1 follows it into R28 750 / R23 8200: +1.73..-1.86 V unloaded
   (drawing: "+1.7 to -1.7V").
9. **Rectangular**: Q11 (2N4058, emitter at GND) compares the buffer plus the width bias
   (R40, R1 43K each; R44 390K to -10 V); Q12 (2N3392, emitter at -10 V) switches R13 6200
   against R22 4700 to GND: 0 / -4.30 V unloaded (drawing: "(3.5V)"); R2 910K gives
   hysteresis.

Shared parts of the board (one set for the three oscillators):

- **-5 V reference**: IC9 (741) with R177 10K 1% (GND) and R173 10K 1% (-10 V), C13 27 uF;
  its output drives Q38 (2N3392 emitter follower) through R167 4.7K; Q38's emitter (17A)
  goes through the harness to the range switches and back (19A); the -5 V is sensed at the
  switches through 18A and R178 5.1K.
- **Octave string**: from the switches (20A) through R169 68.1 1% and R168 25 WW (the
  "octave" trimpot) to **-10 V**. Section 2.15's text says "to ground"; the drawing shows
  the -10 V line, and only that works with Q38 (an NPN follower, which can source current
  but not sink it). The drawing is followed (discrepancy B1-2).
- **"-4V" line**: R166 6.2K from -10 V and R174 3.9K to GND, C14 0.12 uF: -3.861 V.
- Oscillator 3's reverse sawtooth: Q37 (2N3392) with R164 330, R165 2.2K, R175 68K, R182
  390K, R176 18K, R172 7.5K (the drawing's label for the collector resistor reads "R175
  7500"; Table 7-5 has R172 7.5K and R175 68K).

## Oscillators 2 and 3

Their cores, converters and shapers are oscillator 1's under their own designators (IC4 and
IC6 for IC1, R62/R60 and R109/R107 for the range and scale trimmers). What differs, from
Figure 9-3 and the front panel's wiring (Figure 9-17), transcribed in
[`board1-osc23.lib`](../../circuits/boards/board1-osc23.lib) and checked in ngspice:

1. **Oscillator 2's FREQUENCY.** The front panel's R4 (5K linear, +10 V to R35 5.1K to GND;
   wiper +5.05..+10 V) reaches IC4's + input (pin 3) through R95 220K; R87 91K pulls that node
   toward -10 V and R94 1K (oscillator 1's R10) holds it near the -5 V line. The pot moves
   IC4's reference by millivolts, and the + input lowers the pitch as it rises, so the knob's
   clockwise end is R35's.
2. **Oscillator 3's input node and OSC. 3 CONTROL.** Its bend, tune, keyboard, modulation
   and external resistors (R110, R120, R129, R136, R144) and its range trimmer chain (R109,
   R108 to -10 V) meet at a node brought out on 16A, apart from IC6's summing junction
   (15A). SW2 on joins the two; off, it moves 16A onto the -5 V line at the range switches,
   where those currents go instead (at the same voltage, so nothing else shifts). Its range
   resistor R179 reaches the summing junction directly. The manual's text for this board
   (2.18): SW2 "interrupts the keyboard, modulation, external, and pitchbend voltage on
   oscillator three and also increases the range" of its control.
3. **Oscillator 3's control stage IC8.** The front panel's R5 (as R4, with R41) drives IC8's
   inverting input through R181 51K (9A) and, SW2 off, R180 15K too (10A). IC8's + input is
   R150 1K from +10 V over R155 3.01K to GND (7.51 V, the pot's centre), R162 3.01K from
   +10 V and R170 15K feedback put its output at -5.0 V with the pot centred, and R143
   51.1K carries its change into IC6's summing junction.
4. **The modulation and external buses' pull-ups.** R163 and R156 33K run from +10 V to the
   MOD and EXT buses: against the three oscillators' 51.1K inputs to -5 V they hold an
   undriven bus at +0.1 V.
5. **Oscillator 3's reverse sawtooth, Q37.** A shunt-feedback inverter on oscillator 3's
   sawtooth output (13B) through R171 33K: R175 68K from collector to base, R182 390K to
   -10 V, emitter at GND; the collector (R165 2.2K from +10 V through R164 330, decoupled
   by C10 2.2 uF) level-shifted by R172 7.5K against R176 18K to -10 V to 7B: gain about
   -1.45, resting at 0 V, -2.13..+2.55 V unloaded. R171 loads the sawtooth all the time.
   The WAVEFORM switch's second position on oscillator 3 is this, not the shark tooth
   (dwg 1448).

The factory tuning (Folkman 1973): FREQUENCY at mid-position and OSC. 3 CONTROL on,
oscillator 1's procedure repeated on each (range trimmer for high A at 3520 Hz, scale for
low A at 440 Hz on 2'); TUNE and the octave trimmer are shared, set on oscillator 1.

Checked against the service manual in ngspice (`vco_osc23.rs`):

| Check | Manual | ngspice |
|---|---|---|
| Oscillator 2's FREQUENCY, its travel | 14-17 semitones (5.35) | 16.70 semitones |
| Oscillator 3's, OSC. 3 CONTROL on | 14-17 semitones | 16.76 semitones |
| OSC. 3 CONTROL off | the keyboard has no effect; a wider range (2.18; the later board's 2.3: +-3 octaves) | 0.000 semitones from A2 to A3; 68.8 semitones of travel |
| CONTROL off, LO, FREQUENCY at minimum | a click every 2 to 5 s (5.36) | 0.205 Hz, 4.9 s: with LO five octaves below 32' (A3; four would give 2.4 s, six 9.7 s) |
| LO's top against 32''s bottom (CONTROL off) | they overlap (5.36) | 10.95 Hz against 6.58 Hz |

The real-time models (`tuning.rs`: `osc_drive`, `osc3_control`; `revsaw.rs`), each
oscillator tuned independently by the same procedure in both:

| Part | Agreement with ngspice |
|---|---|
| The trimmers the procedure leaves | R62 235.6 ohm (ngspice 235.5), R109 142.3 ohm (142.2); R60 and R107 0.1351 (0.1352) |
| Oscillator 2, FREQUENCY at 0, 5, 10; oscillator 3 likewise with CONTROL on (8', 32', 2', low and high A, up to 4.5 kHz) | Within 0.035 cent |
| Oscillator 3 with CONTROL off | Within 0.29 cent (untuned there: the op-amp macromodel's offsets at their operating points are in the model, below) |
| Above 4.5 kHz (2' high A with FREQUENCY at 10: 5.6 kHz) | -1.25 cents: the core's model is fitted from 0.7 Hz to 4.2 kHz (A20) |
| IC8's output | Within 16 uV |
| The reverse sawtooth, driven by ngspice's own ramp buffer | Within 0.11 mV at 156 Hz and 0.73 mV at 4.8 Hz (C10's ripple and Q37's junction and diffusion capacitances modelled), away from the resets: in the 5 us after each reset's 8 V edge the stage recovers about 1 us behind ngspice (backward Euler at 1 us steps; 81 mV at the edge). Without Q37's capacitances it was 2.6 mV off. The loaded sawtooth within 5 uV; its output resistance 5459 ohm (ngspice 5458) |

Found on the way: with OSC. 3 CONTROL off oscillator 3 ran 2.7 cents sharp of ngspice.
The tuning had hidden three microvolt-level offsets of the reference, with CONTROL on, in
R109: IC8's input offset in the 741 macromodel (0.188 mV at its +7.5 V common mode, its
90 dB CMRR), the summers' (18.6 uV) and the -5 V line's (19 uV). They are now constants the
test measures in ngspice (`tuning::IC8_OFFSET`, `expo::SUMMER_OFFSET`, `expo::M5_LINE`). A
real 741's offset is random and up to millivolts: a real oscillator 3 with CONTROL off is
off by more than this, untuned.

## Discrepancies and open items

- **B1-1** SG3821 vs 3046 revision date (above).
- **B1-2** Octave string return: drawing (-10 V) vs Section 2.15 text (ground).
- **B1-3** Q5/Q6/Q16/Q17/Q28/Q29: 2N4402 (S-F93) vs 2N3906 (S-T75). Both models are in
  `mm-devices.lib`; the reset timing's sensitivity to the choice is to be measured.
- **B1-4** Section 2.16 (the old board's text) says the keyboard gives "a 20mV decrease at
  the output of IC1" per volt; R20/R27 give 19.6 mV.
- **B1-5** R20's temperature coefficient is not given ("temperature compensating", 1 K,
  +-3 %, 1 W); 3500 ppm/C (the usual value for such resistors) is assumed.
- **B1-6** The range switches' string. synthfool's errata list four editions; the 1st
  (old boards, no octave buffer) has 10 ohm steps and no current-loop resistor; later ones
  have 1K 1 % steps, R29 4.75K and R59 90.9 ohm (Figure 9-17, bulletin 804C). The 1st
  edition's LO step and the sense point are not documented: the benches assume the -5 V is
  sensed at the 2' end (as in Figure 9-17) and LO five octaves below 32' (assumptions A3).
- **B1-7 (resolved)** With every control input at 0 V the calibration could not reach its
  targets (1.5 octaves short, with R11 at its end). The inputs do not rest at 0 V: the pitch
  wheel's 25K pot runs from +10 V to GND with its wiper 15.3K from GND in the detent
  (+6.12 V; dwg 1449, Folkman), and TUNE is R1 5K from +10 V over R22 5.1K to GND (+7.52 V
  at its centre; Figure 9-17). With them, R11 settles at 184 ohm.
- **B1-9** R162 is labelled 3K on Figure 9-3 and R181 51K. Modification 8.2 (boards with
  serial numbers 1300 to 10175: the kit 94-001 configuration this board follows) changes
  R181 from 56K to 51K and R162 from 3K to 3.01K 1 % together (with R69, R105, R141 15K and
  R170 1 %, which the drawing shows): R162 is taken as 3.01K.
- **B1-10** Figure 9-17 (the interconnecting wiring) is the later board's (octave buffer,
  1K string). The oscillators' FREQUENCY pots and SW2 are taken from it; which end of each
  pot is clockwise is not drawn and follows from the pitch rising clockwise.
- **B1-8** Oscillator 2's exponential converter is labelled IC2 pins 6/7/8 and 9/10/11 twice
  on both drawings, which is more transistors than a 3046 has; it is probably IC7's spare
  transistor. It changes only which transistors share a chip (thermal and matching), which
  the model does not represent yet.

## The real-time oscillator (`crates/ca72`)

Derived from the circuit above and checked against it (tests in `crates/ca72-lab`):

| Part | Real-time model | Where its numbers come from | Agreement with ngspice |
|---|---|---|---|
| Exponential converter (`expo.rs`) | The circuit's equations solved by iteration: IC1's summing balance with R42's feedback through the tail node, the divider loaded by the reference base current, R7's drop, the pair's intrinsic VBE difference with RB and RE, both collectors' Early effect, R20's TC, ngspice's temperature laws for IS, BF and ISE, the 741s' bias currents | Component values; `CA3046_NPN` (a test keeps the Rust constants equal to the library's) | After one constant (+1.45 cent; microvolt-level offsets of the references, absorbed by calibration): within 0.27 cent on 32'..2', 1.0 cent on LO, at 15, 25 and 40 C |
| Core (`vco.rs`) | C1 plus 15.9 pF of junction capacitance integrates the timing current, linear in the ramp voltage (Early effect times R42's loop gain, taken from the converter at the ramp's two ends); reset after the Schmitt trigger's slope-dependent delay; jump to a slope-dependent top; 1.2 us hold | `scripts/fit_core.py` on 48 notes from 0.7 Hz to 4.2 kHz (`ca72-lab core`) | With ngspice's own currents: period within 0.07 cent on 8' and 2' |
| Shapers | Buffer linear (2.0346 v + 3.9098); sawtooth R33/(R33+R34); triangle tabulated from ngspice (`tables.rs`, regenerated and compared by a test); rectangle's comparator thresholds (slope 0.998 per volt of width bias, 0.41 V of hysteresis), its edges 2.74 us after the reset and 3.75 us after the ramp passes the threshold less 4.9 mV; the reset's triangle glitch (-3.8e-7 V s) and buffer lag (-4.8e-7 V s) as band-limited impulses | ngspice DC sweeps and transient runs | Every harmonic below 20 kHz within 1 % of the strongest (worst -43 dB: the sawtooth's 5th at 19.8 kHz) at 110, 698 and 3954 Hz |
| Band-limiting | Core at 4x the output rate, two-point band-limited steps at every jump, halfband decimation (flat to 20 kHz, -98 dB stopband) | | Largest alias below 20 kHz: -101 dB at 110 Hz, -86 dB at 698 Hz, -70 dB at 3954 Hz; the steps' own droop is -0.3 dB at 20 kHz (8x oversampling would reduce it to about -0.1 dB) |
| Calibration (`tuning.rs`) | Folkman's 1973 procedure run on the model (26 notes, 0.02 s) | | Tuned independently, model and circuit agree within **0.088 cent** on 40 notes from 22 Hz to 4.2 kHz, 0.69 cent on LO |

Not modelled in real time yet: the core's temperature dependence (its thresholds are 25 C's),
the control path's dynamics (C2, the op-amps' bandwidth: matters for audio-rate
modulation), noise and jitter, the loading of the outputs (the mixer's model supplies it).
Oscillators 2 and 3: below.
