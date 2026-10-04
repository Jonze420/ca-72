# Contributing to the CA-72

Bug reports, measurements and fixes are welcome.

- **Bugs:** open an issue with your system, host and its version, the format (VST3 or
  CLAP), the CA-72's version, and what you did. A crash report or the host's log helps.
- **Changes:** open a pull request. Before you do, build and test as the README describes
  (`cargo xtask bundle ca72-plugin --profile bundle`, `cargo test --workspace`), and run
  `cargo fmt --all` and `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  (on Linux `--all-features` needs the ALSA, JACK and OpenGL headers as well:
  `libasound2-dev libjack-jackd2-dev libgl-dev`). CI builds, tests and runs clippy so, with
  the validators, on macOS, Windows and Linux.
- **The sound:** the voice is derived from circuit simulations and tested against them
  (`docs/circuit/`). A change that alters the samples needs its reason and a measurement.
  `crates/ca72-plugin/tests/preset_render.rs` compares every factory preset with the code
  before it, run by hand: on the code before your change,
  `CA72_RENDER_OUT=<dir> cargo test --release -p ca72-plugin --test preset_render -- --ignored --nocapture`,
  then on yours the same with `CA72_RENDER_REF=<dir>`.
- **Decisions** are recorded in `docs/decisions.md`; a change of behaviour adds to it.

By contributing you agree that your contribution is licensed under the GNU General Public
License, version 3 or later, as the rest of the CA-72 is.
