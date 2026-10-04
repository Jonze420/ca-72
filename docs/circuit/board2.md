# Board 2: dual contour generator and keyboard circuit

Netlists: [`circuits/boards/board2-contour.lib`](../../circuits/boards/board2-contour.lib)
(circuit No. 8) and [`board2-keyboard.lib`](../../circuits/boards/board2-keyboard.lib)
(circuit No. 1). Reference designators are Figure 9-7's (S-F97). The front panel's parts
(ATTACK, DECAY, SUSTAIN, AMOUNT OF CONTOUR, GLIDE) come from Figure 9-17 (S-F917) and the
front panel parts list (S-F93, section 8); the DECAY and GLIDE switches from the left hand
controller (Figure 9-12, S-F912) and the service manual's text (section 2.10).

## How it works (from the drawing, checked in ngspice)

1. **Trigger section.** A key joins the keyboard's trigger bus to its +10 V bar. The bus
   charges C7 (0.1 uF) through the germanium CR10 and R60 100K; C7 drives Q20's base
   through R55 47K. At rest CR10's reverse leakage holds C7 near -10 V, so after a key goes
   down C7 must charge past Q20's threshold first: about 8 ms. Q20 pulls down the reset
   line (R37 from +9.3 V); Q12, its base on that line through R19, inverts it into V-trig
   (+9.3 V while a key is held, 0.05 V at rest). V-trig's rise (R23/R32's "set" line) is
   AC-coupled through C1 [C4] into both flip-flops. After the key is released Q20 stays on
   until C7 has drained into its base: about 12 ms. EXT. S-TRIG (pin 19) pulls the reset
   line down through R49 220 like Q20. Legato playing does not retrigger: the bus stays at
   +10 V while any key is held.
2. **Flip-flops** Q1/Q4 [Q25/Q15]. Set, Q5 [Q16] (saturated) charges the timing capacitor C5
   [C2] (10 uF) through R7 [R42] 100 and the ATTACK pot (1M audio, a rheostat) toward the
   +9.3 V rail. The flip-flop resets when the output, divided by R33/R29 [R24/R27] toward
   -10 V, drives enough current through CR3 [CR6] into Q4's [Q15's] base to take Q1 [Q25]
   out of saturation: at 4.49 V [5.28 V] out. It is also held reset while the reset line
   drives CR1 [CR8].
3. **Decay and sustain.** Reset, Q6 [Q17] turns Q7 [Q18] on, saturated: the capacitor
   discharges through the DECAY pot (1M audio) into the sustain node, which R10 [R45] 27K
   and Q7's base current (1.3 mA from Q6 through R9) feed and the PNP follower Q8 [Q19]
   holds at the SUSTAIN divider's voltage (R13 4.7K from the 5K SUSTAIN pot, R17 150K to
   -10 V and R1 3K to ground [R57 4.7K and 4.7K to ground]).
4. **Release.** V-trig at rest pulls the sustain node down through CR2 [CR9] and the DECAY
   jack's normal contacts: DECAY on, the capacitor decays to about 0.66 V at the DECAY
   time; DECAY off, it is also dumped through CR7 [CR4] and R1401 1.5K to V-trig.
5. **Outputs.** The filter contour's follower is Q22 (NPN) with Q21 (PNP): one base-emitter
   drop (rest +0.15 V, peak 4.49 V); the loudness contour's is a Darlington, Q3 and Q2: two
   drops (rest -0.48 V, peak 5.28 V). The filter contour drives AMOUNT OF CONTOUR (5K) and
   through its wiper R74 47K on board 4; the loudness contour R59 68K into the VCA's Q18.
6. **Decoupling.** Q26 (2N3415) makes the +9.3 V rail from +10 V (R62 330 into its base,
   +15 V through R63 15 to its collector); the timing capacitors' charging (up to 14 mA
   with both attacks at their fastest) sags it about 75 mV.
7. **Keyboard circuit** (circuit No. 1): Q9 (PNP) and Q11 (NPN), matched, source 8.33 mA
   (Figure 9-17: 8.48 mA) into the 43 resistor string (10 ohm 1% each; 83 mV per key); the
   pitch bus (the string's voltage where the pitch bar touches it, lowest key first) is
   held on C9 0.33 uF. An amplifier (Q23/Q14 matched, Q24) drives C6 1 uF through R61 330
   and the GLIDE pot (5M, a rheostat; shorted by the GLIDE switch); Q13 (JFET) connects C6
   only while a key is held (CR5 pinches it off from the trigger bus); Q10 (JFET) follows
   C6 to the output (R18 3.9K to -10 V), and R30 10K closes the amplifier's loop from the
   output. With glide the loop saturates: C6 charges toward the amplifier's rails (+10 V up,
   about -4.4 V down through R59/R54), so gliding up and down run at different rates.
   Found in ngspice: with no key down the pitch bus does not rise to +10 V but settles near
   3.2 V, where R53's current equals Q23's base current (Q23 takes the whole tail, Q14 is
   off and its collector at +10 V, as the service manual says). At the release the hold
   switch lets go about 1.2 ms after the trigger contact opens (C13 through R34, then CR5
   pinching Q13's gate); a pitch bus released at the same moment drifts meanwhile, and the
   note would be held about 6 cents sharp (hence the contacts' order, A18). At the press,
   CR5's capacitance kicks Q13's gate up as the trigger bus rises, and the hold switch
   conducts within about 30 us.

## The real-time contour generators (`crates/ca72/src/contour.rs`)

| Part | Real-time model |
|---|---|
| Trigger | C7 a state (trapezoidal), charged through CR10's junction law and R60, drained by Q20's base current; Q20 and Q12 as full Gummel-Poon transistors (saturation included), the reset line and V-trig solved from their KCL; C13's node on the trigger bus a state |
| Flip-flops | Latches (their transitions take microseconds): set as V-trig rises, reset while the reset line drives CR1 [CR8] or when the output reaches the threshold derived from the latch's devices (Q1 [Q25] leaving saturation) |
| Timing capacitors | Set: the RC charge through R7 and ATTACK from the rail (exact trapezoidal). Reset: the capacitor and the sustain node together (Newton on both), with Q7's base current, Q8 as a PNP follower on its divider, CR2 to V-trig, and with DECAY off CR7 into R1401's node |
| Outputs | The followers' drops from their transistors at the load's current (the AMOUNT OF CONTOUR pot and the peak divider on the filter contour) |

Agreement with ngspice (2026-09-28; ngspice's waveforms taken at the model's 48 kHz: the
flip-flops' resets put 30 ns spikes on the rail and outputs):

| Scenario | Result |
|---|---|
| Held key, DECAY on and off; retriggered 30 ms after a release | Within 27..44 mV; peaks within 7 mV; trigger edges within 0.04 ms |
| Released during a slow attack | Within 20 mV; peaks within 7 mV; edges within 0.15 ms |
| Fastest attack (500 ohm), SUSTAIN at 0 and 10 | Within 72 mV (mid-attack, the rail's sag: A16); peaks within 11 mV; edges within 0.13 ms |

The same scenarios at the voice's 24 kHz (interpolated onto ngspice's 48 kHz grid): within
86 mV (the fastest attack), trigger edges within 0.19 ms (assumptions.md A17). At 12 kHz
the fastest attack was 183 mV off, and before the trigger's solves were run to
convergence (60 steps, not 8) a trigger at 12 kHz could be missed.

## The real-time keyboard circuit (`crates/ca72/src/keyboard.rs`)

The whole circuit (No. 1) is solved as its netlist by a small nodal solver
(`crates/ca72/src/mna.rs`, numerics.md): ngspice's device equations (Gummel-Poon
transistors with their series resistances on internal nodes, the JFETs' channels with
their gate junctions on the internal drain and source, CR5 with its depletion and
diffusion charge), backward Euler steps, Newton's method with SPICE's junction and FET
limiting. The key string is in the circuit: the current source as a Thevenin source fitted
to its own operating points (Q9's output resistance shows when two keys held together
short part of the string), the string's resistance above the highest key held, between the
lowest and highest held, and below the lowest, and the two keys' contacts (0.1 ohm, A18).

Agreement with ngspice (2026-09-28, `crates/ca72-lab/tests/keyboard_realtime.rs`,
48 kHz; ngspice at 2 us steps):

| Behaviour | Result |
|---|---|
| The string's current | 8.41263 mA against 8.41265 mA |
| Every key's settled output, with the three oscillators' and KEYBOARD CONTROL 1's load (-3.9 mV at the lowest, +7.5 mV above the bus at the highest: a 0.3 % scale error the factory tuning absorbs, with about 0.45 mV of curvature) | Within 0.1 uV |
| GLIDE off: detached notes, legato down and back (the output slews about 5 V/ms, Q13 at its saturation current) | Within 11.5 mV anywhere (2 us of a slew); within 0.02 mV where the output is quasi-static |
| GLIDE 100K: legato up and down (up toward +10 V, down toward -4.4 V) | Within 0.18 mV; quasi-static within 0.07 mV |
| GLIDE 1M: released half way, held, played again | Within 0.03 mV |
| The trigger contact 2 ms ahead of the pitch contact | Within 7.4 mV; quasi-static within 0.03 mV |

Found on the way: the transistors' series resistances are worth 1.35 mV (1.6 cents) at the
top key; the JFET's gate junctions must sit behind its drain resistance (8 mA through
10 ohm moved the forward-biased gate 80 mV and Q13's current 4 %); stepping in blocks once
the output was still stepped over the trigger bus's rise (the quiet test now watches the
trigger bus and Q13's gate and waits 2 ms after any change).

Cost: the circuit is solved only while it moves (8 substeps per sample for 2 ms after a
contact changes, then per sample until still, then in blocks of 64 samples); 0.1-0.27 s
per second of the busy test scenarios at 48 kHz, 0.2 s per second in `v0-bass.json`.

## Discrepancies and open items

- **B2-1** The loudness sustain divider's resistor to ground is labelled R41 on Figure 9-7,
  which is also the flip-flop's 560 ohm: a drawing error; transcribed as R47 4.7K.
- **B2-2** Figure 9-12 draws SW1401 (DECAY) with R1401 looping back to its own common: the
  service manual's text (2.10) is followed (DECAY off: the dump line through 1.5K to Q12's
  collector); S-ERR lists a correction to this switch.
- **B2-3** The trigger delay (about 8 ms from a key to the attack) is set by CR10's
  reverse leakage holding C7 near -10 V at rest; the 1N34A's leakage is a generic value
  (components.md) and Q20's emitter-base breakdown (about -5 V for small-signal parts),
  which ngspice's model does not include, might clamp C7 higher in a real unit (a shorter
  delay).
- **B2-4** The keyboard current is 8.33 mA on Figure 9-7 and 8.48 mA on Figure 9-17; the
  circuit's own value follows from Q9/Q11 and R1: 8.413 mA in ngspice and in the model
  (84.13 mV per key). The oscillators' tuning uses the keyboard circuit's output.
- **B2-5** The key contacts' resistance and their timing are not documented (A18): with two
  keys held the string's current flows through their contacts, and the lower key sounds
  I x R sharp (1.1 cents at 0.1 ohm).
