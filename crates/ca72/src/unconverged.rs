//! The model's iterative solves that stopped at their cap without converging, counted
//! on each thread by solver; tests require none. Such a solve leaves its last iterate, which
//! can be off by more than its tolerance: No Compromises' contours held their transistors'
//! steps to 0.2 V and stopped short 447 times in the worst case's 3 s (up to 5e-6 V at the
//! contours) before this counted them. The nodal solver's failures are counted by the
//! circuits that own one (`failed`); Potato's filter stops at two iterations by design and
//! is not counted.

use std::cell::Cell;

/// The solvers counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Solver {
    /// The contours' scalar root finders (`contour::root_slope`, `root_dir`).
    ContourRoot,
    /// The contours' grounded transistors, Q20 and Q12 (`contour::grounded_npn`).
    ContourTransistor,
    /// The contours' decay step (the capacitor and the sustain node).
    ContourDecay,
    /// A junction behind a resistance (`devices::series_junction_from`).
    SeriesJunction,
    /// A differential pair behind its drop (`devices::degenerated_from`, `_warm_to`).
    DegeneratedPair,
    /// A transistor's or a tail pair's junctions (`devices::solve_junctions_limited`,
    /// `solve_tail_pair`: the VCA's bias).
    Junctions,
    /// The VCA's bias passes, its tails not settled to 1e-12 of their currents
    /// (`vca::Vca::solve_bias`).
    VcaBias,
    /// An oscillator's converter (`expo::ExpoCircuit::solve_current`).
    Converter,
    /// The filter's control node and ladder current (`vcf::root`, `FilterExpo::q28`).
    FilterNode,
    /// The filter's loop (Newton's method each step), outside Potato.
    FilterLoop,
}

const N: usize = 10;

/// The solvers' names, in [`Solver`]'s order.
pub const NAMES: [&str; N] = [
    "contour root",
    "contour transistor",
    "contour decay",
    "series junction",
    "degenerated pair",
    "junctions",
    "VCA bias",
    "converter",
    "filter node",
    "filter loop",
];

thread_local! {
    static COUNTS: [Cell<u64>; N] = const { [const { Cell::new(0) }; N] };
}

/// Notes a solve of `s` that stopped at its cap without converging.
#[inline]
pub fn note(s: Solver) {
    COUNTS.with(|c| {
        let k = &c[s as usize];
        k.set(k.get() + 1);
    });
}

/// This thread's counts since the last take, by solver (only those not zero), and zeroes
/// them.
pub fn take() -> Vec<(&'static str, u64)> {
    COUNTS.with(|c| {
        NAMES
            .iter()
            .zip(c.iter())
            .filter_map(|(name, k)| {
                let n = k.replace(0);
                (n > 0).then_some((*name, n))
            })
            .collect()
    })
}
