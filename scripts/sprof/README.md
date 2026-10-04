# sprof: a sampling profiler that needs no kernel setting

`perf` and `samply` need `/proc/sys/kernel/perf_event_paranoid` at 1 or lower; on the Linux
reference machine (Ryzen 7 7800X3D) it is 2, and machine settings are the owner's. This
profiler is preloaded into the program instead. Every 1 ms of CPU time (the kernel's timer
resolution) a `SIGPROF` handler records the interrupted instruction pointer. At exit the
addresses go to a file, and `sym.py` symbolizes them with `llvm-symbolizer`. It records
only the innermost frame, with its inlined callers inside the same function, not full
stacks.

```bash
gcc -O2 -shared -fPIC -o /tmp/sprof.so scripts/sprof/sprof.c
```

```bash
BIN=$(cargo test --release -p ca72 --lib solver_parts --no-run 2>&1 | grep -oE "target/release/deps/ca72-[0-9a-f]+" | head -1)
```

```bash
MM_BENCH_REPS=300000 SPROF_OUT=/tmp/pcs.txt LD_PRELOAD=/tmp/sprof.so $BIN solver_parts --ignored --nocapture
```

```bash
python3 scripts/sprof/sym.py $BIN /tmp/pcs.txt
```

`SPROF_SKIP_US` leaves out the samples in a program's first microseconds of CPU time (a
voice's setup, say: `ca72-lab worst` builds the VCA's Potato table first, which otherwise
shows as a large share of `exp`). `MM_BENCH_REPS` lengthens the keyboard bench's eight-substep loop so that it dominates the
samples. The release profile keeps debug information (`debug = 1`), so lines and inlined
functions resolve. Addresses outside the executable (the C library's `memset` and
`memcpy`, say) show as `??`.
