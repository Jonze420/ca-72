# Board 4: filter, VCAs, external preamplifier, A-440

Netlists: [`circuits/boards/board4-vcf.lib`](../../circuits/boards/board4-vcf.lib)
(circuit No. 6), [`board4-vca.lib`](../../circuits/boards/board4-vca.lib) (circuit No.
7), [`board4-preamp.lib`](../../circuits/boards/board4-preamp.lib) (circuit No. 12, the
external preamplifier and overload lamp driver) and
[`board4-a440.lib`](../../circuits/boards/board4-a440.lib) (circuit No. 13, the A-440).
Reference designators are Figure 9-11's (S-F911); the parts list is Table 7-8.

## Which drawing and which values

- S-F911 (Figure 9-11, late 1970s) is the only complete drawing of the board with the filter
  SCALE trimpot (R49, R70), which S-KEH dates from about serial 1664 (1972).
- S-RAM dwg 1446 (filter, rev C 6/14/71, "SN 1130") and dwg 1445 (dual VCA, 12/31/70) are the
  same circuits under R. A. Moog designators (R6xx, R7xx), without the scale trim; dwg 1446's
  regeneration resistor (R608) is 150 ohm where Figure 9-11's R76 is 330 ohm.
- The VCA's input attenuator R2, the output stage's emitter resistors R8/R28 and the A-440's
  feed R40 exist in three documented states (1970: 47K, 27 ohm, 1K; Folkman's July 1973 kit:
  160K, 4.7 ohm, 10K; Figure 9-11: 82K, 8.2 ohm, 22K). The owner chose Folkman's for the
  default (history.md); all three are parameters.

## How it works (from the drawing, checked in ngspice)

1. **Mixer** (front panel; dwg 1446 and Figure 9-17): each source's volume pot (25K linear;
   the external input's 1M audio) feeds the bus through a series resistor when its switch
   is on: 33K for the oscillators and the external input, 11K for noise (12K on dwg 1446).
   Off, the switch shorts the resistor across the wiper and nothing reaches the bus. The bus
   (pin 4) goes through C27 10 uF to Q29's base, which R54 470 ties to the bias chain's
   bottom (C24 220 uF there makes it an AC ground). The bus is therefore a current-summing
   node: each channel arrives about 70 times smaller than its wiper, so three oscillators
   at full volume drive the input pair into its nonlinear region. This is the circuit's
   mixer overdrive.
2. **Control node** (pin 5): the front panel's resistors meet here: CUTOFF FREQUENCY
   (R11 5K linear between +10 V and -10 V) through R55 200K, keyboard control 1/3 (R53 300K)
   and 2/3 (R54 150K), filter modulation (R52 33K), external control (R51 100K); on the
   board, AMOUNT OF CONTOUR through R74 47K, the SCALE trim R49 (500 ohm) with R70 1.8K to
   ground, C18 100 pF. The node sits near 0 V.
3. **Exponential converter**: the node is Q26's base (TIS93 PNP; collector through R45 330 to
   -10 V); the RANGE trimpot R39 (10K from +10 V to ground) feeds Q26's emitter through R46
   68K; that emitter is Q28's base (TIS92 NPN, emitter at ground). Q28's collector current,
   exponential in the node's voltage (17.8 mV per octave at 25 C), is the ladder's standing
   current through R60 680. Q26's junction cancels Q28's VBE; nothing compensates the
   scale's temperature dependence.
4. **Input pair and ladder**: Q29/Q30 (TIS97, matched) take the audio (Q29) and the
   emphasis feedback (Q30); four stacked pairs (Q23/24, Q19/20, Q10/11, Q2/3) with .068 uF
   across each pair's emitters; the bases are fed from the chain R4 220, R16, R32, R41, R52
   150, R67 200 (with C24) from +10 V; the top pair's collectors at +10 V.
5. **Gain recovery amplifier**: the voltage across C3 (the top pair's emitters) is
   AC-coupled (C5, C1 .22 uF into R31, R38 47K: a 15 Hz high-pass) into followers Q8, Q6
   and the differential pair Q7/Q5 (tail R26 1K to -10 V, no degeneration); the output is
   Q7's collector (R3 1K, R7 180, R6 47 to +10 V, C9 100 uF).
6. **Emphasis**: the signal at R3/R7's junction leaves through C10 10 uF (pin 7) and returns
   (pin 6) through the EMPHASIS control, R14 50K reverse audio **used as a rheostat** between
   pins 7 and 6 (Figure 9-17), then the REGEN CAL trimpot R73 1K and R76 330 into Q30's base.
7. **VCAs**: the filter's output reaches the first pair Q16/Q15 (TIS97) through R2 and C6
   0.33 uF, attenuated by R34 820 against the bias node (R22 120, R36 510, C15 220 uF): R2
   sets how hard the first VCA is driven. Q18's current (R37 6.8K, R43 270K to -10 V),
   driven by the loudness contour through R59 68K, is its tail. Its collectors drive the
   second pair Q14/Q13 (tail Q21 from the EXT. LOUDNESS input, R51 33K, R42 3.3K; the rear
   jack J3 normally ties that input to +10 V through 33K: about 1.4 mA, fully on). The
   second pair's collectors drive the PNP output pair Q12/Q17 (R8/R28 to Q1, a current
   source referenced to the VCAs' supply current through R10); the output is Q17's
   collector over R29 680, clamped by CR1/CR2, AC-coupled by C2 10 uF to the main output
   and the headphone amplifier through 1K each. The A-440 joins at Q14's collector through
   R40 and C8 0.1 uF.

## First ngspice results (filter alone, 2026-09-28)

- Ladder balanced at DC (Q29 and Q30's bases within 36 uV) once EMPHASIS is a rheostat.
- Cutoff (-3 dB) against the CUTOFF control's voltage through R55: 34 Hz at -9 V, 91, 259,
  772 Hz at 0 V, 2.29 k, 6.62 k, 17.7 kHz at +9 V: a factor of 2.95 per 3 V (about 0.52
  octave per volt through 200K, so about 1 octave per volt through the keyboard's 100K
  before the SCALE trim).
- Gain from the Q29 base to the output about +35 dB (the mixer's 33K against 470 ohm is
  -37 dB).

## The real-time filter (`crates/ca72/src/vcf.rs`)

Derived from circuit No. 6 and checked against it (`crates/ca72-lab/tests/vcf_realtime.rs`;
both driven with the same ladder current, Q28's collector from ngspice, so the exponential
converter is checked separately):

| Part | Real-time model | From |
|---|---|---|
| Ladder | Four stages' differential capacitor voltages; each stage's emitter current is the one below's less its base currents (Gummel-Poon at the stage's current); each pair's tanh behind its series drop (RE, RB against the base currents), with Vt raised by high injection (IKF); each stage's C plus half its nodes' junction and diffusion capacitance | Circuit equations; `QTIS97` |
| Input | C27 and R54 into the bias chain's bottom (b5: R67, the chain, C24), a state; the input pair's base currents drop across R54 and R73/R76 (its input resistance: 1.5 % of gain at 500 uA) | Circuit equations |
| Output | C5/C1 into R31/R38 as a state, loading the top stage; the gain recovery amplifier as its DC transfer against the followers' differential base voltage, and the followers' base currents (Q8 saturates on the positive side, pumping the coupling capacitors), tabulated from ngspice with nt held (`OUT_DI`, `OUT_DIB`; regenerated and compared by a test); nt (R6, C9) a state: not an AC ground below 100 Hz, where it adds up to 20 % to the emphasis feedback | ngspice DC sweeps |
| Emphasis | C10, R14, R73 (both sides of its wiper), R76 into b5 | Circuit equations |
| Numerics | Nine states in one implicit loop (no delays): trapezoidal rule, Newton with the analytic Jacobian (checked against finite differences), 4x oversampling, the ladder's corner prewarped up to 16 kHz | numerics.md |
| Control node and exponential converter | The node's resistors as a Norton source against R49/R70 and Q26's base current; Q26 and Q28 as full Gummel-Poon transistors (Early effect: Q26's collector near -10 V lowers its VEB by 3 mV, 175 cents; RB, RE, RC, high injection; Q28's reverse current), Q28's collector on R60 from the ladder's tail node (the input pair's emitters under the bias chain), so the tail saturates near 1.7 mA as it does in the circuit. For real time, ln I0 and Q26's base current tabulated against the node's voltage (1 mV steps, -0.8..+0.8 V; built in 0.6 s per temperature and trim setting; 62 ns a lookup) | Circuit equations; `QTIS92`, `QTIS93`, `QTIS97` |

Agreement with ngspice (2026-09-28, 4x at 48 kHz):

| Behaviour | Settings | Result |
|---|---|---|
| Small-signal response | CUTOFF -6..+8 V (ladder 6.3 uA..0.98 mA), EMPHASIS R14 50K..1.5K; where within 40 dB of the peak | CUTOFF -6..0 V: within 0.04 dB; +3..+8 V: within 0.25 dB to 10 kHz, up to 0.7 dB at 20 kHz (the trapezoidal rule's warping at 4x; 0.28 dB at 8x) |
| Resonant peak | CUTOFF -3..+6 V, R14 1.5K and 700 | Frequency +0.6..+6.3 cents (0.9 cent at 1.2 kHz); height within 0.3 dB up to +18 dB peaks, -0.9 dB at +24 dB (3.8 kHz), -2.5 dB at +25 dB (11 kHz): the output stage's capacitances (A11) |
| Self-oscillation | EMPHASIS at 10, CUTOFF -3, 0, +3 V (425, 1239, 3647 Hz) | Frequency +1.1, +1.4, +3.0 cents; amplitude (0.69..0.85 V) within 0.04 dB; 2nd and 3rd harmonics within 0.25 dB |
| Drive distortion | 200 Hz sine, 0.5..20 V into one 33K mixer channel, CUTOFF +6 V (no resonance) and 0 V with R14 1.5K | Every harmonic above -100 dB within 0.33 dB (the 3rd, at -10.8 dB for 20 V, within 0.03 dB) |
| Ladder current | CUTOFF -10..+10 V at 15, 25, 40 C, and with the keyboard input at 2 V (1.3 uA..1.8 mA) | Within 0.09 cent while Q28 is active, 0.49 cent where it saturates; the table adds under 0.001 cent below the knee |

Not modelled in real time yet: the output stage's capacitances (A11); the control node's
dynamics (C18 100 pF against about 2K: a 0.2 us lag, irrelevant below 100 kHz); the VCAs.

## The real-time VCAs (`crates/ca72/src/vca.rs`)

Derived from circuit No. 7 and checked against it (`crates/ca72-lab/tests/vca_realtime.rs`):

| Part | Real-time model | From |
|---|---|---|
| Signal path | Three pairs (Q16/Q15, Q14/Q13, the PNP Q12/Q17), each a tanh behind its series drop (RB, RE, and R8 for the output pair) and its per-side source resistances against its base currents (so the balance trims act as they do in the circuit); the input through R2, C6 and R34 (C6's reference moves with bb); the A-440's R40 and C8 on Q14's collector; the output node Q17's collector across R29 and, through C2, R77 and the load; the output pair's Early effect (its collectors swing volts) | Circuit equations; `QTIS97`, `Q2N4058` |
| Tails | Q18 from the loudness contour (it saturates near 5 V: the gain peaks there), Q21 from EXT. LOUDNESS (J3's 33K to +10 V), Q1 from the chain's node nl: full Gummel-Poon transistors | Circuit equations |
| Supply chain | R10 with C4, R11, R13, R22, R36 with C15, loaded by the pairs: the first pair's tail lowers nl and raises Q1's current, so the output's resting voltage rises 0.2 V with the contour (C4, about 23 ms): the circuit's thump, about 117 mV at the output after C2 for a 5 V contour | Circuit equations |
| Balance trims | The factory procedure (service manual 5.24.1, 5.25; Folkman 1973) run on the model: R12 nulls EXT. LOUDNESS feedthrough with the first VCA off (0.670), R14 at full sustain (0.558) | `VcaCircuit::calibrated` |
| Numerics | Couplings (C6, C8, C2) by the trapezoidal rule, the chain by backward Euler; each tail solved together with the pair it feeds (their three junctions by Newton's method, Kirchhoff's law at the pair's emitters), passes over the three until they agree to 1e-12; Q18 and Q21 (which follow the contour, directly or through nn) every sample, warm-started Newton steps to convergence; Q1 and the pairs' base-current ratios (which follow the chain) every 1/3000 s, interpolated, and at once whenever the contour has moved 2 mV since (a fast attack or release), or every sample while EXT. LOUDNESS is plugged | numerics.md |

Agreement with ngspice (2026-09-28):

| Behaviour | Settings | Result |
|---|---|---|
| Tails and resting output | Contour 0..8 V, trims as drawn and calibrated | Tails within 0.007 %; resting output within 6.2 mV |
| Balance trims | The factory procedure in both | R12 0.6700 against 0.6722, R14 0.5584 against 0.5581 |
| Gain | Contour 0.5..8 V at 1 kHz; 10 Hz..20 kHz at 5 V | +0.08..+0.09 dB throughout (the output pair loads the second pair's collectors about 5 % less in the model: its linearised base current) |
| Static transfer | First pair driven directly, contour 1 and 5 V | Within 28 mV of the 6.45 V swing |
| Thump | Contour 0 to 5 V in 5, 1 and 0.3 ms, 350 ms, back as fast; no signal | Peak 117 mV; within 1.3 mV at each (2026-09-29; before, 2.0, 446 and 2125 mV: a click at every fast attack, numerics.md) |
| EXT. LOUDNESS at audio rate | 1 kHz at 50 mV in, J3 between 1 and 5 V at 0, 5, 100 Hz and 1 kHz, bias every sample | 41.2, 36.7, 38.8 dB below the output; 27.9 dB at 1 kHz, the model's step (35.2 dB at 8 times the rate; numerics.md) |
| EXT. LOUDNESS overdriven | J3 from 5 to 9 V, the contour at -0.35, 0, 2 and 5 V (each tail solved with its pair, 2026-09-29) | Tails within 0.1 %, output at rest within 0.1 mV, gain within 0.06 dB while the VCA is on and off where the circuit shuts it (above about 6.4 to 6.6 V); every bias solve settles (before: tails up to 1,360 % off, gain up to 140 dB, between 6.2 and 7 V) |
| Drive distortion | 1 kHz, 0.5..5 V into R2, contour 5 and 2 V | H1 within 0.08 dB, H2 and H3 within 0.51 dB, higher harmonics above -50 dB within 1.3 dB |

Found on the way: with the trims at mid-travel the circuit leaks EXT. LOUDNESS into the
output at 0.067 V/V; Q18 saturates above about 4.5 V of contour (its collector sits under
the first pair's emitters), so the gain peaks at 5 V (+1.9 dB) and falls slightly beyond.

## The external preamplifier and overload lamp (circuit No. 12)

1. **Where it sits** (Figure 9-17): the EXTERNAL INPUT jack J6 feeds R9 (1M audio, the
   front panel's EXTERNAL INPUT VOLUME) ahead of the preamplifier; its wiper is pin 11. The
   preamplifier's output leaves through C20 1.0 uF (pin 17) to SW10 and R46 33K on the
   mixer's bus (the service manual 2.12 agrees).
2. **The preamplifier** ("a 200 gain amplifier", 2.12): R78 1K and C23 .1 uF into Q27 (R66
   100K to ground), paired with Q32 (TIS97; tail R58 150K to -10 V); Q27's collector (R65
   27K, with R64 100 and C22 100 pF to +10 V) drives Q33 (2N4058), whose collector is the
   output with R57 10K to -10 V. R61 200K against R62 1K, AC-grounded by C26 220 uF, and
   R63 1K close the loop on Q32's base (C21 10 pF): a gain of 201 in the audio band, 1 at
   DC. In ngspice from a 100K source: 37.7 dB, within 1 dB of flat from 100 Hz to 20 kHz
   (-0.9 dB at 20 kHz, -4 dB at 50 kHz). It clips at about -7.3 and +9.9 V.
3. **The lamp driver**: Q25 (2N3392, R53 10K from the output) charges C14 .47 uF with the
   output's positive peaks; R56 100K and R48 680K divide it toward -10 V onto Q31 (TIS93),
   which follows into Q34 (TIS92), sinking the lamp B1 (from +15 V, pin 19) through R72 47
   to the switched ground (pin 20). It lights at about 2 V of peak output and holds about
   0.26 s after the drive stops.

## The A-440 (circuit No. 13)

1. **A Wien bridge oscillator** (2.11): Q22 (TIS97; R25 75K collector, R47 33.2K emitter;
   its base biased by R50 82K from the rail) amplifies, Q9 (2N4058; R15 3.32K emitter from
   the rail, R17 4.75K collector) inverts: a gain of about 3.2. The bridge runs from Q9's
   collector through C13 .030 uF (with C17, "selected 600-1200 pF") and R68 (1K wirewound,
   A-440 ADJ.) with R71 4.75K to Q22's base, which R55 16.9K with C19 .030 uF (and C25,
   selected) hold to ground. CR3, CR4 (SG3246, anti-parallel), C12 .1 uF and R27 470K
   limit the amplitude at Q22's collector. Q4 (2N3392) takes Q9's emitter to the output
   across R5 22K; its collector's R1 is "680K 22M SELECTED" on the drawing and "2.2
   Megohm, Selected" in Table 7-8. SW18 connects the rail (pin 14) to +10 V.
2. **In ngspice** (unloaded): it starts within about 150 ms of switch-on; 1.41 V peak to
   peak at 7.45 V; second and third harmonics 18 and 25 dB under the fundamental; 430.7 Hz
   with R68 at its end, about 420 Hz at its other (B4-7).
3. **Its load** (circuit No. 7): R40 and C8 0.1 uF into Q14's collector. R40 is 1K (1970),
   10K (Folkman's kit, the model's default), 22K on Figure 9-11's legend and 2.2K in Table
   7-8.

## The factory calibration (Folkman 1973; `crates/ca72/src/filter_cal.rs`)

Folkman's three filter steps, done on the real-time filter as the procedure does them on
the instrument (listening to the filter oscillate at EMPHASIS 10 and zero-beating it with
the A-440):

1. **Regeneration Cal.** Mixer switches off, CUTOFF at -1, EMPHASIS at 7.5: R73 turned
   until regeneration starts. R73's travel covers onsets from EMPHASIS 7.0 to 9.0.
2. **Filter Range.** KEYBOARD CONTROL off, CUTOFF at -1, EMPHASIS at 10: R39 for 440 Hz.
3. **Filter Scale.** KEYBOARD CONTROL 1 and 2 on: CUTOFF for 1760 Hz on the third A, R49
   for 440 Hz on the low A, "repeated until the filter will track three octaves".
   Alternating the two as written diverges on the model (R49 swings wider each round:
   420, 232, 445, 209, 477 ohm); the octaves between the two keys depend on R49 alone, so
   the state the procedure aims at is solved directly (R49 such that, with CUTOFF tuning
   the low A to 440 Hz, the third A sounds 1760 Hz).

The result (`FACTORY`): R39 at 0.632, R49 327.0 ohm, R73 at 0.783. The test
`filter_calibration` runs the procedure again and checks the calibrated filter against the
service manual's later figures: fully clockwise it oscillates above 16 kHz (the
measurement stops at the rate's 24 kHz; 5.13: at least 16 kHz); counterclockwise it
regenerates down to 60 Hz (5.13: under 300 Hz); with KEYBOARD CONTROL 1 alone and the low A
at 440 Hz the high A sounds 884 Hz (5.15: 880 +- 50); 1760 Hz lies 2.00 divisions of
CUTOFF above 440 Hz (5.18: about 2); three octaves track within 2.3 cents; regeneration
starts between EMPHASIS 7.4 and 7.6. CUTOFF -1 ends at 430.8 Hz (Range is set before
Scale, which moves it; the procedure does not return to it).

## The real-time preamplifier, lamp and A-440

- **Preamplifier and lamp** (`preamp.rs`): solved as their circuit by the nodal solver.
  The preamplifier runs four substeps a sample by the trapezoidal rule (backward Euler's
  damping upset its loop near its bandwidth: -9 dB at 20 kHz), its input upsampled and its
  outputs decimated by the halfband resamplers. The lamp driver runs once a sample from
  the preamplifier's output (R53's load on that output, a few ohms in the loop, is left
  out). The lamp is a 200 ohm resistor from +15 V, fully lit at 60 mA (its type is not
  documented). Both run only while SW10 is on. In the DAW the model was developed in, the
  device reports the lamp's
  level (its peak, falling over 50 ms, for a reader polling now and then) and the panel
  lights its jewel by it (voice.md).
- **A-440** (`a440.rs`): its unloaded waveform's steady state (24 harmonics) and its start
  from switch-on (the fundamental's amplitude and the mean every millisecond for 300 ms),
  derived in ngspice (`ca72-lab tables`, `a440_table.rs`), played at 440 Hz into the VCA's
  R40 and C8 (the VCA takes it at C8's far side). See B4-7.

| Test | Result |
|---|---|
| `board4ext_realtime.rs`: the preamplifier against ngspice from a 100K source | Gain within 0.003 dB at 100 Hz, 0.002 at 1 kHz, 0.024 at 10 kHz, 0.027 at 20 kHz; driven 20 V into clipping, its levels within 0.010 V; the lamp over half its current from 0.5 ms to 263.1 ms (ngspice 0.04 to 262.6 ms), 60.2 mA |
| `derived_data.rs`: `the_a440_table_matches_the_circuit` | The committed table equals a fresh ngspice run |
| `ca72/tests/a440.rs` | The played steady state's harmonics within 0.6 uV of the table, its mean exact; the start within 0.02 of the table at 20 and 100 ms |
| `voice.rs`: `the_a440_and_the_external_input_meet_the_service_manual` | The A-440 at 440.00 Hz and -5.3 dB at the output (5.7: -8 +- 2: 0.7 dB above, B4-7); -30 dB at 1 kHz into the external input: the lamp lights at VOLUME 9.5 while the output's distortion is the filter's soft overdrive (4.4 %), before the preamplifier clips (20 % at 10) (5.26) |
| `filter_cal.rs` | As above |

## Discrepancies and open items

- **B4-1** R76 330 (S-F911) vs R608 150 (dwg 1446).
- **B4-2** The front panel's pot tapers ("audio", "reverse audio") are not specified beyond
  their names; generic A and C tapers are assumed (assumptions.md A9).
- **B4-3** The TIS97/92/93 have no published models; generic models with the data sheets'
  gain bins (components.md). The matched pairs are modelled identical: mismatch (and the
  VCA balance trims' job of nulling it) is not modelled.
- **B4-4** Q26's collector return (pin 8 or 3) is read as -10 V; it has to be below Q26's
  emitter for the converter to work.
- **B4-5** The output stage's transistor capacitances set the last few cents of the
  resonance at high cutoff (A11); the TIS97's capacitances are typical values, not data.
- **B4-6** The thump (0.2 V at Q17's collector for a full contour) comes from the supply
  chain as transcribed: Q1's base on nl, below R10 which the first pair's tail loads. It
  is the circuit's; whether a unit's thump matches it is for the owner's recordings.
- **B4-7** The A-440 as transcribed cannot be trimmed to 440 Hz: unloaded, R68's travel
  gives 420 to 430.7 Hz; loaded by R40 and C8 as drawn it runs far lower (348 Hz with
  Folkman's 10K, 386 Hz with 22K) and its waveform clips, because Q4 (its collector through
  the selected 2.2M) passes the load onto Q9's emitter. The manual calls Q4 a buffer and
  expects the trim to reach 440 Hz (5.7); its remedy for a trim that will not reach, a
  capacitor across C13, lowers the pitch. The limiter diodes (SG3246) are generic models.
  The model therefore plays the unloaded waveform, as through an ideal buffer, at 440 Hz
  (assumptions A26). At the output it reads -5.3 dB, 0.7 dB above 5.7's window. For the
  owner's comparison.
