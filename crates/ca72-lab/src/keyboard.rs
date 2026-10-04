//! Board 2's keyboard circuit on a bench (circuit No. 1, `board2-keyboard.lib`): the key
//! string fed by its current source, keys joining the pitch bus to the string and the
//! trigger bus to +10 V, the GLIDE control, and the output's load, for the real-time
//! keyboard circuit to be checked against.

use crate::bench::Solver;
use crate::circuits_dir;
use ca72_spice::{Error, Ngspice, Plot};
use std::fmt::Write as _;
use std::path::Path;

/// The key string's resistors (43 of 10 ohm, 1 %: dwg 1436) and the keys (44, F to C).
pub const STRING_R: f64 = 10.0;
pub const KEYS: usize = 44;

/// A key contact's resistance, ohm: unmeasured (a gold spring on a gold bar), the real-time
/// model's default (docs/circuit/assumptions.md A18). With two keys held the string's
/// current flows through their contacts.
pub const CONTACT_R: f64 = 0.1;

/// The output's load at the instrument's usual settings, as a resistance to a voltage: the
/// three oscillators' keyboard inputs (51.1K each, to summing junctions at -5 V) and the
/// filter's KEYBOARD CONTROL 1 (R53 300K, to its control node near 0 V): the real-time
/// model's `keyboard::LOAD_DEFAULT`.
pub const LOAD_DEFAULT: (f64, f64) = (
    1.0 / (3.0 / 51.1e3 + 1.0 / 300e3),
    -5.0 * (3.0 / 51.1e3) / (3.0 / 51.1e3 + 1.0 / 300e3),
);
/// The bench.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardBench {
    /// Keys played: (key from the lowest F, 0..44; down at s; up at s).
    pub presses: Vec<(usize, f64, f64)>,
    /// GLIDE: the 5M pot's resistance in circuit, ohm; `None` with the GLIDE switch off
    /// (its contacts short the pot).
    pub glide: Option<f64>,
    /// The output's load: resistance, ohm, to a voltage, V.
    pub load: (f64, f64),
    /// How much later than the pitch contact the trigger contact closes, and how much
    /// earlier it opens, s (the keyboard is set up so the pitch comes first: service
    /// manual 2.2.6).
    pub contact_lead: f64,
}

impl Default for KeyboardBench {
    fn default() -> Self {
        KeyboardBench {
            presses: Vec::new(),
            glide: None,
            load: LOAD_DEFAULT,
            contact_lead: 0.0,
        }
    }
}

/// The netlist's common part: models, board, options, supplies, the string and the glide.
fn common(title: &str, glide: Option<f64>, load: (f64, f64), solver: Solver) -> String {
    let dir = circuits_dir();
    let mut s = format!(
        "{title}\n.include {m}\n.include {k}\n\
         .options temp={t} tnom=25 reltol={rt} abstol=1e-12 vntol=1e-7 method={me} maxord=2\n\
         vp10 p10 0 10\nvn10 n10 0 -10\n\
         * the key string: its low end grounded, fed at the top by the current source\n\
         rtop kcur s{top} 1m\nvs0 s0 0 0\n",
        m = dir.join("models/mm-devices.lib").display(),
        k = dir.join("boards/board2-keyboard.lib").display(),
        t = solver.temp,
        rt = solver.reltol,
        me = solver.method,
        top = KEYS - 1,
    );
    for k in 0..KEYS - 1 {
        let _ = writeln!(s, "rs{k} s{} s{k} {STRING_R}", k + 1);
    }
    let _ = writeln!(
        s,
        "* GLIDE: the pot between pins 8 and 7 (the switch off shorts it)\n\
         rglide gout gin {}\n\
         * the output's load: the oscillators' and the filter's keyboard inputs\n\
         rload out lv {}\nvload lv 0 {}\n\
         x1 kcur bus trig gout gin out p10 n10 mm_keyboard",
        glide.unwrap_or(1e-3).max(1e-3),
        load.0,
        load.1
    );
    s
}

/// The static transfer: for each key held alone (the trigger bus at +10 V, settled), the
/// pitch bus and the output, V; and the string's current, A.
pub fn static_keys(
    spice: &Ngspice,
    work: &Path,
    load: (f64, f64),
    solver: Solver,
) -> Result<(Vec<(f64, f64)>, f64), Error> {
    let mut net = common("minimoog keyboard static", None, load, solver);
    // Every key's contact, open; the trigger bus held at +10 V.
    for k in 0..KEYS {
        let _ = writeln!(net, "rk{k} bus s{k} 1e12");
    }
    net.push_str("vtrig trig 0 10\n");
    let mut analyses = Vec::with_capacity(KEYS);
    for k in 0..KEYS {
        let mut a = String::new();
        if k > 0 {
            let _ = writeln!(a, "alter rk{} = 1e12", k - 1);
        }
        let _ = write!(a, "alter rk{k} = {CONTACT_R}\nop");
        analyses.push(a);
    }
    let refs: Vec<&str> = analyses.iter().map(String::as_str).collect();
    let plots = spice.run(&net, &refs, work)?;
    let keys = plots
        .iter()
        .map(|p| (p.scalar("v(bus)"), p.scalar("v(out)")))
        .collect();
    // The string's current from its top with key 0 held (the bus's current then goes to
    // ground at the bottom, not through the string).
    let current = plots[0].scalar("v(kcur)") / (STRING_R * (KEYS - 1) as f64);
    Ok((keys, current))
}

/// The netlist of the transient bench: each press closes its key's pitch contact, and the
/// trigger contact while any key is held.
pub fn netlist(b: &KeyboardBench, solver: Solver) -> String {
    let mut net = common("minimoog keyboard", b.glide, b.load, solver);
    let _ = writeln!(
        net,
        ".model swkey sw(vt=0.5 vh=0.1 ron={CONTACT_R} roff=1e12)"
    );
    // A contact's control: closed from `d` to `u` (closed from the start, operating point
    // included, when `d` is 0).
    let edge = 1e-6;
    let pwl = |d: f64, u: f64| {
        if d <= 0.0 {
            format!("pwl(0 1 {u} 1 {} 0)", u + edge)
        } else {
            format!("pwl(0 0 {d} 0 {} 1 {u} 1 {} 0)", d + edge, u + edge)
        }
    };
    // Pitch contacts, one per press (a key pressed twice gets two).
    for (i, &(k, d, u)) in b.presses.iter().enumerate() {
        let _ = writeln!(
            net,
            "vpk{i} pk{i} 0 {}\nspk{i} bus s{k} pk{i} 0 swkey",
            pwl(d, u)
        );
    }
    // The trigger contact: closed while any key is held (each press's trigger contact in
    // parallel, closing `contact_lead` after its pitch contact and opening that much before).
    for (i, &(_, d, u)) in b.presses.iter().enumerate() {
        let d = if d <= 0.0 { 0.0 } else { d + b.contact_lead };
        let u = (u - b.contact_lead).max(d + edge);
        let _ = writeln!(
            net,
            "vtk{i} tk{i} 0 {}\nstk{i} p10 trig tk{i} 0 swkey",
            pwl(d, u)
        );
    }
    // With no press the trigger bus floats on the board's R65/R34 to -10 V.
    net
}

/// A transient run of `tstop` s with steps no longer than `tmax`.
pub fn transient(
    spice: &Ngspice,
    work: &Path,
    b: &KeyboardBench,
    tstop: f64,
    tmax: f64,
    solver: Solver,
) -> Result<Plot, Error> {
    let net = netlist(b, solver);
    let plots = spice.run(
        &net,
        &[&format!("tran {tmax:e} {tstop:e} 0 {tmax:e}")],
        work,
    )?;
    plots
        .into_iter()
        .next()
        .ok_or_else(|| Error::Raw("no plot".into()))
}
