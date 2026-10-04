# Board 3: noise generator, modulation mix and power supply

Netlists: [`circuits/boards/board3-noise.lib`](../../circuits/boards/board3-noise.lib)
(circuit No. 3), [`board3-modmix.lib`](../../circuits/boards/board3-modmix.lib) (circuit
No. 15) and [`board3-regulator.lib`](../../circuits/boards/board3-regulator.lib) (circuit
No. 10). Reference designators are Figure 9-8's (S-F98). The front panel's parts (the NOISE
switch SW14 and R50, MODULATION MIX R3 with R23 and R24, the mixer's NOISE channel R10 and
R48, the OSCILLATOR MODULATION and FILTER MODULATION switches) come from Figure 9-17's
wiring (S-F917); the wheels from the left hand controller (Figure 9-12, S-F912, and dwg
1449, S-RAM). The rectifier (Figure 9-13) gives the regulators' unregulated rails.

The headphone amplifier (circuit No. 11) is not transcribed, at the owner's direction
(2026-09-28): the voice's output is the main output.

## How it works (from the drawing, checked in ngspice)

1. **The noise source.** Q15's emitter-base junction is biased into reverse breakdown from
   +10 V through R47 75K (decoupled by C25 10 uF) and R67 470K, its collector open (service
   manual 2.5: "selected, burned-in and retested"). Its noise is in no document; the
   model's source is a voltage behind the junction's small-signal resistance, its density
   set as the factory set R26 (A23, B3-1).
2. **White.** C12 couples the junction to Q12, a common-emitter stage (R17 100K collector,
   R29 4.7M from the collector to its base, C11 100 pF). R26, NOISE LEVEL (a 2.5K
   rheostat in its emitter), is the factory's trim. Q4 follows (C5 100 pF from its base to
   its emitter). White noise leaves through C18 (pin 5): flat within 1.1 dB from 100 Hz to
   10 kHz, with C18 into its load rolling it off below.
3. **Pink.** R16 10K from Q4's emitter into two series RC branches to -10 V, C3 with R8 and
   C2 with R13: poles and zeros alternating from about 100 Hz to 22 kHz, the usual pinking
   network. C1 couples it to Q3 (gain about R3/R1, 27; its bias from its collector through
   R6 and R7, decoupled by C4). Pink noise leaves through C20 (pin 6). It falls 10 dB per
   decade from 100 Hz to 1 kHz, 13.5 from 1 to 10 kHz.
4. **Red.** R12 10K and C7 (a pole at 106 Hz) into Q6, C22 from its collector to -10 V;
   out through C28 with R59 100K (pin 2).
5. **The NOISE switch** (SW14, Figure 9-17). At WHITE it sends the white noise to the
   mixer's NOISE channel (R10 25K linear, its wiper through R48 11K to the bus) and the
   pink to MODULATION MIX. At PINK it sends the pink through R50 24K to the mixer and the
   red to MODULATION MIX. R50 brings the pink down to the white's level; the factory's
   check (5.27) requires both to read -5 +-3 dB at the output with one setting of R26.
6. **MODULATION MIX.** Oscillator 3's switch output (through R23 24K) and the noise
   (through R24 24K) reach the ends of R3 (25K linear, its wiper grounded); each end feeds
   the modulation mix amplifier through R64 or R62 43K. Oscillator 3's end is
   counterclockwise, so turning the knob clockwise grounds oscillator 3's share and opens
   the noise's.
7. **The modulation mix amplifier** (circuit No. 15): Q18/Q17 a pair on R54 43K, Q17's base
   grounded; Q17's collector (R42 6.2K) drives Q7 (PNP), whose collector is the output with
   R30 10K to -10 V; R60 91K closes the loop (R38 390 and C14 compensate it). It inverts:
   from either end at its own end the gain is -0.838, with +39 mV of offset. Q7 only
   sources current; R30 alone pulls down. It is linear to 0.05 % over +-5 V and flat to
   100 kHz.
8. **The line.** R57 1K takes the output to the left hand controller: the MODULATION wheel
   (R1402, 50K audio used as a rheostat to ground, "1.2K WHEN FULLY FORWARD", Figure 9-12)
   and the two switches. OSCILLATOR MODULATION puts the line on the oscillators' MOD bus
   (their three 51.1K inputs to their summing junctions against R163 33K from +10 V); off,
   the bus is grounded (A21). FILTER MODULATION puts it on the filter's control node through
   R52 33K; off, R52 is grounded. With the wheel fully forward and oscillator 3's square,
   the line swings about 2 V peak to peak (Figure 9-2: "1.75 P-P MAX"). A line that would
   need Q7 to sink current clips: Q7 turns off and R30 drives the output alone.
9. **The pitch wheel** (dwg 1449): a 25K linear pot from +10 V to ground, its wiper 15.3K
   from ground in the detent (+6.12 V), +-2 V over its travel (Figure 9-2: "5V +- 2V";
   A22), into the oscillators' pitch bus.
10. **The regulators** (circuit No. 10). +10 V: Q1 (MPS-U55, PNP) passes from the
    rectifier's +20 V. CR3 (1N821, 6.2 V at 7.5 mA; R44 511 sets its current) hangs the
    reference under the +10 V sense line on Q9's base; R39, R21 (25 ohm, the +10 V trim) and
    R34 divide the sense line onto Q8's base. The pair Q9/Q8 (2N4058, heat sunk together)
    has its tail R31 from the output; Q9's collector (R27 to ground) drives Q5 through R20,
    and Q5 (C6 its Miller capacitor) pulls Q1's base through R10. Q2 is the start-up: with
    the output down it is off and R5 and R14 turn Q5 on; with the output up it takes R5's
    current. -10 V: Q20 (MPS-U05, NPN) passes from -20 V. R65, R58 (10 ohm, the -10 V
    trim) and R52 divide between the two sense lines, and the pair Q13/Q14 (TIS-97, heat
    sunk together) holds that point at the ground sense line, so -10 V tracks +10 V. Q13's
    collector drives Q19, and Q19 drives Q20's base. R45 and R66 (10 ohm) join each sense
    line to its output on the board. Found in ngspice: trimmed to +-10.000 V, R21's wiper
    sits at 0.30 and R58's at 0.78 of their travel, and CR3 carries 7.3 mA, the 1N821's
    specified 7.5 mA.

## The real-time models

- **Noise** (`ca72/src/noise.rs`): the circuit at its operating point (the nodal
  solver), linearised into G and C matrices and the source's input vector
  (`linear.rs`). This is the circuit's small-signal model, not a fitted filter. It is
  discretised by the trapezoidal rule at four times the output rate and decimated
  (halfband stages). The source is Gaussian white noise from a seeded generator (PCG32,
  Box-Muller), so a render repeats. Its density is the factory's calibration (A23). The
  outputs are AC coupled, so their loads (the NOISE switch, the mixer's VOLUME,
  MODULATION MIX) change only the output nodes' conductances. A change is applied at most
  every 64 samples, and the nodes keep their state across it.
- **Modulation mix** (`modulation.rs`): the amplifier's DC transfer (offset, the gain from
  each end, and its output resistance while sourcing about 1 mA) solved from its circuit
  at 65 MODULATION MIX positions and interpolated. The line is the output behind that
  resistance and R57 against the wheel and the switched loads, with the second regime
  where Q7 turns off (A24). It uses the last sample's oscillator 3 (a sample's delay in a
  path flat to 100 kHz).
- **Wheels** (`modulation.rs`): the pitch wheel's voltage and the MODULATION wheel's
  resistance, from their pots (A22).
- **Regulators**: not in the real-time model; the rails are ideal (A1), on the measurements
  below.

## Tests

| Test | What | Result |
|---|---|---|
| `noise_realtime.rs` | The noise circuit's operating point, its small-signal response from the source to each output as the switch loads them, the discretised response, and the generated noise's spectrum | Operating point within 0.065 mV of ngspice. Responses within 0.001 dB and 0.00 degrees from 1 Hz to 20 kHz. Discretised at 4x within 0.09 dB (white), 0.21 dB (pink) and 0.001 dB (red) of the continuous response to 16 kHz (0.6 dB allowed to 20 kHz, the trapezoidal rule's warping). Generated noise in octave bands within 0.42 dB of the response |
| `ca72/tests/noise.rs` | Loads changed in place against the circuit solved again with them | Within 8.5e-15; the state carries over |
| `modmix_realtime.rs` | The amplifier's transfer at five MODULATION MIX positions; the line with the wheel fully forward and the MOD bus, oscillator 3 from -4.3 to +2.5 V | Offset and gains equal to four digits. The line within 1.7 mV while Q7 sources firmly; 7.5 mV at +1.8 V and 17 mV at +2.5 V, near the clip (A24) |
| `regulator.rs` | Circuit No. 10 trimmed as 5.6 does, at 150 mA a rail from +-20 V | Output impedance 41.6 milliohm (+10 V) and 47.4 (-10 V) at most, 10 Hz to 100 kHz; -10 V moves 41.7 milliohm with +10 V's load (it tracks), +10 V 0.4 with -10 V's. Load regulation 50 to 300 mA: 10.6 mV and 22.6 mV. Ripple at 120 Hz 78 dB down: from 1.25 V peak to peak on each unregulated rail, 0.16 mV on +10 V and 0.30 mV on -10 V |
| `voice.rs`: `modulation_and_the_wheels_meet_the_service_manual` | 5.37, 5.19 and 5.35 on the voice | Oscillator 1 swings 18.0 semitones under oscillator 3's low square, wheel fully forward (5.37: 13 to 23). The filter's corner (its I0) rises 33.8 times with FILTER MODULATION (5.19: at least 2400/440 = 5.45). The pitch wheel's travel 16.1 semitones (5.35: 13 to 17) |
| `voice.rs`: `the_noise_sits_under_the_triangle_as_the_factory_set_it` | 5.8 and 5.27 on the voice | At the output, both channels at VOLUME 4 and the filter open: white -6.39 dB and pink -6.02 dB against oscillator 1's triangle (5.8 and 5.27: -6, each within 3 dB). From 125-250 Hz to 4-8 kHz the white tilts -0.5 dB and the pink +15.1 dB (3 dB an octave) |
| `voice.rs`: `oscillator_3s_frequency_and_wide_range_meet_the_service_manual` | 5.35 and 5.36 on the real-time oscillator 3 with the voice's tuning | FREQUENCY spans 16.8 semitones at middle C (14 to 17). With OSC. 3 CONTROL off on LO it clicks every 4.95 s at its minimum (5.36: 2 to 5 s); its maximum, 10.99 Hz, overlaps 32''s minimum, 6.48 Hz. This also checks LO's place (A3) |
| `ca72-lab` `patch_keys` | Every patch key is taken, every patch in `patches/` parses, a misspelt key, a wrong type or a knob out of range is refused | Passes |

Found by these tests:

- **The pink network was transcribed wrong.** C3 and R8 were two shunts, not one series
  branch. That network is a single pole, and the pink fell 6 dB an octave above 1 kHz. The
  voice's check against 5.27 found it: at the output the pink sat 12 dB under the white,
  which no setting of R26 could bring into 5.27's window. Figure 9-8 draws C3 and R8 on one
  vertical, like C2 and R13.
- **Changing a load rebuilt the noise model from its operating point, and reset it**, every
  sample while a knob moved. It was too slow, and the red noise's long time constants
  would have restarted each time. The loads now change in place (above).
- **`ca72-lab` took `osc3_control` for an oscillator's key** and refused it; `patch_keys`
  now covers every documented key.
- **Counting a reset twice**: a fast reset spans two or three samples at the output rate,
  so the first period measurement read 2.48 s where the oscillator ran at 4.95 s.

## The rails: why ideal (A1)

The loads that move with the signal draw a few milliamperes from the rails: the
oscillators' rectangles and buffers into the mixer, the VCA, the output. Through 40-50
milliohm that is about 0.1-0.2 mV. The ripple adds 0.16 mV (+10 V) and 0.30 mV (-10 V)
peak to peak at 120 Hz. A rail reaches an oscillator's pitch through its pull-ups and pots
at well under 1 octave per volt: the MOD bus's pull-up about 0.26 octave per volt of +10 V,
the pitch wheel's pot about 0.2. Even at a whole octave per volt, 0.3 mV is 0.36 cent, and
the rails stay ideal. Hum
reaching the audio path through the circuits' own supply rejection is not modelled
(assumptions A1).

## Open items

| # | Item | Status |
|---|---|---|
| B3-1 | Q15's noise density is in no document | The factory calibration sets it (A23); a unit's own R26 setting is within 5.27's +-3 dB |
| B3-2 | 5.27 does not give the mixer's VOLUME | VOLUME 4 (Table 5-3, the only mixer setting in the procedure), under which white and pink both fall in 5.27's window (A23) |
| B3-3 | The MPS-U05 and MPS-U55 models are generic (no data sheet in hand) | The regulators' output impedance falls with the pass transistors' gain: at half the gain (BF 75) +10 V's is 72 milliohm, still under 0.1 ohm |
| B3-4 | With the MODULATION wheel up, oscillator 3's triangle raises the pitch's centre (+0.26 semitone at wheel 0.1, oscillator 3 on LO) | The circuit's: the triangle's mean is not 0 V and the amplifier's offset is +39 mV, both through the line to the MOD bus. A candidate for the owner's comparison |
| B3-5 | Service bulletin 832 (mod wheel bleed-through) | Not yet read against this circuit |
