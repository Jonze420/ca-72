# Revision manifest

The target is a Minimoog Model D of about 1972-73 with the second-generation ("old")
oscillator board, whose exponential converters are CA3046 transistor arrays (or Silicon
General's SG3821, its drop-in equivalent). A year or a serial number does not fix a circuit:
boards were modified in service by factory kits and replaced. So the configuration is
defined board by board, by drawing, with the evidence for each choice. Source tags are in
[sources.md](sources.md).

## The oscillator board's three revisions

| Revision | Exponential converter | Serial numbers | Evidence |
|---|---|---|---|
| 1, R. A. Moog | discrete 2N4058 | to about 1150-1300 (sources differ) | S-KEH; forum accounts; no schematic found |
| **2, "old" board** | **CA3046 / SG3821 + tempco resistor** | **about 1237-10174** | S-F93 title, S-T75, S-08001, S-FOLK, board photos (SG3821N date code 7337, CA3046 RCA 701) |
| 3, "new" board | heated uA726 | 10175 on | S-SM 2.3, Figure 9-2 |

## Configuration MM-73

| Board / assembly | Drawing used | Variant | Evidence it belongs | Status |
|---|---|---|---|---|
| 1 Oscillator | S-F93 (with S-T75 values) | kit 94-001 applied (see Q1) | Revision 2 by its "3046" arrays and serial range; S-FOLK (7/73) prescribes kit 94-001 for boards above 1300 "to improve tracking and pitch stability", and S-F93 is that board | one oscillator transcribed and simulated |
| Range switches (front panel) | S-ERR "1st edition": 10 ohm steps, no current loop resistor, no buffer board | parametric (A3) | The editions' order and the old board's 25 ohm octave trimpot, which only has authority over a low-impedance string | values not transcribed (B1-6) |
| Waveform switches | S-RAM dwg 1448 | as drawn | Same wiring as the later S-F916/S-F917 | used |
| 2 Contour generators and keyboard | S-RAM dwg 1436 (keyboard); contour to be chosen between S-F97 and the R. A. Moog contour drawing, with S-ERR's early-unit notes | to do | | keyboard current (8.48 mA) used |
| 3 Power supply, noise, modulation mix, headphone amp | S-F98 and the R. A. Moog drawings | to do | | rails ideal (A1) |
| 4 Filter, VCAs, preamp, A-440 | S-F911, with S-RAM dwg 1446 (filter, rev C 6/14/71) and 1445 (VCA, 12/31/70) for the earlier values | Folkman's 1973 values for R2, R8/R28, R40 (owner, Q2) | S-F911 has the filter scale trim that S-KEH dates from serial 1664 (1972) | in progress |
| 5 Rectifier | S-F913 | | | not needed while rails are ideal |
| Left hand controller | S-RAM dwg 1449 rev B (7/1/71) | as drawn | Same as S-F912 for the pitch wheel | pitch wheel detent used |
| Keyboard | 44 keys, 43 x 10 ohm 1 % string, 8.48 mA source (dwg 1436) | | | used |

## Differences between the two drawings of board 1

Kit 94-001 (S-FOLK Section XII; S-SM 8.2) turns S-08001 into S-F93:

| Part | S-08001 (as built, 1972) | S-F93 (kit applied) |
|---|---|---|
| R69, R105, R141 | 6.8K | 15K |
| R78, R106, R128 | 2K | 2K with 220 ohm + .01 uF in series across it (RC network 65-032) |
| R181 | 56K | 51K |
| R170 | 15K 5 % | 15K 1 % |
| R162 | 3K 5 % | 3.01K 1 % (S-F93 still draws 3K) |
| C3, C5, C7 | 47 pF | 100 pF |

Differences not explained by the kit (S-08001 vs S-F93): R34 4700 vs 4300; C14 0.1 vs
0.12 uF; Q38 2N3398 vs 2N3392; the dual JFET's type (hand-lettered on S-08001, E402 on S-F93).

## The owner's answers (2026-09-28)

- **Q1: kit 94-001** (Figure 9-3) is the default; the as-built board is a variant.
- **Q2: Figure 9-11 with Folkman's 1973 values** for R2, R8/R28 and R40 (160K, 4.7 ohm,
  10K) is the default; the 1970-71 values (47K, 27 ohm, 1K) and Figure 9-11's own (82K,
  8.2 ohm, 22K) are variants.

## Questions for the owner (as asked)

- **Q1.** Board 1: model the board as built (S-08001) or with kit 94-001 (S-F93)? Both are
  a 1973 instrument's circuit: the kit was prescribed in July 1973 for boards above serial
  1300. The netlist follows S-F93 now; the as-built values are a small, local change
  (R69, the RC network, C3). Recommendation: S-F93 as the default, the as-built board as a
  selectable variant, measured against each other.
- **Q2.** Board 4: with or without the filter modification of S-FOLK (to reduce
  intermodulation distortion when mixing oscillators; serial numbers below 2000)? It
  changes the mixer and filter input's overload character, so it is audible.
