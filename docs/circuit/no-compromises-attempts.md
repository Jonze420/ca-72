# No Compromises: what was tried for real time

A list for a fresh look (2026-09-29): what was done to make the Minimoog model run in real
time in its No Compromises mode, what worked, what did not, and what was left. The full
record, with every figure, is in [numerics.md](numerics.md) ("No Compromises") and
[history.md](../history.md). The owner stopped the optimisation here, judging it optimised
enough; these notes are for ideas, not a plan.

## Context

- **The instrument:** a Minimoog Model D modelled from its circuit, in Rust
  (`crates/ca72`).
- **The goal (No Compromises mode):** one voice in real time at 256-frame blocks
  (5.33 ms at 48 kHz) under a worst-case load (`ca72-lab worst`). That load has every knob
  moving, the external input on, every rear jack driven, and fast notes with glide.
- **Machine:** Ryzen 7 7800X3D, 8 cores / 16 threads on one chiplet, Linux (the Linux
  reference machine).

**Constraints**

- Output must stay within 1e-7 of full scale of a recorded reference. `ca72-lab perf check
  target/mm-perf/nc-ref-vca` judges this.
  - Renders that are chaotic under rounding may differ by as much as their own rounding
    spread. `ca72-lab perf record` built with `--features twins` measures that spread by
    nudging every elementary function one ulp up and down.
- In practice changes must be bit-identical. Changing only the rounding (dividing by
  multiplying with reciprocals, say) already pushed the worst case to 1.29e-7, past the
  limit.
- The keyboard's stepping policy and Newton tolerance are part of the reference, so
  changing them moves the output far beyond 1e-7:
  - 8 substeps a sample for 2 ms after a key contact changes, and while the output moves
    more than 1 mV a sample;
  - backward Euler;
  - Newton stops when every |dx| ≤ 1e-9 + 1e-9·|v| and no junction was limited.
- `unsafe` is denied workspace-wide.
- Math uses the `libm` crate (FreeBSD-style `exp`, which contains a division) so results
  match across platforms.
- The DAW the model was developed in runs devices on a 32-sample control grid, so the
  voice is driven in 32-sample passes.

## Where it stands

- **In turn** (one thread): 2.01× real time, down from 3.52×.
- **Threaded, 6 threads** (`crates/ca72/src/threaded.rs`):
  - 512-frame blocks: real time (1–4 late blocks of 11,250 over 2 min).
  - 256-frame blocks: 0.13–0.5% of blocks late.
- **The only thread that still overruns is the keyboard circuit**
  (`crates/ca72/src/keyboard.rs`, solved by the general nodal solver in
  `crates/ca72/src/mna.rs`), in its heaviest blocks: up to about 24 µs a sample
  against the 20.8 a sample lasts.
  - Circuit size: 31 solved nodes, 58 parts (36 resistors, 3 capacitors, 5 diodes, 6
    junction capacitances, 3 diffusion charges, 3 BJTs, 2 JFETs). It is one connected
    component; 14 of its nodes touch only linear parts.
  - Per Newton iteration, about 1.33 µs all-in (1.43 before the third pass): load about
    0.76, sparse LU about 0.43 (230 multiply-adds), step bookkeeping the rest.
  - Solves take 1 iteration 91% of the time, 2 iterations 8.5%. Pivots move in 0.07% of
    solves.
- **Profile of the keyboard at 8 substeps a sample:**

| Share | Where |
|---|---|
| 30% | elimination |
| 19% | device load |
| 12% | `exp` |
| 10% | stamping |
| 8% | Newton bookkeeping |
| 6% | junction charge series |
| 6% | unresolved (likely memset/memcpy) |

## Tried: kept

**First pass: numerics changed within the rule (~3e-9 of full scale)**

1. **Quadratic Newton predictor** from the last three accepted points: 1.98 → 1.09
   iterations a solve. Linear extrapolation gave 1.27; scaling it for a changed step gained
   nothing.
2. **Accepted charges** taken as q + C·dv from the last load, not evaluated again.
3. **One exponential for a transistor's `exp` and `expm1`.**
4. **Junction depletion charge `a^-m`** from a nearby exact value via a degree-6 binomial
   series.

**First pass: bit-identical**

5. **Stamp positions and the Jacobian's pattern** worked out once, not every load.
6. **Sparse LU structure recorded and replayed** while every pivot stays the one recorded;
   re-recorded from the first column whose pivot moves.
7. **Parts taken out of their Vec for the load loop** (no per-part copies); resistor
   conductances cached.
8. **Voice split across five threads:** keyboard, contours, preamp, back (filter + VCA),
   and the caller running the front. Streamed sample by sample through atomics.

**Second pass, all bit-identical**

9. **Flattened LU replay:** a flat list of the additions, without the bookkeeping or the
   updates that only zeroed entries below the pivot. Linear solve 0.50 → 0.43 µs.
10. **Depletion constants precomputed per part:** the series coefficients, plus SPICE's
    F1–F3 beyond FC·VJ. Those cost two `pow` calls at every load for every forward-biased
    junction. Load 1.00 → 0.79 µs.
11. **`accept` borrows parts instead of copying** them (a part is 120 bytes): about 2.5%.
12. **VCA bias split from its signal path.** The bias solve was 90% of the VCA's time and
    reads nothing of the audio. `Vca::control` / `Vca::signal` / `VcaDrive` hand 20 values
    a sample to the back.
13. **VCA bias on its own (6th) thread,** fed the loudness contour directly by the
    contours thread (a worker can now feed another worker). Late blocks at 256: about 3%
    → 0.13–0.34%.
14. **Per-thread input/output widths** via const generics.

**Third pass, from a second agent's review (bit-identical)**

15. **Newton's bookkeeping trimmed** (the review's first idea):
    - the residual's maximum worked out only when a solve fails (it is only reported);
    - a solve's limiting started only for the 10 parts that limit (diodes, transistors,
      JFETs);
    - a step's charges accepted only for the 17 parts that hold one (every other part's
      charge stays 0);
    - the saved and history voltages swapped instead of copied.

    Eight-substep bench 12.24–12.28 → 11.42–11.79 µs a sample (about 7%); in turn 2.06 →
    2.01× real time. Threaded, 60 s at 256 frames, the clean runs of alternating pairs:
    mean 3.28–3.29 → 3.14–3.20 ms, 99th percentile 5.00–5.05 → 4.88–4.90, 13 and 21 →
    8 and 19 late blocks. It helps; it does not close the gap.

## Tried: dropped

1. **Reciprocal products instead of divisions** (transistor constants, LU pivots): no
   speed gain, and past the 1e-7 limit.
2. **Constant Jacobian base** (GMIN plus resistor slopes; 96 of 102 resistor stamps land
   before any nonlinear stamp at their entry): no gain. The resistors' cost is their
   residual, not their Jacobian adds.
3. **Column-at-once LU replay** (all of a column's factors first, then one flat loop):
   slower. The LU seems bound by latency along a chain of divisions (each column's pivot
   waits for the last), not by loop overhead.
4. **`Bjt::at` memoised per thread** (temperature scaling: a log and three exps): no gain.
5. **An approximate `exp` from a nearby exact value** (first pass): past the limit
   (1.3e-7).
6. **Link-time optimisation** (fat LTO, one codegen unit; the review's fifth idea):
   bit-identical, under 1% in turn (2.01–2.03 → 2.00× real time). Not worth slower builds
   of the whole workspace.
7. **`-C target-cpu=native`** (Zen 4; the review's fifth idea): bit-identical but slower
   over all the scenarios (28.2 s against 26.9 s), whatever the keyboard bench showed.
   Also machine-specific.
8. **Helper thread for the keyboard's Newton iterations.** It evaluated some parts while
   the keyboard thread did the rest, then every stamp was added in part order.
   Bit-identical, but slower whatever the partition:
   - 13.6–16.0 µs a sample against 13.1 in the bench;
   - in the threaded worst case, 97 late blocks against 38.
   - A measured core-to-core round trip with the payload is about 240 ns (95 ns with
     almost none), as much as the work it moves.
   - The code was kept aside, uncommitted, in the DAW the model was developed in; it is
     not in this repository.

## Findings that matter

- **The machine is not always quiet:** runs of `ca72-lab worst` taken minutes apart can
  differ by hundreds of late blocks (a 60 s run: 13 late, then 460 with the median block
  from 3.2 to 4.0 ms) when something else runs. Compare changes by alternating runs, and
  trust the in-turn timing (`ca72-lab perf check`) for small differences.

- **Thread placement:** threads pinned to separate cores had about a quarter of the late
  blocks of two threads on one core's SMT pair (at five threads: 126–140 against 454
  late). The DAW does not pin its threads; `MM_PIN=1,2,3,…` pins the benchmark's.
- **An earlier "keyboard at 33–36 µs a sample" figure was inflated** by SMT sharing; in
  turn its heaviest blocks are 14–21.
- **Where the VCA bias solve's time goes:** 42% `exp`, 28% the tail-pair Newton solve, 17%
  transistor currents. That is inherent to the model's iterations.
- **The `libm` crate's `exp`** has a division and a `scalbn` call. An inlined `scalbn` was
  estimated at about 2%.

## Not tried (ideas left)

- **One block of lookahead:** the threads ahead of the audio path (keyboard, contours,
  bias, preamp) run a block ahead. Samples stay bit-identical, one block later, reported
  as latency. It would also remove the per-32-sample pass fill/drain overhead. The owner
  was offered this and chose to stop instead.
- **Zero only the Jacobian entries in use** (pattern plus fill-in, row swaps accounted
  for) instead of the whole 31×31 matrix: about 5% estimated.
- **Bit-identical re-implementation of `libm`'s `exp`** with `scalbn` inlined: about 2%
  estimated.
- **From the second agent's review, not built:** a generated straight-line kernel for the
  keyboard's topology (stamping and LU replay with fixed indices, falling back to the
  general solver when a pivot moves; 5–15% guessed); reusing elimination arithmetic that
  depends only on constant values (14 of 65 factors; 2–5% guessed); reusing a transistor's
  forward exponential for its diffusion charge when the argument's bits match (no gain
  measured); interleaving independent device evaluations on one core (guess); and, not
  exact, a fill-reducing node ordering (156 → 101 matrix updates, 65 → 53 divisions on one
  captured Jacobian) or fused multiply-adds.
- **Pinning the Minimoog's threads to separate physical cores inside the DAW:** the
  owner's call.
- **Non-exact options, which would need the owner to move the reference:** a different
  substep policy or integrator, eliminating the 14 linear-only nodes (Schur complement),
  modified Newton reusing the Jacobian.

## Tools

- **Worst-case benchmark:** `ca72-lab worst [--block N] [--seconds S] [--serial]`. Built
  with `--features profile` it adds per-thread 99th percentile / worst, and the blocks each
  thread alone overran. `MM_PIN=c0,c1,…` pins the caller and each worker.
- **Solver statistics:** `ca72-lab solvers` gives iterations, the iteration histogram,
  eliminations re-recorded, and keyboard subdivisions.
- **Fidelity:** `ca72-lab perf check target/mm-perf/nc-ref-vca` is the pass/fail rule. The
  reference is not in git; it is recorded with twins from the build of that day's model,
  and again (2026-09-30, the owner's decision) once the contours' transistor solves
  converged: its old copy is `target/mm-perf/nc-ref-vca-until-2026-09-30`.
- **Keyboard bench:** `cargo test --release -p ca72 --lib solver_parts -- --ignored
  --nocapture` (load and LU per iteration, the load by kind of part, the eight-substep
  regime; `MM_BENCH_REPS` sets its length).
- **VCA bench:** `vca_parts`, run the same way.
- **Profiling:** `scripts/sprof/` is a SIGPROF sampler preloaded with
  `LD_PRELOAD`, plus a symbolizer script, because `perf_event_paranoid` is 2 and machine
  settings are the owner's. Its README has the commands.
