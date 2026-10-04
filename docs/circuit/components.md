# Component models

Models live in [`circuits/models/`](../../circuits/models/). Provenance classes: **DS**
data-sheet derived, **GS** generic substitute (typical silicon values plus the data sheet's
specified points), **FIT** fitted to measurements, **INF** inferred from circuit operation,
**UNK** unknown. Confidence is for the behaviour that matters where the part is used.

## Semiconductors

| Model | Part | Where (board 1) | Class | What matters there | Confidence | Notes |
|---|---|---|---|---|---|---|
| `CA3046_NPN` | CA3046 / SG3821 array transistor | expo pair and tail (IC2, IC7) | DS | The pair's VBE difference vs current ratio (ideality), matching, temperature; base and emitter resistance at high currents (R42 compensates it) | Medium: IS and ideality from spec points, RE/RB split assumed | C1. Intersil's own model (D-3046M) has IS 10x lower (its VBE at 1 mA is 0.651 V, the data sheet says 0.715 V) and no RB/RE: cross-check only |
| `Q2N3392` | 2N3392 (GE) | buffer output Q3, triangle Q1/Q2, current source Q4, Q12, Q38 | GS | Q2's saturation (the triangle's fold), Q3's gain | Medium-low | Gain bin from MPS3392 |
| `Q2N4058` | 2N4058 | buffer PNP pair Q8/Q9, Q11 | GS | Gain and speed of the buffer | Medium-low | |
| `Q2N4402` | 2N4402 | reset Q10, Schmitt Q5/Q6 | GS | Switching speed and storage time: the reset's duration, so the pitch at high notes | Low | C4: TR estimated from toff |
| `Q2N3906` | 2N3906 | Table 7-5's reading of Q5/Q6 | GS | as above | Low | A7 |
| `JE402` | E402 dual N-JFET | ramp buffer input Q7 | DS | Gate leakage (low frequencies), offset, transconductance | Medium | Mid-range of the data sheet's limits |
| `ua741` | uA741 | IC1, IC3, IC9 | DS (Boyle 1974 method) | Gain, bandwidth, slew rate, output swing | Medium | No noise, no input common-mode limit |
| `QTIS97` | TIS97 (TI) | board 4: the ladder and its input pair Q29/Q30, the gain recovery amplifier Q8/Q6/Q7/Q5, the VCAs | GS | The ladder: the exponential law, beta (base currents lower each stage's current and load R54 and R73/R76), RB and RE (series drop), IKF (high injection lowers gm by 0.25 % at 250 uA), and the junction and diffusion capacitances (0.01 % of the ladder's C); the output pair's transfer and Q8's saturation; the output pair's capacitances (Miller effect on Q7's base) | Low: only the gain bin is specified; the rest are typical small-signal values | B4-3. The real-time filter's derived quantities follow the model's parameters (a test keeps the Rust constants equal to the library's) |
| `QTIS92` / `QTIS93` | TIS92 / TIS93 (TI) | board 4: the filter's exponential converter Q28/Q26 | GS | The converter's VBE matching (+-3 mV at 20 mA, bulletin 804C), Early effect and high injection at the top of the range | Low | |
| `Q2N3392` | 2N3392 (GE) | board 2: the flip-flops, switches and followers of the contour generators, the trigger section, the keyboard amplifier | GS | Saturation voltages (the release floor), the flip-flops' threshold | Medium-low | |
| `Q2N3415` | 2N3415 | board 2: the +9.3 V decoupling Q26 | GS | Its drop at the charging currents (the rail's sag) | Medium-low | |
| `J2N4303` | 2N4303 N-JFET (Siliconix) | board 2: the keyboard's glide switch Q13 and follower Q10 | DS | Q13's saturation current (the keyboard's slew with GLIDE off, about 5 V/ms) and its forward-biased gate while it slews; the follower's offset (inside the loop: no effect on the output) | Medium | The real-time model is the same equations (`mna::J2N4303`, a test keeps them equal) |
| `D1N34A` | 1N34A germanium point contact | board 2: the flip-flops' peak and reset feeds, the trigger input CR10 | GS | Forward drop (the attack's peak), reverse leakage (the trigger delay: B2-3) | Low | Data sheet gives maxima only |
| `D1N4004` | 1N4004 rectifier | board 2: the sustain nodes' and capacitors' releases, the glide switch's gate | GS | Forward drop at milliamperes (the release floor) | Medium-low | Generic rectifier values |
| `QMPSU05` / `QMPSU55` | MPS-U05 NPN / MPS-U55 PNP (Motorola 1 W plastic) | board 3: the regulators' pass transistors Q20 and Q1 | GS | Their current gain (the regulators' loop gain and so their output impedance) | Low: no data sheet in hand | B3-3: at half the gain the output impedance is 72 milliohm, not 42 |
| `D1N821` | 1N821 temperature-compensated reference | board 3: the +10 V regulator's CR3 | DS | Its voltage at 7.5 mA (the rail's trim range) and its dynamic impedance | Medium | Spec points only; its temperature compensation is not modelled |

The real-time nodal solver (`mna.rs`) uses the same device equations as ngspice's
Gummel-Poon and JFET models, including the junction depletion charges (CJE, CJC) and
diffusion charge (TF) on the transistors' internal nodes; Rust constants mirror the
library's parameters, and a test keeps them equal where both are used.
