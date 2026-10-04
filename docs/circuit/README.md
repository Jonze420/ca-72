# The circuit model

The CA-72's voice is a circuit-derived emulation of a Minimoog Model D of about 1972-73
with the CA3046 ("old") oscillator board. Start with [plan.md](plan.md); the decisions
made along the way are in [history.md](../history.md). These documents were written while
the model was a device of the digital audio workstation (DAW) it was developed in, and
still mention that DAW here and there.

| Document | Contents |
|---|---|
| [plan.md](plan.md) | The three concerns (specification, offline reference, real-time), stages, acceptance |
| [sources.md](sources.md) | Every source, tagged, with the manifest of URLs and hashes |
| [manifest.md](manifest.md) | The revision manifest: which drawing for which board, and why |
| [board1.md](board1.md) | Board 1 (oscillators): transcription, how it works, the real-time oscillator, discrepancies |
| [board4.md](board4.md) | Board 4 (mixer, filter, VCAs, external preamplifier, A-440): transcription, the real-time filter, converter, VCA, preamplifier and A-440, the filter's factory calibration |
| [board2.md](board2.md) | Board 2 (contour generators, keyboard circuit): transcription, the real-time contours |
| [board3.md](board3.md) | Board 3 (noise, modulation mix, regulators) and the wheels: transcription, the real-time noise and modulation, the rails' measurements |
| [voice.md](voice.md) | The voice: the real-time models wired as the instrument, renders from patches |
| [numerics.md](numerics.md) | Numerical studies: integration, oversampling, prewarping, tables, control rates |
| [no-compromises-attempts.md](no-compromises-attempts.md) | What was tried to make No Compromises real time, what worked and what did not, ideas left |
| [components.md](components.md) | Device models, their provenance and confidence |
| [assumptions.md](assumptions.md) | Simplifications, what they leave out, when they are revisited |

## Running the circuit lab

ngspice 47 (built from its source release into `~/.local/opt/ngspice-47`):

```
curl -L -o ngspice-47.tar.gz https://downloads.sourceforge.net/project/ngspice/ng-spice-rework/47/ngspice-47.tar.gz
# sha256 894e649651f1838a14095e5a5439e7d3aa63e87ede14d283173fda4fcdef675f
tar xzf ngspice-47.tar.gz && cd ngspice-47 && mkdir release && cd release
../configure --prefix=$HOME/.local/opt/ngspice-47 --with-readline=yes --enable-openmp --without-x --disable-debug
make -j12 && make install
```

On macOS, `brew install ngspice` also works while Homebrew's is version 47; `CA72_NGSPICE` names a binary explicitly.

```
cargo run -p ca72-lab --release --bin ca72-lab -- calibrate          # Folkman 1973
cargo run -p ca72-lab --release --bin ca72-lab -- calibrate norlin   # Norlin's page
cargo test -p ca72-spice -p ca72-lab                                # needs ngspice
cargo test -p ca72                                                 # the real-time models alone
cargo run -p ca72-lab --release --bin ca72-lab -- play <patch.json> <out.wav>  # voice.md
```

Tests that need ngspice say `SKIPPED` without it, unless `CA72_REQUIRE_NGSPICE` is set,
when they fail. Work files go to `target/ca72-lab/`.
