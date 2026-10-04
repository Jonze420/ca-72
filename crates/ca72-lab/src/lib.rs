//! The Minimoog circuit lab (`docs/circuit/`): the offline reference.
//!
//! The instrument's boards are transcribed as SPICE netlists in `circuits/boards/`, with
//! device models in `circuits/models/`. This crate assembles test benches from them, runs
//! them in ngspice ([`ca72_spice`]) and measures what the real-time models
//! ([`ca72`]) are checked against: operating points, frequencies, the factory
//! calibration, waveforms.
//!
//! Agreement with ngspice shows the real-time models agree with the transcribed circuit and
//! its device models; it says nothing by itself about agreement with a physical instrument
//! (`docs/circuit/assumptions.md`, "Limits of validation").

pub mod bench;
pub mod board3;
pub mod board4ext;
pub mod contour;
pub mod keyboard;
pub mod quality;
pub mod vca;
pub mod vcf;
pub mod vco;
pub mod worst;

use std::path::PathBuf;

/// The repository's `circuits/` directory.
pub fn circuits_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../circuits")
}

/// A work directory for a named run, under the workspace's `target/ca72-lab/`: runs
/// keep their netlists and logs there so a failure can be inspected.
pub fn work_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    base.join("ca72-lab").join(name)
}
