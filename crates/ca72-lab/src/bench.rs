//! Assembling netlists from the repository's circuit files.

use crate::circuits_dir;

/// Supplies for a bench. The first benches use ideal rails (docs/circuit/assumptions.md
/// A1): the regulators of board 3 are not in circuit yet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supplies {
    pub vp: f64,
    pub vn: f64,
}

impl Default for Supplies {
    fn default() -> Self {
        Supplies {
            vp: 10.0,
            vn: -10.0,
        }
    }
}

/// Simulator settings shared by the benches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Solver {
    /// Circuit temperature, C.
    pub temp: f64,
    pub reltol: f64,
    pub abstol: f64,
    pub vntol: f64,
    /// Integration method: "gear" (order 2) or "trap".
    pub method: &'static str,
    /// Upper bound on the time step, as a fraction of the expected period.
    pub steps_per_period: f64,
}

impl Default for Solver {
    fn default() -> Self {
        Solver {
            temp: 25.0,
            reltol: 1e-4,
            abstol: 1e-12,
            vntol: 1e-6,
            method: "gear",
            steps_per_period: 400.0,
        }
    }
}

/// The start of every bench: title, model and board libraries, options and supplies.
pub fn header(title: &str, supplies: Supplies, solver: Solver) -> String {
    let dir = circuits_dir();
    let lib = |p: &str| format!(".include {}\n", dir.join(p).display());
    let mut s = format!("{title}\n");
    s.push_str(&lib("models/mm-devices.lib"));
    s.push_str(&lib("models/ua741.lib"));
    s.push_str(&lib("boards/board1-vco.lib"));
    s.push_str(&lib("boards/board1-refs.lib"));
    s.push_str(&lib("boards/board1-osc23.lib"));
    s.push_str(&format!(
        ".options temp={} tnom=25 reltol={} abstol={} vntol={} method={} maxord=2\n",
        solver.temp, solver.reltol, solver.abstol, solver.vntol, solver.method
    ));
    s.push_str(&format!(
        "vp vp 0 {}\nvn vn 0 {}\n",
        supplies.vp, supplies.vn
    ));
    s
}
