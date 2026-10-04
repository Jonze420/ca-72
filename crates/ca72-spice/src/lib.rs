//! ngspice as the offline circuit reference (`docs/circuit/`).
//!
//! The model is derived from transcribed netlists of the instrument's boards
//! (`circuits/boards/`). ngspice solves those netlists at the circuit level; the
//! real-time models are checked against its answers. ngspice is a tool here, never a
//! dependency of the plug-in: nothing in it links or starts ngspice.
//!
//! - [`Ngspice::find`] looks for the binary: `CA72_NGSPICE`, then the newest
//!   `~/.local/opt/ngspice-*/bin/ngspice`, then Homebrew's, then `PATH`.
//! - [`Ngspice::run`] writes a netlist, appends a control block that runs the given
//!   analyses and writes each one's vectors to a binary rawfile, runs ngspice in batch
//!   mode and reads the rawfiles back ([`Plot`]).
//! - Anything ngspice reports as an error (a singular matrix, a timestep too small, an
//!   unknown model, a failed analysis) is an [`Error`], never a partial result.
//!
//! Tests that need ngspice call [`for_test`]: without ngspice 47 ([`REFERENCE`]) it says
//! so and the test returns early, unless `CA72_REQUIRE_NGSPICE` is set (the Linux
//! reference machine), where a missing or other ngspice fails the test.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A located ngspice binary and its version line.
#[derive(Debug, Clone)]
pub struct Ngspice {
    pub path: PathBuf,
    /// For example `ngspice-47`.
    pub version: String,
}

/// What went wrong running a netlist.
#[derive(Debug)]
pub enum Error {
    NotFound(String),
    Io(String),
    /// ngspice reported an error; the text is the offending lines of its output.
    Simulator(String),
    /// A rawfile could not be read.
    Raw(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(s) => write!(f, "ngspice not found: {s}"),
            Error::Io(s) => write!(f, "ngspice run failed: {s}"),
            Error::Simulator(s) => write!(f, "ngspice reported errors:\n{s}"),
            Error::Raw(s) => write!(f, "bad rawfile: {s}"),
        }
    }
}

impl std::error::Error for Error {}

/// One analysis's vectors, as ngspice wrote them.
#[derive(Debug, Clone)]
pub struct Plot {
    /// `Transient Analysis`, `Operating Point`, `DC transfer characteristic`, ...
    pub name: String,
    /// Vector names as ngspice gives them (`time`, `v(out)`, `i(v1)`), lower case.
    pub names: Vec<String>,
    /// For real plots, one vector per name.
    pub real: Vec<Vec<f64>>,
    /// For complex plots (AC), one (re, im) vector per name; empty for real plots.
    pub complex: Vec<Vec<(f64, f64)>>,
}

impl Plot {
    /// The real vector named `name` (case-insensitive; `out` finds `v(out)` and `@q1[ic]`
    /// finds `i(@q1[ic])` too).
    pub fn get(&self, name: &str) -> Option<&[f64]> {
        let i = self.index(name)?;
        self.real.get(i).map(Vec::as_slice)
    }

    /// Like [`Plot::get`], panicking with the available names when it is missing.
    pub fn vec(&self, name: &str) -> &[f64] {
        match self.get(name) {
            Some(v) => v,
            None => panic!("no vector {name} in {}: {:?}", self.name, self.names),
        }
    }

    pub fn complex_vec(&self, name: &str) -> Option<&[(f64, f64)]> {
        let i = self.index(name)?;
        self.complex.get(i).map(Vec::as_slice)
    }

    fn index(&self, name: &str) -> Option<usize> {
        let want = name.to_ascii_lowercase();
        let voltage = format!("v({want})");
        // Saved device quantities (`@q1[ic]`) come back as `i(@q1[ic])`.
        let device = format!("i({want})");
        self.names
            .iter()
            .position(|n| *n == want)
            .or_else(|| self.names.iter().position(|n| *n == voltage))
            .or_else(|| self.names.iter().position(|n| *n == device))
    }

    /// The first scalar of a vector: an operating point's value.
    pub fn scalar(&self, name: &str) -> f64 {
        self.vec(name)[0]
    }
}

impl Ngspice {
    /// Looks for ngspice (see the crate documentation).
    pub fn find() -> Result<Ngspice, Error> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = std::env::var_os("CA72_NGSPICE") {
            candidates.push(PathBuf::from(p));
        }
        if let Some(home) = std::env::var_os("HOME") {
            let opt = Path::new(&home).join(".local/opt");
            if let Ok(dir) = std::fs::read_dir(&opt) {
                let mut found: Vec<(u32, PathBuf)> = dir
                    .filter_map(Result::ok)
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        let n = name.strip_prefix("ngspice-")?.parse::<u32>().ok()?;
                        Some((n, e.path().join("bin/ngspice")))
                    })
                    .collect();
                found.sort();
                candidates.extend(found.into_iter().rev().map(|(_, p)| p));
            }
        }
        candidates.push(PathBuf::from("/opt/homebrew/bin/ngspice"));
        candidates.push(PathBuf::from("/usr/local/bin/ngspice"));
        candidates.push(PathBuf::from("ngspice"));
        let mut tried = Vec::new();
        for c in candidates {
            match version_of(&c) {
                Some(version) => return Ok(Ngspice { path: c, version }),
                None => tried.push(c.display().to_string()),
            }
        }
        Err(Error::NotFound(format!("tried {}", tried.join(", "))))
    }

    /// Runs `netlist` (a complete circuit without `.control` or `.end`) in `work`, running
    /// each of `analyses` (`op`, `tran 1u 10m`, `dc v1 0 5 0.01`, ...) in turn, and
    /// returns their plots in the same order.
    pub fn run(&self, netlist: &str, analyses: &[&str], work: &Path) -> Result<Vec<Plot>, Error> {
        std::fs::create_dir_all(work).map_err(|e| Error::Io(e.to_string()))?;
        let cir = work.join("circuit.cir");
        let log = work.join("ngspice.log");
        let mut text = String::with_capacity(netlist.len() + 512);
        text.push_str(netlist.trim_end());
        text.push_str("\n.control\nset filetype=binary\nset noaskquit\n");
        let mut raws = Vec::new();
        for (i, a) in analyses.iter().enumerate() {
            let raw = work.join(format!("plot{i}.raw"));
            let _ = std::fs::remove_file(&raw);
            text.push_str(a);
            text.push('\n');
            // ngspice runs in `work`, so the rawfile is named relatively: a path with a
            // space would be split into two arguments by `write`.
            text.push_str(&format!("write plot{i}.raw all\n"));
            raws.push(raw);
        }
        text.push_str("quit\n.endc\n.end\n");
        std::fs::write(&cir, &text).map_err(|e| Error::Io(e.to_string()))?;
        let out = Command::new(&self.path)
            .arg("-b")
            .arg("-o")
            .arg(&log)
            .arg(&cir)
            .current_dir(work)
            .output()
            .map_err(|e| Error::Io(format!("{}: {e}", self.path.display())))?;
        let mut messages = std::fs::read_to_string(&log).unwrap_or_default();
        messages.push_str(&String::from_utf8_lossy(&out.stdout));
        messages.push_str(&String::from_utf8_lossy(&out.stderr));
        let errors = error_lines(&messages);
        if !errors.is_empty() {
            return Err(Error::Simulator(errors.join("\n")));
        }
        if !out.status.success() {
            return Err(Error::Simulator(format!(
                "exit status {}\n{}",
                out.status,
                tail(&messages, 30)
            )));
        }
        let mut plots = Vec::new();
        for raw in &raws {
            let bytes = std::fs::read(raw).map_err(|e| {
                Error::Raw(format!("{}: {e}\n{}", raw.display(), tail(&messages, 30)))
            })?;
            let mut p = parse_raw(&bytes)?;
            if p.len() != 1 {
                return Err(Error::Raw(format!(
                    "{} holds {} plots",
                    raw.display(),
                    p.len()
                )));
            }
            plots.push(p.remove(0));
        }
        Ok(plots)
    }
}

/// The ngspice the reference measurements were made with: another version's answers may
/// differ in the last digits the tests compare.
pub const REFERENCE: &str = "ngspice-47";

/// ngspice for a test: `None` (after saying why) when it is not installed or is not
/// [`REFERENCE`], unless `CA72_REQUIRE_NGSPICE` is set, when either panics.
pub fn for_test(test: &str) -> Option<Ngspice> {
    let why = match Ngspice::find() {
        Ok(n) if n.version == REFERENCE => return Some(n),
        Ok(n) => format!("{} is {}, not {REFERENCE}", n.path.display(), n.version),
        Err(e) => e.to_string(),
    };
    if std::env::var_os("CA72_REQUIRE_NGSPICE").is_some() {
        panic!("{test}: CA72_REQUIRE_NGSPICE is set and {why}");
    }
    eprintln!("SKIPPED {test}: {why}");
    None
}

fn version_of(path: &Path) -> Option<String> {
    let out = Command::new(path).arg("-v").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.lines()
        .find_map(|l| {
            let l = l.trim_start_matches(['*', ' ']);
            l.strip_prefix("ngspice-")
                .map(|rest| format!("ngspice-{}", rest.split_whitespace().next().unwrap_or("")))
        })
        .or_else(|| text.contains("ngspice").then(|| "ngspice".to_string()))
}

/// Lines of ngspice's output that mean the result cannot be trusted.
fn error_lines(messages: &str) -> Vec<String> {
    const BAD: &[&str] = &[
        "error",
        "singular matrix",
        "timestep too small",
        "simulation(s) aborted",
        "unknown model",
        "unknown subckt",
        "could not find",
        "no such vector",
        "iteration limit reached",
        "doanalyses:",
    ];
    // "Dynamic gmin stepping failed" and the like are not errors by themselves: ngspice
    // goes on to the next method. They count only when no later method completed.
    const STEPPING: &[&str] = &["gmin stepping failed", "source stepping failed"];
    // Lines ngspice prints that contain those words but are not errors.
    const BENIGN: &[&str] = &[
        "note: can't find the initialization file",
        "reltol",
        "errors: 0",
    ];
    let lower_all = messages.to_ascii_lowercase();
    let recovered = lower_all.contains("stepping completed");
    messages
        .lines()
        .filter(|l| {
            let lower = l.to_ascii_lowercase();
            let bad = BAD.iter().any(|b| lower.contains(b))
                || (!recovered && STEPPING.iter().any(|b| lower.contains(b)));
            bad && !BENIGN.iter().any(|b| lower.contains(b))
        })
        .map(str::to_string)
        .collect()
}

fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Reads an ngspice binary rawfile (one or more plots).
pub fn parse_raw(bytes: &[u8]) -> Result<Vec<Plot>, Error> {
    let mut plots = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        let (plot, next) = parse_one(bytes, at)?;
        plots.push(plot);
        at = next;
        // Trailing newlines between plots.
        while at < bytes.len() && (bytes[at] == b'\n' || bytes[at] == b'\r') {
            at += 1;
        }
    }
    Ok(plots)
}

fn parse_one(bytes: &[u8], start: usize) -> Result<(Plot, usize), Error> {
    let mut name = String::new();
    let mut complex = false;
    let mut nvars = 0usize;
    let mut npoints = 0usize;
    let mut names = Vec::new();
    let mut at = start;
    let mut in_vars = false;
    loop {
        let end = bytes[at..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| at + p)
            .ok_or_else(|| Error::Raw("header ends early".into()))?;
        let line = String::from_utf8_lossy(&bytes[at..end]).into_owned();
        at = end + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("Binary:") {
            break;
        }
        if trimmed.starts_with("Values:") {
            return Err(Error::Raw(
                "ASCII rawfiles are not read; set filetype=binary".into(),
            ));
        }
        if in_vars {
            let fields: Vec<&str> = trimmed.split_whitespace().collect();
            if fields.len() >= 2 && fields[0].parse::<usize>().is_ok() {
                names.push(fields[1].to_ascii_lowercase());
                continue;
            }
            in_vars = false;
        }
        if let Some(v) = trimmed.strip_prefix("Plotname:") {
            name = v.trim().to_string();
        } else if let Some(v) = trimmed.strip_prefix("Flags:") {
            complex = v.contains("complex");
        } else if let Some(v) = trimmed.strip_prefix("No. Variables:") {
            nvars = v
                .trim()
                .parse()
                .map_err(|_| Error::Raw(format!("bad count {v}")))?;
        } else if let Some(v) = trimmed.strip_prefix("No. Points:") {
            npoints = v
                .trim()
                .parse()
                .map_err(|_| Error::Raw(format!("bad count {v}")))?;
        } else if trimmed.starts_with("Variables:") {
            in_vars = true;
        }
    }
    if names.len() != nvars {
        return Err(Error::Raw(format!(
            "{} names for {nvars} variables",
            names.len()
        )));
    }
    let width = if complex { 16 } else { 8 };
    let need = nvars * npoints * width;
    if bytes.len() < at + need {
        return Err(Error::Raw(format!(
            "{} data bytes, {need} expected",
            bytes.len().saturating_sub(at)
        )));
    }
    let f = |i: usize| {
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[i..i + 8]);
        f64::from_le_bytes(b)
    };
    let mut plot = Plot {
        name,
        names,
        real: Vec::new(),
        complex: Vec::new(),
    };
    if complex {
        plot.complex = vec![Vec::with_capacity(npoints); nvars];
        for p in 0..npoints {
            for v in 0..nvars {
                let i = at + (p * nvars + v) * 16;
                plot.complex[v].push((f(i), f(i + 8)));
            }
        }
        // The scale (frequency) as a real vector too.
        plot.real = plot
            .complex
            .iter()
            .map(|c| c.iter().map(|x| x.0).collect())
            .collect();
    } else {
        plot.real = vec![Vec::with_capacity(npoints); nvars];
        for p in 0..npoints {
            for v in 0..nvars {
                plot.real[v].push(f(at + (p * nvars + v) * 8));
            }
        }
    }
    Ok((plot, at + need))
}

/// Samples `y(t)` (given at the increasing times `t`) at a uniform rate by linear
/// interpolation, from `t0` for `n` samples.
pub fn resample(t: &[f64], y: &[f64], t0: f64, rate: f64, n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    let mut j = 0usize;
    for k in 0..n {
        let tk = t0 + k as f64 / rate;
        while j + 2 < t.len() && t[j + 1] < tk {
            j += 1;
        }
        let (ta, tb) = (t[j], t[j + 1]);
        let a = if tb > ta {
            ((tk - ta) / (tb - ta)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(y[j] + (y[j + 1] - y[j]) * a);
    }
    out
}

/// Times at which `y` crosses `level` going downward (`rising = false`) or upward,
/// interpolated linearly between the solver's points.
pub fn crossings(t: &[f64], y: &[f64], level: f64, rising: bool) -> Vec<f64> {
    let mut out = Vec::new();
    for i in 1..y.len().min(t.len()) {
        let (a, b) = (y[i - 1] - level, y[i] - level);
        let hit = if rising {
            a < 0.0 && b >= 0.0
        } else {
            a > 0.0 && b <= 0.0
        };
        if hit {
            let f = a / (a - b);
            out.push(t[i - 1] + f * (t[i] - t[i - 1]));
        }
    }
    out
}
