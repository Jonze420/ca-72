# Numerical studies

## Circuit reference (ngspice 47)

Solver settings: `.options reltol=1e-4 abstol=1e-12 vntol=1e-6 method=gear maxord=2`, time
step at most the expected period / 400, 25 C. A running oscillator has no DC equilibrium,
so each transient starts from the circuit with its ramp held at -2 V by a switch that opens
after 1 us (the operating point then converges and slow parts such as C13's 135 ms filter
start settled).

| Study | Result | Test |
|---|---|---|
| Frequency against the solver: reltol 1e-5, 4x finer time steps, trapezoidal integration, on 21.8 Hz, 293.6 Hz and 3953.8 Hz | Largest change 0.0114 cent | `vco_numerics.rs` |
| The runner treats a failed analysis as an error | A missing model and a floating node fail; a stepping method that fails before another completes does not | `ca72-spice/tests/runner.rs` |

## Real-time oscillator

| Study | Result |
|---|---|
| Oversampling 4x against 8x | Frequency identical (the core's events are exact in time); aliasing and the 20 kHz droop improve with 8x |
| Band-limiting | Two-point polyBLEP at 4x plus halfband decimation: largest alias below 20 kHz -70 dB at 3954 Hz |

## Real-time filter

| Study | Result | Test |
|---|---|---|
| Analytic Jacobian of the nine-state loop against central differences, small and saturated states | Agrees to 1e-5 of each row's scale | `ca72/tests/vcf_jacobian.rs` |
| Trapezoidal rule at 4x against 8x (the error above 10 kHz is the integrator's warping, not the model) | At 20 kHz above a 11 kHz corner: -0.64 dB at 4x, -0.12 dB at 8x; below 10 kHz both within 0.25 dB | `small_signal_response_matches_the_circuit` |
| Prewarping the ladder's corner: none, to 8, 12, 16 kHz, or always | Prewarping puts the resonance where the continuous model has it (4x and 8x agree to 0.5 cent; without it 4x is 11 cents flat at 11 kHz). Prewarping a corner above the audio band (30 kHz at +9 V) makes the audio band worse (-0.97 dB at 20 kHz, against +0.01 dB when prewarped at 16 kHz). Chosen: the corner, up to 16 kHz | `Vcf::prewarp_hz`; history.md |
| Self-oscillation reference against ngspice's step | 500, 2000 and 4000 steps per period: -0.07 cent, reference, +0.004 cent (at 300 per period of a lower guessed frequency the reference was 2 cents flat) | `oscillation_reference_converges` |
| Where the remaining resonance difference comes from | Removing capacitances in ngspice: the output stage's account for 0.9..3.2 cents and 2.2 dB (11 kHz, +25 dB peak); the ladder's 0.15..0.4 cent (now modelled); with all removed, the model agrees to 0.2 cent and 0.36 dB | `capacitance_budget` (diagnostic) |
| The converter's table against its circuit solution | 1 mV steps: 0.0003 cent below the saturation knee, 0.11 cent in the knee, 0.23 cent in deep saturation; 4 mV steps were 7 cents off in the knee | `ca72/tests/expo_table.rs` |
| The converter's circuit solution | Q28 by Newton's method on its two internal junction voltages with SPICE-like step limits (a fixed point on its base current failed once the base-collector junction conducted) | `vcf_control.rs` |

## Real-time VCAs

| Study | Result | Test |
|---|---|---|
| The bias network solved every sample, against every 1/3000 s with the tails interpolated, against Q18 and Q21 every sample and the rest interpolated | Thump within 0.4 mV, 12.5 mV (the tails lagged the contour by a block), 2.0 mV; cost 29 %, 5.7 %, 8.5 % of real time at 48 kHz (to be optimised with the benchmarks). Chosen: the last | `thump_matches_the_circuit` |
| Q18's and Q21's Newton steps per sample from the last sample's solution | Two steps give the same result as converging to 1e-12 V (while the contour moves slowly; see the next row) | `thump_matches_the_circuit` |
| A fast attack (2026-09-29): the contour rising to 5 V in 5, 1 and 0.3 ms | With the bias interpolated a block behind and two Newton steps, the thump was 2.0, 446 and 2125 mV off the circuit's 118 mV: a click at the start of every fast attack (0.125 of full scale in one sample on the bass line). Now, once the contour has moved 2 mV from where the bias was last solved, it is solved at once and taken whole, and the tails' Newton steps run to convergence (60 at most): 1.3 mV at every rise. Renders change only at attacks and releases; 2 % slower | `thump_matches_the_circuit` |
| EXT. LOUDNESS at audio rate: a tremolo between 1 and 5 V on a 1 kHz tone, the bias solved every sample | 0 to 100 Hz: 36.7 to 41.2 dB below the circuit's output. 1 kHz: 27.9 dB; the error is the model's step, first order, falling as the VCA is oversampled (31.4, 33.8, 35.2 dB at 2, 4, 8 times). Not the transistors' charges: ngspice without them gives the same figures. (Before the fast-attack correction this case read 35.5 dB: the tails two Newton steps behind cancelled part of the step's error, and gave the attack's click) | `the_loudness_jack_follows_audio_rate_cv`, `tremolo_step_budget` (diagnostic) |

## Real-time keyboard circuit (the nodal solver)

The keyboard circuit is solved as its netlist (`crates/ca72/src/mna.rs`): node
voltages by Newton's method with analytic device Jacobians, backward Euler steps (the
closed hold loop's pole is a few microseconds when the hold switch conducts: stiff at any
audio step, where the trapezoidal rule would ring at the sample rate), SPICE's pnjlim on
junctions and fetlim on the JFETs' gate voltages, GMIN on every node and junction, a
failed step retried from the last solution at half the step (down to 1/1024).

| Study | Result | Test |
|---|---|---|
| Analytic Jacobian (NPN and PNP, forward active, saturated and off; the JFET either way round; a diode with depletion and diffusion charge; capacitors) against central differences | Within 2.6e-7 of each entry (entries below 10 nS, GMIN's, within 1e-11 S absolutely) | `ca72/tests/mna.rs` |
| A diode and resistor from a wild start (5 V across the junction); an RC step | The closed form to 1 uV and 1e-9 A; backward Euler exactly | `mna.rs` |
| What failed on the way | Damping every Newton step to 1 V held linear nodes back and let a junction creep to 0.99 V forward in small unlimited steps (currents of amperes); a diode's diffusion charge taken at the unlimited voltage reached 1e21 C; a failed step retried from its own wreckage. Now: full steps with junction limiting only, charges at the limited voltage, the last solution restored before a retry | the keyboard's tests (no failed steps) |
| Steps per sample around a key event: 1, 8, 32 | The output's timing after the trigger contact: backward Euler at one step per sample lags the trigger bus's 48 us rise; 8 substeps for 2 ms after a change and while the output moves over 1 mV a sample agree with 32 within 1 mV. Chosen: 8 | `playing_matches_the_circuit` |
| Stepping in blocks once still | Blocks of up to 64 samples once the hold capacitor, the output, the trigger bus's node and Q13's gate are still (under 0.1 uV a sample) and 2 ms have passed since any change; the pitch bus may drift meanwhile (it does while holding, over seconds) | `playing_matches_the_circuit` |
| The test's contact times | Rounded to samples: 0.1 + 0.002 s in floating point lands a sample late and looked like a 52 mV error | `playing_matches_the_circuit` |
| Transistor capacitances in the solver (junction depletion charge for CJE and CJC on the internal nodes, diffusion charge for TF, as ngspice's Gummel-Poon) | Oscillator 3's reverse sawtooth (Q37) went from 2.6 mV to 0.11 mV of ngspice away from the resets; the keyboard circuit's results are unchanged | `revsaw_realtime.rs`, `mna.rs` |

## Real-time noise generator and modulation mix

The noise generator is linear at its operating point (its transistors' signals are
microvolts to millivolts), so the real-time model is the circuit's small-signal model:
G, C and the source's input vector from the nodal solver at the operating point
(`linear.rs`), discretised by the trapezoidal rule, x1 = A x0 + B (u0 + u1).

| Study | Result | Test |
|---|---|---|
| The small-signal model against ngspice's AC analysis, white, pink and red as loaded | Within 0.001 dB and 0.00 degrees, 1 Hz to 20 kHz | `noise_realtime.rs` |
| Trapezoidal rule at 4x (192 kHz), against the continuous model | Within 0.09 dB (white), 0.21 dB (pink), 0.001 dB (red) to 16 kHz, within 40 dB of each output's peak; the warping reaches 3.8 % at 20 kHz. At the output rate the white noise's top octave, which reaches past C11's pole near 16 kHz, would bend: hence 4x | `noise_realtime.rs` |
| The source: Gaussian, standard deviation S sqrt(fs / 2) per oversampled step for a one-sided density S, decimated by halfband stages | The generated spectra (200 averaged periodograms) within 0.42 dB of the response in octave bands from 31 Hz to 11.3-16 kHz (the decimators' transition starts at 20 kHz) | `noise_realtime.rs` |
| Loads changed in place (the output nodes' conductances) against the circuit solved again | Within 8.5e-15 | `ca72/tests/noise.rs` |
| The modulation mix amplifier's DC transfer at 65 positions, interpolated, against ngspice | Offset and gains equal to four digits at five positions, between solved points too | `modmix_realtime.rs` |
| Its output resistance, measured at the offset point against measured sourcing about 1 mA | At the offset point Q7 barely conducts and the resistance is not the working one; measured sourcing 1 mA (as the wheel's line draws), the line agrees within 1.7 mV where Q7 sources firmly | `modmix_realtime.rs` |

## Regulators (ngspice)

| Study | Result | Test |
|---|---|---|
| The trims, as 5.6: R21 for +10.000 V, then R58 for -10.000 V; bisection with its direction from the wiper's ends | To 0.1 mV; R21 at 0.295, R58 at 0.779 | `regulator.rs` |
| The pass transistors' gain (generic models, B3-3), 150 against 75 | +10 V's output impedance 41.6 against 72 milliohm | by hand, board3.md |

## Real-time external preamplifier (the nodal solver's trapezoidal rule)

The nodal solver (`mna.rs`) integrates charges by backward Euler by default (L-stable: the
keyboard circuit's hold loop is stiff). It now also takes the theta method
(`Circuit::set_theta`; 0.5 is the trapezoidal rule), which the preamplifier uses.

| Study | Result | Test |
|---|---|---|
| The preamplifier at four substeps: backward Euler against the trapezoidal rule, the input linearly interpolated | -1.6 and -9.3 dB at 10 and 20 kHz by backward Euler; about the same by the trapezoidal rule: the error was the input's linear interpolation (a 48 kHz sine linearly interpolated loses exactly that) | `board4ext_realtime.rs` |
| The same with the halfband resamplers on the input and outputs, ngspice point-sampled | Within 0.027 dB to 20 kHz | `board4ext_realtime.rs` |

## The filter's factory calibration

| Study | Result | Test |
|---|---|---|
| "Filter Scale" as written: CUTOFF on the third A, R49 on the low A, alternated | Diverges on the model (R49 420, 232, 445, 209, 477 ohm); solved directly for the state it aims at (R49 for two octaves with the low A at 440 Hz): 327.0 ohm, three octaves within 2.3 cents | `filter_cal.rs` |
| Regeneration's onset | "Grows or is already at its limit" over 0.4 s from a kick; R73's travel covers onsets from EMPHASIS 7.0 to 9.0 | `filter_cal.rs` |
| Self-oscillation above the rate's Nyquist | At the top of CUTOFF with R49 high the oscillation passes 24 kHz and the measurement reads 0 Hz: the procedure's CUTOFF searches stay within 0.05..0.85 of its travel | `filter_cal.rs` |

## Performance (stage 7; `ca72-lab perf`)

The owner's request (2026-09-29): a 100 times faster voice with no loss of audio quality or
latency. What counts as no loss here: every scenario rendered within 1e-7 of full scale of a
recording made before any optimization (below a 24-bit output's last bit), every accuracy
test against ngspice passing with its figures, the latency unchanged. `ca72-lab perf record
<dir>` renders seven scenarios (idle; `v0-bass`, `v0-three`, `v0-board3`; `perf-ext`,
`perf-a440`, `perf-screech`) at 48 kHz and keeps every sample; `ca72-lab perf check <dir>`
renders them again and reports the largest difference and the speed-up (release build,
the Linux reference machine, Ryzen 7 7800X3D).

Where the time went before (a held note, three oscillators, per sample): the contour
generators 18.9 us (at half rate: 38 us a tick; nested solves: 1-D roots by finite
differences around junction solves of up to 60 iterations), the filter 11.3 us (four
trapezoidal Newton steps a sample, two iterations each, three evaluations of the nine-state
loop per step), the three oscillators 10.5 us (the converter's two fixed-point solves a
sample, 1.3 us each; the core itself 0.27 us), the VCA 2.1 us, the noise 1.7 us, the rest
under 0.5 us: 45 us, 2.2 times real time.

| Change | Exact? | Effect |
|---|---|---|
| Oscillators nothing hears are not computed (A28) | Their output reaches nothing; a re-enabled one resumes where it stopped | Up to two thirds of the oscillators' cost in the default patch |
| The converter's solves reused when their arguments repeat exactly (the drive, the warm start, the circuit, the temperature; the last two kept) | Yes | An oscillator on a steady drive: 2.9 us to 0.29 us a sample |
| Contour parts at rest (moving under 1e-13 V a tick) not solved again until an input changes (A28) | To 1e-13 V times the time constant | Contours 18.9 to 10.5 us on a held note, near zero at rest |
| The filter's pairs solved from their last solution; the prewarping factor kept with the bias | To the pairs' tolerance (1e-15) | The filter 7.0 to 6.5 us on the test signal |
| The contour sections' decay solved with its Jacobian analytic (each junction in series with its resistance moves by g / (1 + R g) a volt) instead of by differences | To the solve's tolerance (1e-11 V) | One residual an iteration instead of three; the voice's construction 2.0 to 1.35 s (its contours settle for 2 s) |
| A tried memo of the whole contour tick at exact fixed points | Never hits: every state keeps changing in its last bits (slow tails, one-ulp jitter) | Dropped |

Result of this first pass: 1.4 to 2.5 times faster by scenario, 1.6 times over all seven
(92.8 s to 57.4 s of rendering for 39.5 s of audio); every scenario within 2.2e-9 of full
scale (-173 dBFS) of the recording.

### Second pass (2026-09-29, the same night)

Built with the `profile` feature, `ca72-lab perf check` also reports where each scenario's
time goes, part by part (`crates/ca72/src/prof.rs`; without the feature the laps
compile to nothing). The profile showed the solvers doing work the models do not need:

| Change | Exact? | Effect |
|---|---|---|
| The series junction's solve (`series_junction_from`) stops on an exact zero residual and accepts a Newton point on its bracket's end | The root, where it returned a point up to 1e-13 V from it | Whenever the junction carried next to nothing, the root sits within rounding of the bracket's end: the Newton point there was rejected and the solve bisected away from the root it had found, 13 to 55 iterations. Now 1.5 to 1.8 on average. The contours 9.4 to 2.6 us a sample on a held note; the voice's construction 1.35 to 0.83 s |
| The contour's roots whose direction the circuit fixes (C7, the reset line, V-trig, the dump node, the followers) no longer probe the far end of their brackets | Bit-identical | The probe solved a transistor far from its warm start (60 limited steps) |
| Q20 and Q12 solved with their Jacobian analytic (`BjtAt::currents_d`); the reset feeds' and Q12's base junctions warm-started | To the solves' tolerances | The trigger 9.4 to 5.4 us, V-trig 2.5 to 0.4 us |
| The trigger's and V-trig's roots with their slopes analytic: each transistor's current sensitivity from its final Jacobian (the implicit function theorem), each junction behind a resistance g / (1 + R g) | To the roots' tolerance | One evaluation a step instead of two; the trigger 1.25 to 0.79 us |
| The filter's Newton system solved by its structure: stages 1 to 3 and W follow stage 0 along the ladder, leaving a 5 by 5 system | To rounding | Instead of a pivoted 9 by 9 elimination |
| The filter step's derivatives and output at the solution from the last Newton iteration's, to first order (the step is under 1e-10 V; the gradient of the output comes with the Jacobian) | To far below rounding | One evaluation of the loop fewer a step |
| Gummel-Poon's base charge in closed form: with I_f = Ic qb, qb = q1 (1 + sqrt(1 + 4 I_f / IKF)) / 2 solves to qb = q1 (1 + q1 Ic / IKF) | To rounding at the ladder's currents, within 1e-12 at 1 mA (where six fixed-point passes had not converged) | The filter's bias 2.6 to 1.0 us a call; the converters faster too |
| The filter's input pair shares the first stage's slope and factor; a junction's constant capacitance factor taken once | Bit-identical | |
| The pairs' tanh anchored (`PairWarm`): each pair keeps a point whose tanh libm gave; within 0.01 of it tanh follows by the addition formula with tanh d to its d^7 term. Every value comes from an exact anchor, so rounding cannot accumulate | Within an ulp or two of libm's tanh | Most of the filter's and the VCA's tanh calls saved: the filter 4.2 to 3.3 us on the test signal, the VCA 1.73 to 1.38 us |
| The noise model's state update four rows at a time, each row's sum in its own order | Bit-identical | The noise 1.72 to 1.15 us |
| The nodal solver (keyboard, preamplifier): critical voltages kept; the elimination visits only the rows and columns with entries (bit masks set by the stamps and by fill-in), in the same order, and back substitution uses the pivot rows' lists | Bit-identical | The solve 45 % faster; the keyboard 1.67 to 1.18 us on the bass line |
| The depletion charge's a^-m as exp(-m ln a) | Within an ulp or two of pow | Half pow's time; the external input's scenario 4 % faster |
| Tried and dropped: a chord step (the second Newton iteration on the first's Jacobian) | The step's end would carry (J1 - J2) dy, about 5e-12 V a step, which a self-oscillating filter could accumulate | Dropped |
| Tried and dropped: resistors' conductances kept; a division skipped for rows with nothing under the pivot | Bit-identical | No measurable gain (the divisions were pipelined) |

Result (on the Linux reference machine): 2.5 to 5.6 times the first recording's speed by
scenario, 3.8 times over all seven (92.8 s to 24.7 s of rendering for 39.5 s of audio;
idle 0.40 of real time, a held bass note 0.50, three oscillators 0.57, the A-440 0.45, the
screaming filter 0.74, the external input 1.53); the voice built in 0.57 s instead of 3.2 s.
Every scenario within 2.6e-8 of full scale (-152 dBFS) of the recording, below a 24-bit
output's last bit: the most in `perf-screech` at its third note, where the filter at
EMPHASIS 9.5 starts to oscillate and amplifies rounding-level differences (the difference
rises with the note and dies away; it does not drift). Every accuracy test against ngspice
keeps its figures (a regeneration frequency and a THD figure move in their eleventh
significant digit); latency unchanged (no resampler, oversampling factor or delay changed).

Where the time goes now (a held bass note, us a sample, with the profiler's own laps):
the filter 4.9 (bias 1.0, evaluations 2.1, solves 1.2, resampling 0.45), the contours 2.1
(trigger 0.8, decay 0.7, V-trig 0.3, followers 0.2), the VCA 1.4, the noise 1.2, the
keyboard 1.2 (its solver's load 0.7, solve 0.3), the oscillator 0.5. With the external
input on, its preamplifier alone takes 24 us (a 19-node circuit at four trapezoidal
substeps, 2.8 Newton iterations each): that scenario runs at 1.5 times real time on this
machine, too slow to play live.

**The 100 times asked for is not reached, and cannot be without changing the output.**
100 times would be about 0.45 us a sample; the filter alone needs four implicit Newton
steps of a nine-state loop a sample, two iterations each, which cost more than that. What
is left for exact work is small: the bias's chain of logarithms and exponentials (1 us,
latency-bound), a polyphase interpolator, the noise model's decimators interleaved, the
VCA's fast solves with an analytic Jacobian: perhaps 10 to 15 % together. The
preamplifier needs its own solver (the nodal solver's generality is most of its cost).
Anything beyond that changes the models: lower oversampling, lower contour and bias rates,
reduced models, or relaxing "no loss" from below the last bit of a 24-bit output to below
audibility (say -120 dBFS), which would allow looser tolerances, a chord Newton and
table-driven transcendental functions. Those are the owner's decision.

## The worst case (quality modes; `ca72-lab worst`)

The owner's No Compromises mode must play in real time at 256-frame blocks (5.33 ms) in the
worst case (history.md, "Three quality modes"). `ca72-lab worst [--block N] [--seconds S]
[--serial]` (`ca72-lab/src/worst.rs`) runs the voice as the DAW the model was developed
in did: the panel set
anew every 32-sample control step with every knob sweeping and every switch and selector
changing on its own period, both wheels moving, the EXTERNAL INPUT on throughout with a
tone at its jack, notes as sixteenths at 120 BPM over the whole keyboard (detached and
legato, GLIDE on half the time); every rear jack plugged (the oscillators' control input
at audio rate, the filter's and the loudness input swept, S-TRIG pulsed); each block timed,
and a failure when one takes longer than it lasts (the workers and the caller at the audio
threads' priority).

Baseline (2026-09-29, the Linux reference machine, 60 s, the voice on its three threads):
every block over; mean 12.5 ms, 99th percentile 21.0, worst 22.1 ms (4.1 times the block);
in turn on one thread, mean 17.1, worst 33.8. Where it goes (in turn, us a sample): the
external input's preamplifier 26.2, the keyboard circuit 25.1 (every note, GLIDE and
KEYBOARD CONTROL change keeps it at eight substeps a sample; the nodal solver's load and
solve for the two circuits 30.3 and 14.6), the filter 7.3 (its bias every sample: 1.1), the
oscillators 4.8 (their memo defeated by the moving knobs and wheels), the contours 2.9, the
noise 1.9, the VCA 1.5. The two circuits solved by the general nodal solver are most of it.

With the rear jacks plugged and the VCA's fast-attack correction (2026-09-29, the same
machine): threaded, 60 s, every block over, mean 12.1 ms, 99th percentile 21.3, worst 22.5
ms (4.2 times the block); in turn, 20 s, mean 18.9, worst 35.0. In turn, us a sample: the
preamplifier 26.2, the keyboard circuit 25.0 (the nodal solver's load and solve 30.4 and
14.5), the filter 7.4, the oscillators 5.4, the VCA 4.5 (its bias solved every sample while
EXT. LOUDNESS is plugged), the contours 2.9, the noise 1.9.


## No Compromises (quality modes; from 2026-09-29)

The rule: every render within 1e-7 of full scale of the reference (the model after the
VCA's fast-attack correction, recorded by `ca72-lab perf record` with the worst case's load
added as a scenario, `worst`), every ngspice comparison keeping its figures; the goal: one
instance in real time at 256-frame blocks in the worst case (`ca72-lab worst`). `ca72-lab
solvers` shows each nodal circuit's size and Newton iterations under the worst case's load
(and, built with `profile`, its time per iteration, load and linear solve apart).

Under that load, before any change: the keyboard circuit (31 solved nodes, 58 parts) 11.0
Newton iterations a sample (2.0 a solve, 5.6 solves a sample), 2.7 us an iteration; the
preamplifier (19 nodes, 36 parts) 11.8 (2.9 a solve), 1.7 us; its lamp driver (15 nodes,
28 parts) 2.8, 1.3 us. An iteration's device evaluation (the load) took twice its linear
solve.

| Change | Result | Check |
|---|---|---|
| Newton starts from the solved nodes extrapolated quadratically from the last three accepted points (when the last two steps were as long) | Iterations a solve: keyboard 1.98 to 1.09, preamplifier 2.94 to 2.37, lamp 2.77 to 2.11; linear extrapolation gave 1.27, 2.37, 2.29; scaling it to a changed step gained nothing | Within 2.8e-9 of full scale (-171 dBFS) on every scenario |
| A step's accepted charges from its last load moved by their capacitance over the last update (q + C dv, dv under the 1e-9 V tolerance), not evaluated again | Keyboard 18.7 to 15.2 us a sample | Within 2.9e-9 |
| A transistor's `exp` and `expm1` of one argument from one exponential away from 0 (`exp(x) - 1` is within an ulp there) | Preamplifier 15.7 to 14.7 us | Within 2.9e-9 |
| Each part's stamps' positions and the Jacobian's pattern (DC and step) worked out once, not per load | Load 1.58 to 1.47 us an iteration (keyboard) | Bit-identical |
| The elimination's structure recorded and replayed while each column's pivot is the one recorded (searched as before; worked out again from the first that moves) | Linear solve 0.78 to 0.58 us (keyboard), 0.51 to 0.38 (preamplifier) | Bit-identical; `mna::tests::the_replayed_elimination_matches_the_reference_to_the_bit` against the elimination it replaces, over random sparse systems whose pivots move (a singular one gives a correction that is not finite either way) |
| The load's parts taken out for its loop, not copied one by one (a part is as large as its largest kind, a transistor's model); resistors' conductances kept with their resistances, not divided each iteration | Keyboard load 1.41 to 1.17 us an iteration, preamplifier 0.93 to 0.77 | Bit-identical |
| A junction's depletion charge's `a^-m` from a nearby exact one (the binomial series within 1/256, to degree 6) | Keyboard load 1.12 to about 0.95 us (with its diodes') | Within 3.0e-9 (4.8e-10 in the worst case) |
| The voice on five threads: the preamplifier ahead of the audio path, the back (filter, VCA) behind it | Threaded worst case, 256-frame blocks, the machine otherwise idle: mean 4.65 ms of 5.33, 99th percentile 7.5 ms, worst 8.6 ms (162 %), 23 % of blocks late; 512-frame blocks: mean 8.4 ms of 10.7, worst 14.7 ms (138 %), 11 % late. In turn: 2.18 times real time (3.52 before any change) | Bit-identical to the voice in turn (`tests/threaded.rs`) |
| The recorded elimination replayed from a flat list of its additions (the entry each adds to and the pivot row's entry it reads), without the lists' bookkeeping and without the additions that only zeroed the entries below a pivot (never read again) | Keyboard linear solve about 0.50 to 0.43 us | Bit-identical: the replay test against the reference elimination, and every scenario of `ca72-lab perf check` identical in every sample |
| A depletion charge's constants (its series' coefficients, and SPICE's F1 to F3 beyond FC*VJ, two `pow`s) worked out once per part, not at every load | Keyboard load 1.00 to 0.79 us an iteration: its forward-biased junctions (transistors' base-emitter, conducting diodes) took the two `pow`s at every load | Bit-identical |
| A step's charges accepted without copying each part (a part is 120 bytes) | Keyboard at eight substeps a sample 13.2 to 12.9 us a sample | Bit-identical |
| Tried and dropped: divisions by a transistor's constants and by the pivots made products by their reciprocals | No gain measured | Outside the rule: the worst case 1.29e-7, perf-screech 1.18e-7 (a quarter second each) |
| Tried and dropped: a constant base for the Jacobian (GMIN, and the resistors' slopes that come before any changing slope at their entries: 96 of the keyboard's 102); each column's additions in one run after its factors | No gain: the resistors' cost is their residual; the elimination is bound by its chain of divisions (each column's pivot waits for the last column's), not by its loop | Bit-identical |
| The VCA's bias (its tails, the chain) worked out with the contours, ahead of the audio path, and handed to the back as twenty values a sample (`Vca::control`, `Vca::signal`, `VcaDrive`); each thread's inputs and outputs a sample sized to its own | Threaded worst case, 256 frames, 30 s: 2.3 to 2.9 % of blocks late (132 to 163), pinned 0.6 to 0.75 % (32 to 42), mean 3.3 to 3.7 ms; 512 frames: 2.5 % (69), pinned 0.2 % (6) | Bit-identical (the reference; the threaded voice against the voice in turn) |
| The VCA's bias on a thread of its own, fed the loudness contour and the jack by the contours' thread sample by sample (not through the caller); six threads | Threaded worst case, 30 s, 256 frames: 0.13 to 0.34 % of blocks late (7 to 19), pinned or not, mean 3.2 to 3.3 ms, 99th percentile 5.0 ms; 512 frames: none late, the worst block 97 to 98 % of its 10.67 ms. Each thread's worst (pinned, `profile`): the bias 13.5 us a sample, the contours 17.1, the preamplifier 19.1, the back 15.2, the front 10.7; the keyboard 24.7 (99th percentile 21.1), the only one to overrun a block alone | Bit-identical (the reference; the threaded voice against the voice in turn) |
| Newton's bookkeeping trimmed (from a second agent's review): the residual's maximum only on failure, a solve's limiting started only for the parts that limit, a step's charges accepted only for the parts that hold one, the saved and history voltages swapped, not copied | Eight substeps a sample 12.24–12.28 to 11.42–11.79 us; in turn 2.06 to 2.01 times real time; threaded, 60 s at 256 frames (clean runs): mean 3.28–3.29 to 3.14–3.20 ms, late blocks 13 and 21 to 8 and 19 | Bit-identical |
| Tried and dropped: link-time optimisation (fat, one codegen unit); `-C target-cpu=native` | LTO under 1 % in turn; the native target slower over the scenarios (28.2 s against 26.9) | Bit-identical |
| Tried and dropped: a helper thread for the keyboard circuit, evaluating some of its parts within each Newton iteration (the stamps then added in their order) | Slower: at eight substeps a sample 13.6 to 16.0 us against 13.1, whatever the parts; the threaded worst case 97 late blocks against 38 (pinned 40 against 31). A round trip between two cores with the voltages and stamps is about 240 ns, as much as the work moved | Bit-identical |
| Tried and dropped: each transistor's temperature scaling (`Bjt::at`: a logarithm and three exponentials) kept per thread | No gain: the VCA's bias solve is 42 % exponentials in its Newton iterations, 28 % `solve_tail_pair`'s own work, 17 % the transistors' currents | Bit-identical |

**Where No Compromises stands** (2026-09-29, the VCA's bias on its own thread; the machine
otherwise idle): every render identical to the reference in every sample; the worst case
at 512-frame blocks in real time (30 s, no block late, the worst 98 % of its time); at 256
frames 0.13 to 0.34 % of blocks late, every one the keyboard's: its heaviest blocks (eight
substeps a sample after key changes with GLIDE) take up to 24 us a sample against the 20.8
a sample lasts. A helper thread for its Newton iterations was built and measured slower
(the table): a round trip between cores costs as much as the work it moves. No exact
option for the keyboard is left that I know of; what remains is lookahead (the threads
ahead of the audio path running a block ahead, the samples the same but a block later) or
512-frame blocks. A worker that stops now
leaves every sample it did not give silent and counted (before, the back ran on stand-in
inputs), and the caller never waits for it (`tests/threaded.rs`).

**After the VCA's bias moved to the contours' thread** (2026-09-29, the machine otherwise idle; `ca72-lab worst`
built with `profile` now gives each thread's 99th percentile and worst over all blocks,
and the blocks its work alone overran). Pinned: the keyboard 20.9 and 24.2 us a sample,
61 blocks alone over (with the profile's own timing, about 0.8 us a sample of it); the
contours with the VCA's bias 18.8 and 26.1, 21 over (the bias's spikes, many passes of
its solve, as where EXT. LOUDNESS overdrives it); the preamplifier 16.4 and 17.9, the
front 9.4 and 10.7, the back 12.5 and 14.9, none over. What is left to reach no late
block, exactly: the keyboard's heaviest blocks (a helper thread for its devices'
evaluations within each Newton iteration, stamped in order, is the exact option left),
the bias on a thread of its own (fed the loudness contour by the contours' thread), and
the threads' placement (pinned, a quarter of the late blocks).

**Where No Compromises stood** (2026-09-29, after the second pass; the machine otherwise
idle): every render within the rule (after the second pass, identical to the reference in
every sample); the worst case in turn 2.13 times real time (2.39 before the second
pass); threaded at 256 frames over 30 s, mean 3.7 to 3.9 ms of 5.33, 99th percentile 5.9
to 6.0 ms, 3 to 4 % of blocks late (160 to 230 of 5,625; 23 % before the second pass).

Where the threads run matters. With each pinned to a core of its own (`MM_PIN=1,2,3,4,5
ca72-lab worst`), 2.2 to 2.5 % of blocks are late (126 to 140); with two to a core (its two
hardware threads), 8 % (454). The first pass's figure for the keyboard's slowest blocks (33
to 36 us a sample) was inflated by sharing a core: in turn, its heaviest blocks take 14
to 21 us a sample (eight substeps at about 1.1 Newton iterations, 1.3 us an iteration, and
0.3 us a step besides). Measured with each thread on a core of its own, the late blocks are
the back's (the filter 11.4, the VCA 12.4 us a sample). While EXT. LOUDNESS is plugged, the
VCA's bias is solved every sample, and that solve is 90 % of its time. The bias reads
nothing of the signal path (only the loudness contour, the jack and its own chain), so it
can run ahead on a thread of its own as the preamplifier does, bit-identically. A model from
the in-turn blocks has that halve the late blocks; every block left is then the keyboard's.
The keyboard is one connected circuit (no part of it can be solved apart), and its Newton
tolerance protects the pitch (an error in its output voltage becomes a drift of phase).

Profiled (a SIGPROF sampler preloaded into the keyboard's bench: no profiler here can use
the kernel's performance counters) at eight substeps a sample: the elimination 30 %, the
load 19 % besides the exponentials 12 %, the stamps 10 %, Newton's bookkeeping 8 %, the
junctions' series 6 %. `ca72-lab solvers` now reports how often each circuit's elimination is
worked out again (the keyboard's 0.07 % of its solves), its solves by Newton iterations
(the keyboard's: 91 % one, 8.5 % two, 21 in 3 s over 16) and the keyboard's subdivided
steps (none).

**How far a render moves under rounding** (found 2026-09-29). In the worst case's load a
change of rounding (the changes above, or a transistor exponential taken from a nearby
exact one, within an ulp or two of `libm`'s) left the output bit-identical for about 2 s,
then parted from the reference by a quarter second's largest difference growing about
tenfold every quarter second (4.8e-10 by 3 s for the changes kept; 1.3e-7 for the
nearby exponential). Probing the voice's signals (`ca72-lab probe`) showed it is not the
circuit's dynamics: the internal signals part within a few samples, by 1e-28 to 1e-16,
and jump where a solver's stopping decision falls differently. The rule "within 1e-7 of
the reference" then measured when a change first altered a bit, not how much.

So the owner chose (history.md) to judge renders against their own spread under rounding:
`ca72-lab perf record` (built with `twins`) renders each scenario again with every
elementary function's result moved one ulp up, then down (`ca72::ulp`: the model as
another, equally correct library would compute it; `sqrt` is correctly rounded everywhere
and not moved), and `check` allows each quarter second of a render whose twins part by more
than 1e-7 ("chaotic") the twins' largest difference there, and every other render 1e-7. The
reference's spreads (the reference built from the model as it then stood, with the `ulp`
module and this `perf`): perf-idle 1.9e-14, v0-bass 2.7e-8, v0-three 7.1e-8, perf-ext
1.3e-11, perf-a440 7.9e-10; v0-board3 6.3e-7 and perf-screech 1.1e-7 (chaotic by that
measure); worst 5.5e-2 from the first quarter second on. So 1e-7 is only a few times the
model's own rounding on the musical renders.

**The worst case's 5.5e-2 was a defect of the VCA** (fixed the same day, history.md):
where EXT. LOUDNESS overdrives Q21 (the jack rising through about 6.4 V, the second pair
cutting off), the bias solve's passes over the coupled tails did not settle, and there
the output was rounding's. Each tail is now solved with its pair (`devices::solve_tail_pair`);
with the model corrected the worst case's twins part by 6.4e-8 and every render is judged
by 1e-7. The reference moved to the corrected model (recorded with twins from the build of
the fix). `ca72-lab solvers` counts the VCA's unsettled solves and the junction solves that
stop unconverged (none of either under the worst case's load).

Found on the way by the DAW's real-time safety test: the recorded elimination's lists lost
their room when a voice was copied from its prototype (a `Vec`'s copy keeps its contents,
not its capacity), so the copy's first recording on the audio thread allocated. The program
now copies with its room, and the Minimoog's own allocation test plays the voice on its
threads too, counted from its first sample, with the external input, GLIDE, the keyboard's
switches and every jack moving (`rt_alloc.rs`; it fails on the old copy).

The reference is `ca72-lab perf record` of that model (kept in `target/mm-perf/nc-ref`, not
in git).

Together: the worst case's load in turn from 3.52 to 2.37 times real time (with the
`profile` build's timing), the threaded voice's worst 256-frame block from 22.5 ms to
14.1 ms (measured before the last two rows) of its 5.33 ms. (These figures were taken with
the owner's demo project playing in another process, using about two cores.)


## High Fidelity (quality modes; from 2026-09-29)

The limits (the owner's, history.md, "High Fidelity: how it is measured"): each on its
own path against No Compromises. Pitch (the keyboard's voltage at 1 V an octave, each
oscillator's timing current, which its frequency follows) within 0.01 cent at every moment,
a shift of up to a sample allowed; the contours within 1e-5 V (-120 dB of 10 V), the same
shift allowed; the audio within 1e-6 of full scale (-120 dBFS) when the pitch and contour
paths are held at No Compromises (`Voice::hold_control_quality`). The goal: one voice in real
time at 256-frame blocks in the worst case, no block late over 2 minutes.

`ca72-lab hifi [--quality high-fidelity|potato] [--scenario NAME]` renders each performance
scenario three times (No Compromises and the mode, both probed each sample; the mode with
the pitch and contour paths held) and fails when a limit is crossed (`ca72_lab::quality`;
`tests/quality.rs` checks the worst case's load in the test suite). The mode is the panel's
`quality` (`voice::Quality`), switchable as the voice plays; in the DAW the Minimoog's
`quality` parameter ("no compromises", "high fidelity", "potato"), set like any other
through the bus, the CLI and MCP (the DAW's API test of the Minimoog plays it in High
Fidelity). Potato is not built yet: it runs as High Fidelity.

| Change | Result | Measured against the limits |
|---|---|---|
| The keyboard circuit's Newton tolerance 1e-5 V (and 1e-6 relative), from 1e-9: a solve seldom needs the second iteration that confirms the first, whose own error is of the order of the correction's square | The heaviest block's iterations 3,830 to 2,344 (the 99th percentile 3,375 to 2,235) | Pitch within 4e-8 cent (the keyboard 2e-9 cent) |
| The preamplifier's and lamp driver's Newton tolerance 1e-5 V (and relative) | The heaviest block's iterations 3,901 to 2,546 | The audio within 4.3e-8 (-147 dBFS; perf-ext) |
| The contour generator's solvers to 1e-9 V (its root finders' and transistors' last step, from 1e-12; the decay's Newton step, from 1e-11); the settled threshold, which skips solves, kept (looser, a slow decay would drift past the limit) | The contours' thread's worst 19.5 to 18.9 us a sample (p99 13.3 to 12.1) | The contours within 1.3e-14 V |
| Tried and dropped: fewer keyboard substeps (4 or 2 a sample) | | Pitch off by 0.2 to 8 cents at key changes: the trigger bus and the hold switch need them |
| Tried and dropped: fewer keyboard substeps while it glides (8 for 2 ms after a contact changes, 1 or 2 after) | In the worst case GLIDE moves every control step, which restarts the 2 ms: nothing saved until GLIDE's changes were let through with fewer substeps too, and then | Pitch off by 0.16 cent (the keyboard) and 0.7 cent (oscillator 1) while GLIDE moves |

**Where High Fidelity stands** (2026-09-29, the machine otherwise idle): every scenario
within the limits, and the goal met. The threaded worst case at 256 frames, paced as an
audio callback runs it (`ca72-lab worst --quality high-fidelity --paced`), 120 s five times:
no block late in any, the slowest 5.08 to 5.27 ms of 5.33 (95 to 99 %), the 99.9th
percentile 4.63 to 4.71 ms. Run back to back (no idle time between blocks, harder than a
callback), 1 late block in 120 s (106 %). No Compromises, paced: 154 late. The margin is
thin: the keyboard circuit's heaviest blocks (eight substeps a sample) remain the slowest
thread (the `profile` build, whose own timing adds about 0.8 us a sample there, shows it at
22.4 us a sample at worst). (2026-09-30: widened with Potato's exact-enough parts; see "High Fidelity candidates
from Potato's work" below.)


## Potato (quality modes; from 2026-09-29)

The goal (the owner's): a hundred voices at once in real time at 256-frame blocks, each
under the worst case's load, fidelity kept as high as possible, measured against No
Compromises (`ca72-lab hifi --quality potato` reports its distances on each path, not judged).
`ca72-lab crowd [--voices N] [--threads T] [--quality MODE] [--paced]` runs N voices under the
worst case's load (a fraction of a second apart in it) on T threads (the machine's less
two), each thread running its voices in turn each block, as a DAW's engine runs its
devices. High Fidelity there: one voice a thread takes 12.7 to 14.5 ms a 256-frame block
(5.33 ms), two 27 ms: a hundred on fourteen threads need each voice about 18 times
cheaper, about 2 us a sample.

**First stage: the circuit models at cheaper settings** (in turn, the worst case's load,
us a sample, High Fidelity to Potato, the `profile` build):

| Setting in Potato | Cost | Against No Compromises |
|---|---|---|
| The keyboard circuit stepped once a sample (not eight times after a contact or GLIDE changes), Newton's tolerance 1e-4 V | Keyboard 6.8 to 1.0 | Pitch off by up to 55 cents at key changes, for 0.4 to 2.6 ms; in the worst case a few cents for up to 49 ms while GLIDE moves |
| The contours at an eighth of the sample rate (not a half), interpolated | Contours 2.1 to 0.7 | Up to 0.82 V at attacks: the contours up to 8 samples late |
| The VCA's bias solved every 16 samples even with EXT. LOUDNESS plugged, and again at once when the jack moves 2 mV (as for the contour) | 6.0 to 1.6, 4.5 with the jack sweeping (the worst case) | The audio within -61 dBFS in the worst case (-11 without the jack's refresh) |
| The preamplifier stepped once a sample (not four times between resamplers); the oscillators at the sample rate and the filter at twice it (not four times), each switched while it plays with resamplers prepared beforehand | Preamplifier 8.4 to 4.1, filter 6.7 to 4.3, oscillators 4.7 to 3.9 | Not measured yet: the resamplers' delay changes, and a plain subtraction (-3 to +0.7 dBFS) measures the shift, not the sound |

In all about 21 us a sample (from 37). The owner noted this stage as potential for High
Fidelity too (2026-09-29): each setting, measured on its own against High Fidelity's
limits, could lighten it where it passes. As measured so far the keyboard's single step
(cents at key changes) and the contours' sub-rate (up to 8 samples late) do not; the
oversampling settings wait for a comparison that allows for the resamplers' delay.

`ca72-lab hifi` now also compares the audio (the pitch and contour paths held) allowing for
a delay (the whole-sample lag within 32 at the cross-correlation's peak, then the largest
difference) and by its spectra (windows of 4096, the magnitude spectra's difference
relative to No Compromises', median and worst window). High Fidelity: no lag, the spectra
the same to rounding (perf-ext -167 dB at worst). Potato's first stage: 24 to 32 samples
early (the oscillators' resamplers left out; 5 in perf-screech and the worst case), the
spectra -41 to -67 dB in the median window (perf-a440 -21, perf-screech -12), the worst
windows -10 to -27 dB, and +10 dB (perf-a440: a triangle at 2', aliased at the sample
rate) and +22 dB (perf-screech: the filter self-oscillating, at twice the rate, not four
times).

**Second stage: tables and cheaper steps from the circuit models** (us a sample in the
worst case, in turn):

| In Potato | Cost | Against No Compromises |
|---|---|---|
| The noise's small-signal model stepped once a sample (not four times and decimated), its loads taken every 512 samples (each rebuilds the model) | 1.53 to 0.39 | The same colouring, its top octave aliased |
| The VCA's bias from a table (`vca::DriveTable`): the full solve settled for each contour (0.05 V steps) with EXT. LOUDNESS empty and each contour and jack voltage (0.1 V steps) with it plugged, with the chain's settled states and the drive's slopes against them, so each sample takes the settled drive to the chain as it is, to first order; made once a process (the first voice builds 0.5 s slower) and shared | 4.5 to 0.10 | Within 0.8 % of the settled solve between its points. Without the slopes the worst case's audio was only -2 dB from No Compromises' by spectra (the jack's 5 Hz sweep moves the chain, and Q1's tail follows it by up to 3 %); with them -45 dB in the median window. The overdrive region (6.2 to 7 V at the jack) has more than one solution, which the full solve's warm start chooses: up and down a slow sweep Q21's tail differs up to five times there |
| Each oscillator's converter from a table (`expo::ConverterTable`): its collector current depends only on one combination of its input current, the + input's source and the trimmer (`ExpoCircuit::drive_u`), so one table of its logarithm on 4096 points (cubic between, at the ramp's two voltages) serves every panel setting; from 1e-11 to 2e-3 A, the full solve outside (above about 1e-2 A the model's converter runs away) | Oscillators 3.9 to 1.1 | Within 0.001 cent of the full solve over the table (`expo::table_tests`) |

The pitch probe of the oscillators was their input current into the summing junction; it
is now their timing current (the converter's output, which the frequency follows), so the
cents are the pitch's. High Fidelity's figures by it: the oscillators within 6e-7 cent in
the worst case (the verdict unchanged). Potato's oscillator pitch follows its keyboard's
transients (8.9 cents where the keyboard's is 9.0); in the worst case oscillator 3 frequency-
modulates oscillator 1 at audio rate, and oscillator 3's waveform at the sample rate differs
from No Compromises', so oscillator 1's instantaneous pitch differs by up to 1730 cents.

Potato in all: about 12.6 us a sample (from 37).

**Third stage: the preamplifier as its paths** (`preamp::Plain`): the input's coupling (C23
against R78, the source and an input resistance of 96.6K), the loop (an open-loop gain of
726 on the feedback node between R61 and R62 with C26's voltage a state, so C26's charge
shifts under an asymmetric clip as the circuit's does), the output clipped at -7.3 and
+9.9 V (a 0.3 V knee), C20 into R46; the lamp from the output's peak held with a time
constant of 0.122 s through a smoothstep from 1.375 to 1.775 V. The input resistance and
open-loop gain are fitted to the circuit's gain from sources of 0, 100K, 250K and 500K
(within 0.1 %); the lamp's threshold and hold to the circuit's (dark at a 1.4 V peak, lit
from 1.78 V; lit 0.21 s after a clipping drive stops). Preamplifier 4.2 to 0.10 us a
sample. Against the circuit: the gain within 0.0 dB from every source, clipping's harmonics
within 0.1 to 0.8 dB (`preamp::plain_calibration` holds the gain within 0.1 dB, the clip
levels within 0.05 V and the lamp's threshold); its treble -0.25 dB from the circuit's at
10 kHz (the loop's pole above the audio band left out). perf-ext's spectra -57 dB from No
Compromises' in the median window (the first window -19 dB: the plain model starts from
silence, the circuit from its operating point). About 8.7 us a sample in all.

**The filter, cheaper steps**: in Potato the ladder's bias from a table (`VcfCircuit::bias`
on 1024 points of the tail current's logarithm, linear between) and Newton's tolerance
1e-6 V (from 1e-10): the filter 4.2 to 2.9 us a sample, its fidelity unchanged. At the
sample rate (not twice it) 2.2 us, but a self-oscillating filter strays (perf-screech's
spectra -0.2 dB from No Compromises' in the median window, not -12): kept at twice. About
7.1 us a sample in all. `ca72-lab crowd`, paced: 28 voices on 14 threads mean 5.3 ms a
256-frame block (396 of 938 late), 50 voices 9.8 ms: a voice among many costs about 10 us
a sample (7 alone), and a hundred need about 3.5 times less.

**The filter's pairs at once**: in Potato each pair's tanh is taken with its series drop
folded into its thermal voltage (tanh(x / (2Vt + b)), `vcf::plain_pair`; the drops are
0.1 to 0.2 % of 2Vt), not solved behind it, and each step takes at most two Newton
iterations to 1e-4 V: the filter 2.9 to 1.4 us a sample, the fidelity as before on every
scenario. With one iteration (1.1 us) the self-oscillating filter strays (perf-screech's
spectra -5 dB from No Compromises' in the median window, not -12). About 5.7 us a sample
in all.

**The keyboard at an eighth of the rate** (the owner's choice of 8 over 4): in
Potato the keyboard circuit takes one step of 167 us every 8 samples (at 48 kHz), its
contacts read at the step, its output interpolated between steps, as the contours already
were. The contacts wait up to 7 samples (0.15 ms) and a pitch change ramps over 8 samples
(0.17 ms): a note starts on the same 8-sample grid as its contours, and Potato's audio is
still 5 to 32 samples ahead of No Compromises'. The worst case's mean block 1.19 to 0.99 ms
(4.7 to 3.9 us a sample, the plain build, in turn). `ca72-lab hifi --quality potato`, the
keyboard's pitch against No Compromises, stepping once a sample then once in 8 (largest;
longest stretch beyond 0.01 cent):

| Scenario | Once a sample | Once in 8 |
|---|---|---|
| v0-bass | 9.0 cents; 0.5 ms | 194 cents (a key change's jump, reached later); 1.2 ms |
| v0-three | 0.57 cents; 2.6 ms | 2.0 cents; 833 ms |
| v0-board3 | 55 cents; 0.4 ms | 92 cents; 1.0 ms |
| perf-ext | 4.5 cents; 0.4 ms | 92 cents; 1.0 ms |
| perf-a440 | 0.7 cents; 0.4 ms | 42 cents; 0.9 ms |
| perf-screech | 0.004 cents; 0 ms | 2.6 cents; 4.8 ms |
| worst | 9.2 cents; 49 ms | 349 cents; 77 ms |

v0-three's long stretch is timing, not the step: a key released while GLIDE still moves
is held where the pitch was when the hold switch opened, which the contacts' wait moves by
up to 0.15 ms; the held pitch is then up to 2 cents off, and the next press glides back
from it at GLIDE's pace. Tried and dropped: the glide resistor set so that a backward
Euler step decays C6 as the circuit's exponential does (half a step's lag taken out):
v0-three unchanged, perf-screech's and the worst case's stretches longer (11.8 and 154
ms). Not a High Fidelity candidate (its timing is within a sample).

**Oscillator 3's reverse sawtooth stage for Q37 alone** (`revsaw::Plain`): the same
equations as the nodal solve (C10, Q37's currents behind RB, RE and RC, its emitter
junction's depletion and diffusion charges and its collector junction's depletion charge,
all backward Euler as the solver steps them), with the linear network folded around Q37's
two junction voltages, which Newton's method solves with the solver's junction limiting,
to 1e-9 V. Within 0.2 uV of the circuit along sawtooths from 20 Hz to 5 kHz and 1.7 uV in
the sample of a reset (`revsaw::tests`); without the junctions' charges it was 0.6 mV off
along a 440 Hz ramp (the Miller capacitance R175 multiplies) and 1.4 mV at the resets. The
worst case's mean block 0.99 to 0.93 ms; Potato's `ca72-lab hifi` figures unchanged. A High
Fidelity candidate (the circuit's solution to well under its limits).

**The goal lowered to about 50 voices** (the owner, 2026-09-29): the same load, 256-frame
blocks. Then: 50 voices on 14 threads, paced, mean 5.9 ms a block (4580 of 5625 late).

**Where Potato's time goes, counted**: `ca72-lab worst` built with `--features count` counts
the elementary functions' calls a sample by call site (`ulp::calls`), and the sampler skips
a program's setup (`SPROF_SKIP_US`): the VCA's Potato table, built first, had been half of
a 30 s run's `exp` calls. About 93 calls a sample, and from them:

- The knob laws (`audio_taper`, the MODULATION wheel's: a `pow` or two each) taken again
  only when their knob moves (4.5 `pow` a sample), the same values; the lamp's hold factor
  worked out once.
- The contours' transistor parameters at the temperature (`Bjt::at`: a `log` and three
  `exp`) worked out once per temperature, not at each of the followers' root finder's
  evaluations: the same values (No Compromises unchanged to the bit).
- The noise generator's model without its transistors' RB, RE and RC (RE added to each
  emitter resistor, where it sets the gain): 22 states instead of 34, a step's 1156
  multiply-adds to 484; its three outputs within 0.008 dB and 0.015 degree of the circuit's
  model from 10 Hz to 20 kHz (`tests/noise.rs`). A switch carries the nodes' state.
- The filter's resamplers at twice the rate with the halfbands' zero taps (their sin(pi k /
  2) about 1e-17 of the rest, not exactly zero, so kept in the other modes) exactly zero
  and skipped, and the interpolators' stuffed zeros' outputs one tap: 395 multiply-adds a
  sample to about 123, within 1e-14 of the full filters (`tests/resample_factors.rs`).
- Tried and dropped: the filter's pairs' tanh from an anchor (`PairWarm`): 19 libm calls a
  sample to 10.5, no time saved.

The worst case's mean block 0.93 to 0.81 ms (3.2 us a sample).

**The VCA's signal path and the contours' slowest ticks** (Potato):

- The VCA's pairs solved to 1e-9 (not 1e-15), and the output pair's Early effect (a fixed
  point in the pair's split: about 0.02 of a change comes back a pass) by Newton steps from
  the last sample's split, its slope worked out with it (a second step when the first
  moves it by more than 1e-3), not four passes from rest: within 0.15 uV of the full path
  on sines and a sawtooth's resets (`vca::plain_signal_follows_the_full_one`); one plain
  pass from the last sample's nodes had been 31 mV off at 5 kHz, two 0.6 mV, three 13 uV.
  A High Fidelity candidate.
- The contours' trigger section woken only by what it reads (the key, EXT. S-TRIG, the
  temperature), not by every panel change; the dump node (DECAY off) from its last
  solution with its slope analytic, not from V-trig's voltage by differences: its
  bisections from there made the slowest ticks (V-trig 5 to 6 us a sample in the worst
  blocks). Where the diode law holds its exponent (beyond 80 thermal voltages) it reports
  no slope, so the root finder bisects: with the held law's slope a Newton step there
  looked converged and a section's capacitor ran away.

One voice: mean block 0.74 ms, p99.9 1.46, worst 1.84 (from 0.81, 2.08, 2.56). 50 voices on
14 threads, paced: mean 4.72 ms, worst 6.97 (736 of 5625 late). Potato's `ca72-lab hifi`
figures unchanged but the idle scenario's audio (-212 to -157 dBFS).

**The crowd as the engine runs it** (`ca72-lab crowd --pool`): the DAW's engine's worker
pool runs a stage's tracks by letting its workers and the calling thread
claim task indices from one counter, so the crowd's fixed assignment (each thread its own
voices: 14 threads, eight of them with 4 voices, some cores with 8) was more pessimistic
than the DAW. In the pool mode each block every voice is a task that `threads - 1` workers
and the calling thread claim (by default the machine's hardware threads less one).

Then, in Potato:

- The tables every voice made the same of made once a process and shared
  (`shared::Shared`): each oscillator's converter table, the ladder's bias table and the
  filter's control node's table (every mode's; the same values). A voice's memory 735 to
  365 KB; the crowd's time unchanged (the caches were not what limited it).
- The contours' diode laws kept per temperature, like their transistors' (the same
  values); the oscillators' cache of solves looked in only when the converter's table has
  no answer.
- The filter: at most two Newton iterations, the second's solution then taken to first
  order as a converged one's (not evaluated a third time); the loop's elimination by its
  pivots' reciprocals. One iteration with that first-order finish lets the
  self-oscillating filter stray (perf-screech's spectra -5 dB from No Compromises' in the
  median window, not -12): kept at two.
- The contours' Q20 and Q12 solved from their last solve within the tick, not the last
  tick's, and their steps limited as the nodal solver's (SPICE's pnjlim), not to 0.2 V
  down: V-trig's slowest ticks (a collector junction swinging volts in steps of 0.2 V, from
  the tick's start at each evaluation) gone from the slowest blocks.
- Tried and dropped: the contours at a sixteenth of the rate (5.5 % saved, the envelopes
  coarser); their tolerance 1e-6 or 1e-5 (under 2 %); a table-driven `exp` (twice libm's
  speed independently, no faster in a dependent chain, which is how Newton's iterations
  use it).

One voice: mean block 0.71 ms, p99.9 1.30, worst 1.65. 50 voices as the engine's pool,
paced, 30 s: on 16 threads none late (mean 3.90 ms, worst 5.29); on 15, 8 of 5625 late
(mean 4.04, worst 5.50). The slowest blocks left are the keyboard's at key changes (some
260 Newton iterations a block at the 167 us step).

**The keyboard's key changes and Potato's exponential; the goal met** (Potato):

- The keyboard's steps start from their extrapolation only when it moves no node more than
  50 mV (`mna::Circuit::predict_limit`): after the jumps a key change makes, the quadratic
  extrapolation overshot and Newton took 50 to 80 limited iterations from there; and a
  step fails after 16 iterations, not 100 (`step_iterations`), to be halved (two halves
  converge in fewer). Its outputs the same to the digits shown; the largest pitch
  differences at key changes, which are the jumps' transients, moved (v0-bass 194 to 120
  cents, the worst case 349 to 281).
- Potato's elementary functions (`fast`): an exponential from a table of 2^(j/64) (correctly
  rounded constants) and a fifth-degree polynomial, within 2 ulp of libm's, and tanh from
  it, within 2e-14 (`fast::tests`); plain arithmetic on the bits, the same on every
  platform. libm's tanh took 9.6 ns (30 in a dependent chain), this 4.5 (16). The filter's
  pairs and the converter tables use them directly; every other `ulp::exp` in a Potato
  part's tick is Potato's through a scope each part holds for its tick
  (`ulp::PotatoScope`), so the other modes, and a voice's making and settling, keep libm's
  (No Compromises unchanged to the bit; `ulp::tests` checks the scope's restoring, a panic
  included). One voice's mean block 0.73 to 0.64 ms, p99.9 1.27 to 1.04.
- Tried and dropped: one filter iteration from a second-order (Adams-Bashforth) predictor
  (perf-screech's spectra +2.6 dB from No Compromises' in the median window).

**The goal met** (2026-09-29): 50 voices as the engine's pool, paced, 120 s at 256-frame
blocks on 15 threads, each under the worst case's load: none of 22500 blocks late (mean
3.78 ms, p99 4.53, worst 4.96: 93 % of the block). On 16 threads, 60 s: mean 3.66, worst
4.84. The fixed assignment (14 threads, each its own voices), 30 s: mean 4.12, 1 late. One
voice in turn: 0.06 to 0.11 of real time on the scenarios (No Compromises 0.39 to 1.94).
Against No Compromises (`ca72-lab hifi --quality potato`, reported): the keyboard's pitch at
key changes off by up to the jump for a millisecond; the contours up to 8 samples late
(0.82 V at attacks); the audio 5 to 32 samples early (its resamplers' delay), then its
spectra -41 to -55 dB from No Compromises' in the median window (perf-a440 -21: a
triangle at 2' aliased at the sample rate; perf-screech -12: the filter self-oscillating at
twice the rate).

An example for the owner: `crates/ca72-lab/patches/demo.json` (a bass riff, a
gliding lead, a resonant sweep; 15.5 s) played by `ca72-lab play <patch> <out.wav>
[--quality potato]`: Potato renders it in 1.4 s, No Compromises in 7.7 s; their levels
within about 1 dB second by second, Potato's audio 29 samples ahead.

**Memory, by mode** (2026-09-30, measured with a counting allocator and the process's
resident size): the same in all three, since each voice keeps every mode's parts so that
the mode switches while it plays without allocating. Once a process, about 8.4 MB: 7.2 MB
of it Potato's VCA table (the settled drive for each contour and EXT. LOUDNESS voltage,
made at the first voice even if Potato is never chosen, with about 0.5 s of work), the rest
the shared tables and the prototype voice. Each voice about 410 KB (380 KB of heap, 32 KB
inline): the keyboard circuit 93 KB, the preamplifier's circuits 136 KB (idle in Potato and
whenever EXT is off), the noise's two models 87 KB, the reverse sawtooth stage's circuit 37
KB, three oscillators 28 KB, the filter 15 KB. 50 voices: 31 MB resident in every mode (13
MB with one). A voice for Potato alone would need about 160 KB; a process without Potato
7.2 MB less.

**The 50 voices in the DAW** (2026-09-30; the fix approved by the owner): the DAW's
device runs each instance as the threaded voice, five
worker threads of its own besides the engine's. In Potato the threaded voice now runs its
parts in turn on the caller's thread (`Threaded::process_in_turn`: each part taken from
its worker's lock, which is free between passes; the workers stay parked), so the engine's
pool spreads the instances; in the other modes the workers run as before, and the mode
switches either way while it plays (`tests/threaded.rs`: the same samples as the plain
voice to the bit, and no pass to the workers in Potato). Measured in the DAW's engine
(a benchmark run by hand): 50 tracks, each a Minimoog in Potato with
every oscillator and the noise on, oscillator 3 modulating the oscillators and the filter,
GLIDE on, a resonant filter, playing sixteenths at 120 BPM a fiftieth of a beat apart;
compiled with the pool on 15 threads at 256-frame blocks in real-time mode, the calling
thread real-time as the audio device's is; paced 120 s: none of 22500 blocks late (mean
4.31 ms, p99 4.66, worst 4.78). One instance under that load: about 0.73 ms a block, and
the engine 0.16 ms of its own (this load is about 15 % heavier a voice than `ca72-lab
worst`'s). Without the device's change the same 50 instances ran more than 20 times slower
than real time (250 workers handing samples along). Without the calling thread's real-time
priority, 4 % of blocks were late (stalls up to 11 ms whatever the count of voices).

**High Fidelity candidates from Potato's work**, measured one by one (2026-09-30, approved
by the owner; `ca72-lab hifi` for the limits, the threaded worst case paced for the
margin):

| Candidate | Against High Fidelity's limits | Kept? |
|---|---|---|
| The fast exponential (`fast::exp`) in every part's tick (`ulp::FastScope`) | Audio -196 to -281 dBFS (it had been No Compromises' to the bit); pitch and contours unchanged to the digits shown | Yes: every thread lighter (the keyboard's worst 20.3 to 19.1 us a sample, the contours' 18.4 to 15.7, the VCA's bias 17.2 to 14.5); in turn 8.8 to 8.0 ms a block |
| The keyboard's extrapolated starts limited to 50 mV | Within | No: at eight substeps the extrapolation is right, and the check cost more (the keyboard's 99th percentile 13.7 to 17.8) |
| The contours' lean solves (Q20 and Q12 from their last solve with pnjlim, the dump node warm with its slope, the trigger woken by its own inputs) | The worst case's contours 4.8e-6 V (limit 1e-5): No Compromises' own error, not High Fidelity's (its held steps hit their 60-iteration cap 447 times in the worst case's 3 s; High Fidelity's never). Since No Compromises was fixed (below), 1.8e-15 V | Yes: the contours' thread 99th percentile 10.4 to 5.5-6.9, worst 15.7 to 7.7-9.5 |
| Oscillator 3's reverse sawtooth stage for Q37 alone | perf-screech's audio -115.8 dBFS: past the limit (its microvolts at each reset through a self-oscillating filter) | No |
| The VCA's signal path by Newton steps (to 1e-9) | Audio -142 to -157 dBFS | Yes |
| The filter's elimination by reciprocals | Unchanged to the digits shown | Yes |
| The sparse resamplers | (1e-14 of the full; not built for High Fidelity) | No: about 1 % in turn, none of it on the keyboard's thread, which sets the margin |

Together, alternating with the build before under the same load (the owner's browser and
chat applications running, load 2.7), the threaded worst case paced, 60 s twice each: before
18 and 124 blocks late (worst 5.91 and 8.23 ms, 99.9th percentile 5.45 and 5.69); after none
(worst 5.32 and 5.26, 99.9th percentile 4.99 and 4.88). The keyboard's thread 99th
percentile 18.8-20.1 to 16.5-17.1 us a sample, worst 22.4-29.6 to 20.0-20.1; the contours'
13.3-14.0 to 6.5-6.9, worst 19.5-20.6 to 8.0-9.5. Paced 120 s twice (the plain build): none
late, the slowest 4.92 and 5.04 ms. High Fidelity's audio now within -142 dBFS of No
Compromises' at worst (perf-ext), its pitch and contours as before but the worst case's
contours (above). Potato is unchanged (it had all of these).

**No Compromises' solves that stopped short, fixed; the reference re-recorded**
(2026-09-30, the owner's decision). Every iterative solve that should converge
now counts the times it stops at its cap (`ca72::unconverged`: the contours' root
finders, transistors and decay step, the series junctions, the differential pairs, the
VCA's junctions and bias, the converters, the filter's node and loop; the nodal solver's
circuits count their own `failed`). Before the fix, No Compromises stopped short in every
scenario with notes: Q20's and Q12's solves (steps held to 0.2 V down, 2 Vt up) 20 to 447
times a render, the decay step twice in perf-ext (in every mode). Everything else, never.
Fixed: Q20 and Q12 limited by pnjlim in every mode (as the nodal solver's), the decay step
allowed 100 iterations (its node's steps held to 0.2 V; 30 were too few for a jump of
volts). Now none stops short in any scenario or mode (`tests/solves_converge.rs` in all
three; `ca72-lab hifi` and `ca72-lab perf check` fail on one, and `perf record` refuses to
record). No Compromises against its old reference: perf-idle the same; perf-ext -270
dBFS, perf-a440 -192, v0-bass -186, v0-three -181, v0-board3 -164 (rounding carried on
from the first changed solve); the worst case -114 and perf-screech -101 (the stalls'
own microvolts, the self-oscillating filter carrying them as a slight shift of phase).
High Fidelity now within 1.8e-15 V of it at the contours (from 4.8e-6). Its speed
unchanged (in turn 27.1 s against 27.0; threaded, alternated under the same load, the
same). The reference re-recorded with its twins; perf-screech is no longer chaotic (its
twins' spread 9.5e-8, from 1.1e-7: the stalls had made it more sensitive), so the
stricter rule holds it; the old reference kept as
`target/mm-perf/nc-ref-vca-until-2026-09-30`.

**Next:** the first stage's settings measured one by one as High Fidelity's candidates;
then, since the circuit models
have floors of their own (a Newton solve a sample for the keyboard and the preamplifier,
an implicit ladder, a converter solve a sample for each oscillator), explicit models for
Potato, each from its circuit model's own curves and time constants and measured against
No Compromises: the preamplifier, the filter, the oscillators, the VCA, the keyboard, the
noise, the contours.

## Several voices at once (2026-09-30)

A No Compromises or High Fidelity voice runs on six threads (five workers and the audio
thread that plays it), all at the audio threads' priority. Measuring several at once in
the DAW's engine (a benchmark run by hand), three No Compromises voices stalled the
machine: the parts waited for each
other by spinning, and with more of them than CPUs a spinning part kept the part it waited
for off its CPU, starving the kernel beneath them (real-time throttling is off here).
Waits are now bounded (`threaded::Backoff`: spin 20 us, yield until 200 us, then sleep),
and a watchdog makes the process's real-time threads ordinary if a CPU's ordinary threads
starve for 500 ms (`ca72_rt`; history.md).

After the fix, the engine at 256-frame blocks, 20 s a run, paced, the Linux reference
machine (blocks late of 3750; none demoted by the watchdog):

| Mode | 1 voice | 2 voices | 3 voices | 4 voices |
|---|---|---|---|---|
| No Compromises | 2 to 50 (mean 2.5 ms) | 150 (mean 3.2 ms) | 376 (mean 3.8 ms) | - |
| High Fidelity | 0 | 0 (mean 2.8 ms) | 0 (mean 3.2 ms) | 0 (mean 3.9 ms) |

Before the fix, High Fidelity had 1 and 28 late with 2 and 3 voices; one No Compromises
voice is the same within the machine's noise (alternating runs with the old waits: 15 and
130 late, with the new: 2 and 50). So on this machine one No Compromises voice or four
High Fidelity voices play in real time at 256 frames, and about 50 Potato voices.
