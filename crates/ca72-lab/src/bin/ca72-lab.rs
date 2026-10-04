//! `ca72-lab`: the circuit lab's command line (docs/circuit/README.md).
//!
//! ```text
//! ca72-lab calibrate [folkman|norlin]      factory tuning of oscillator 1 in ngspice
//! ca72-lab track [out.json]                tracking over every range after calibration
//! ca72-lab core [out.json]                 the core's timing data (fit_core.py's input)
//! ca72-lab shapers [out.json]              DC sweeps of the wave shapers
//! ca72-lab tables                          regenerates ca72's tables.rs
//! ca72-lab waves <range> <key> <out.csv>   two periods of every node, ngspice
//! ca72-lab render <range> <key> <out.wav> [seconds] [oversample]
//!                                        the real-time oscillator, tuned the 1973 way
//! ```
//! Ranges: lo, 32, 16, 8, 4, 2. Keys count from the keyboard's lowest F (0..43).

use ca72::voice::Quality;
use ca72_lab::bench::Solver;
use ca72_lab::{circuits_dir, vco, work_dir};
use ca72_spice::Ngspice;
use std::process::ExitCode;

type Res = Result<(), String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = &args[args.len().min(1)..];
    let result = match args.first().map(String::as_str) {
        Some("calibrate") => spice().and_then(|s| calibrate(&s, rest)),
        Some("track") => spice().and_then(|s| track(&s, rest)),
        Some("core") => spice().and_then(|s| core(&s, rest)),
        Some("shapers") => spice().and_then(|s| shapers(&s, rest)),
        Some("tables") => spice().and_then(|s| tables(&s)),
        Some("waves") => spice().and_then(|s| waves(&s, rest)),
        Some("render") => render(rest),
        Some("play") => play(rest),
        Some("perf") => perf(rest),
        Some("hifi") => hifi(rest),
        Some("worst") => worst(rest),
        Some("crowd") => crowd(rest),
        Some("solvers") => solvers(),
        Some("probe") => probe(rest),
        _ => Err(
            "usage: ca72-lab calibrate|track|core|shapers|tables|waves|render|play|perf|hifi|worst|crowd|solvers (see the \
                  source's header)"
                .into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn spice() -> Result<Ngspice, String> {
    Ngspice::find().map_err(|e| e.to_string())
}

/// The trims Folkman's procedure gives in ngspice (`ca72-lab calibrate`, 2026-09-28).
fn calibrated() -> (vco::Trims, f64) {
    (
        vco::Trims {
            r11: 183.65,
            a8: 0.13518,
            octave_step: 0.29889,
            ..vco::Trims::default()
        },
        7.5315,
    )
}

fn lab_range(s: Option<&String>) -> vco::Range {
    match s.map(String::as_str) {
        Some("lo") => vco::Range::Lo,
        Some("32") => vco::Range::R32,
        Some("16") => vco::Range::R16,
        Some("8") => vco::Range::R8,
        Some("4") => vco::Range::R4,
        _ => vco::Range::R2,
    }
}

fn write_json(path: &str, json: &serde_json::Value) -> Res {
    let text = serde_json::to_string_pretty(json).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
}

fn calibrate(spice: &Ngspice, rest: &[String]) -> Res {
    let t0 = std::time::Instant::now();
    let procedure = match rest.first().map(String::as_str) {
        Some("norlin") => vco::Procedure::NorlinUpdated,
        _ => vco::Procedure::Folkman1973,
    };
    let c = vco::calibrate(
        spice,
        &work_dir("calibrate"),
        vco::Trims::default(),
        Solver::default(),
        procedure,
    )
    .map_err(|e| e.to_string())?;
    println!("{:?}\n{:#?}\ntune {:.4} V", c.procedure, c.trims, c.tune);
    for (r, k, target, hz) in &c.points {
        println!(
            "{r:?} key {k:.4} V: {hz:.3} Hz (target {target}), {:+.3} cents",
            1200.0 * (hz / target).log2()
        );
    }
    println!(
        "{} runs, {:.1} s, {}",
        c.runs,
        t0.elapsed().as_secs_f64(),
        spice.version
    );
    Ok(())
}

fn track(spice: &Ngspice, rest: &[String]) -> Res {
    let solver = Solver::default();
    let cal = vco::calibrate(
        spice,
        &work_dir("track-cal"),
        vco::Trims::default(),
        solver,
        vco::Procedure::Folkman1973,
    )
    .map_err(|e| e.to_string())?;
    use vco::Range::*;
    let keys: Vec<u32> = (0..=43).step_by(3).collect();
    let points = vco::track(
        spice,
        &work_dir("track"),
        &cal,
        &[Lo, R32, R16, R8, R4, R2],
        &keys,
        solver,
    )
    .map_err(|e| e.to_string())?;
    for p in &points {
        println!(
            "{:>4} key {:2} {:10.4} Hz  target {:10.4}  {:+7.2} cents",
            p.range, p.key, p.hz, p.target_hz, p.cents
        );
    }
    if let Some(path) = rest.first() {
        write_json(
            path,
            &serde_json::json!({
                "calibration": {"procedure": format!("{:?}", cal.procedure), "r11": cal.trims.r11,
                    "a8": cal.trims.a8, "tc20": cal.trims.tc20, "octave_step": cal.trims.octave_step,
                    "tune": cal.tune},
                "ngspice": spice.version, "points": points}),
        )?;
    }
    Ok(())
}

fn core(spice: &Ngspice, rest: &[String]) -> Res {
    let (trims, tune) = calibrated();
    use vco::Range::*;
    let mut notes = Vec::new();
    for r in [Lo, R32, R16, R8, R4, R2] {
        for k in (0..=43).step_by(7) {
            notes.push((r, k));
        }
        notes.push((r, 43));
    }
    let points = vco::core_points(
        spice,
        &work_dir("core"),
        &trims,
        tune,
        &notes,
        Solver::default(),
    )
    .map_err(|e| e.to_string())?;
    let path = rest.first().cloned().unwrap_or_else(|| "core.json".into());
    write_json(
        &path,
        &serde_json::json!({"trims": {"r11": trims.r11, "a8": trims.a8,
            "octave_step": trims.octave_step, "tune": tune},
            "ngspice": spice.version, "points": points}),
    )?;
    eprintln!("{} points -> {path}", points.len());
    Ok(())
}

fn shapers(spice: &Ngspice, rest: &[String]) -> Res {
    let mut sweeps = Vec::new();
    for width in [0.0, -1.5, -2.5] {
        for (from, to) in [(0.2, -4.2), (-4.2, 0.2)] {
            let tag = format!("shapers-{width}-{from}");
            sweeps.push(
                vco::shaper_sweep(
                    spice,
                    &work_dir(&tag),
                    width,
                    from,
                    to,
                    0.002,
                    Solver::default(),
                )
                .map_err(|e| e.to_string())?,
            );
        }
    }
    let path = rest
        .first()
        .cloned()
        .unwrap_or_else(|| "shapers.json".into());
    write_json(
        &path,
        &serde_json::json!({"ngspice": spice.version, "sweeps": sweeps}),
    )
}

fn tables(spice: &Ngspice) -> Res {
    let tri = vco::triangle_table(spice, &work_dir("tables"), Solver::default())
        .map_err(|e| e.to_string())?;
    let out = ca72_lab::vcf::output_stage_table(spice, &work_dir("tables-out"), Solver::default())
        .map_err(|e| e.to_string())?;
    let path = circuits_dir().join("../crates/ca72/src/tables.rs");
    std::fs::write(&path, vco::tables_source(&tri, &out, &spice.version))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("-> {}", path.display());
    use ca72_lab::board4ext::{A440Bench, a440_source, a440_table};
    let t = a440_table(
        spice,
        &work_dir("tables-a440"),
        &A440Bench::default(),
        Solver::default(),
    )
    .map_err(|e| e.to_string())?;
    let path = circuits_dir().join("../crates/ca72/src/a440_table.rs");
    std::fs::write(&path, a440_source(&t, &spice.version))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!(
        "-> {} (R68 at {:.4}, {:.4} Hz)",
        path.display(),
        t.a68,
        t.hz
    );
    Ok(())
}

fn waves(spice: &Ngspice, rest: &[String]) -> Res {
    let range = lab_range(rest.first());
    let key: f64 = rest.get(1).and_then(|k| k.parse().ok()).unwrap_or(4.0);
    let (trims, tune) = calibrated();
    let ctl = vco::Controls {
        kbd: vco::key_volts(key),
        range,
        tune,
        ..vco::Controls::default()
    };
    let solver = Solver {
        steps_per_period: 4000.0,
        ..Solver::default()
    };
    let m = vco::measure(spice, &work_dir("waves"), &trims, &ctl, solver, 4)
        .map_err(|e| e.to_string())?;
    let t = m.plot.vec("time");
    let t0 = t[t.len() - 1] - 2.0 / m.hz;
    let names = [
        "ramp", "buf", "saw", "x1.tric", "tri", "rect", "x1.rst", "x1.b5", "x1.nj",
    ];
    let cols: Vec<&[f64]> = names.iter().map(|n| m.plot.vec(n)).collect();
    let mut out = format!("t,{}\n", names.join(","));
    for (i, &ti) in t.iter().enumerate().filter(|(_, ti)| **ti >= t0) {
        out.push_str(&format!("{:.12e}", ti - t0));
        for c in &cols {
            out.push_str(&format!(",{:.6e}", c[i]));
        }
        out.push('\n');
    }
    let path = rest.get(2).cloned().unwrap_or_else(|| "waves.csv".into());
    std::fs::write(&path, out).map_err(|e| format!("{path}: {e}"))?;
    eprintln!("{:.4} Hz -> {path}", m.hz);
    Ok(())
}

/// The real-time oscillator, tuned by Folkman's procedure, to a 32-bit float WAV file
/// (three channels: saw, triangle, rectangle, in volts / 5).
fn render(rest: &[String]) -> Res {
    use ca72::tuning::{self, Range, Tuning, osc1_inputs};
    let range = match rest.first().map(String::as_str) {
        Some("lo") => Range::Lo,
        Some("32") => Range::R32,
        Some("16") => Range::R16,
        Some("8") => Range::R8,
        Some("4") => Range::R4,
        _ => Range::R2,
    };
    let key: f64 = rest.get(1).and_then(|k| k.parse().ok()).unwrap_or(4.0);
    let path = rest.get(2).cloned().unwrap_or_else(|| "vco.wav".into());
    let seconds: f64 = rest.get(3).and_then(|k| k.parse().ok()).unwrap_or(2.0);
    let os: usize = rest.get(4).and_then(|k| k.parse().ok()).unwrap_or(4);
    let sr = 48_000.0;
    let mut v = ca72::vco::Vco::new(sr, os);
    let (t, _) = tuning::folkman_1973(&mut v, sr, Tuning::default());
    let i_in = v.expo.input_current(&osc1_inputs(&t, key, range));
    v.reset();
    let n = (sr * seconds) as usize;
    let mut data = Vec::with_capacity(n * 3 * 4);
    for _ in 0..n {
        let o = v.tick(i_in, 0.0);
        for x in [o.saw, o.tri, o.rect] {
            data.extend_from_slice(&((x / 5.0) as f32).to_le_bytes());
        }
    }
    std::fs::write(&path, wav_f32(3, 48_000, &data)).map_err(|e| format!("{path}: {e}"))?;
    eprintln!("{path}: {seconds} s, tuning {t:?}");
    Ok(())
}

/// `ca72-lab play <patch.json> <out.wav> [--quality MODE]`: the voice (docs/circuit/voice.md)
/// playing a patch's notes, in a quality mode (No Compromises unless given). The patch:
/// {"panel": {...}, "notes": [[start s, length s, MIDI note], ...], "moves": [[time s, key,
/// value], ...], "seconds": total}. The panel's keys are [`set_control`]'s, any left out at
/// their defaults; a move sets one as the patch plays.
fn play(rest: &[String]) -> Res {
    let usage = "ca72-lab play <patch.json> <out.wav> [--quality MODE]";
    let mut q = Quality::NoCompromises;
    let mut positional = Vec::new();
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--quality" => q = quality(args.next().ok_or(usage)?)?,
            other if other.starts_with("--") => {
                return Err(format!("ca72-lab play: unknown option {other}"));
            }
            _ => positional.push(a.clone()),
        }
    }
    let patch_path = positional.first().ok_or(usage)?;
    let out = positional
        .get(1)
        .cloned()
        .unwrap_or_else(|| "voice.wav".into());
    let r = render_file_at(patch_path, q, None, false)?;
    let data: Vec<u8> = r
        .samples
        .iter()
        .flat_map(|&y| (y as f32).to_le_bytes())
        .collect();
    std::fs::write(&out, wav_f32(1, 48_000, &data)).map_err(|e| format!("{out}: {e}"))?;
    let peak = r.samples.iter().fold(0.0f64, |a, y| a.max(y.abs()));
    eprintln!(
        "{out}: {:.1} s in {:.2} s (the voice built in {:.1} s), peak {peak:.3} ({:.1} V)",
        r.samples.len() as f64 / 48_000.0,
        r.ran,
        r.built,
        peak * 5.0
    );
    Ok(())
}

/// A patch's render: the output at 48 kHz (the voice's scale), and the seconds the voice
/// took to build and to render.
struct Rendered {
    samples: Vec<f64>,
    built: f64,
    ran: f64,
    /// Each sample's probes, when asked for (`worst::probes`).
    probes: Vec<ca72_lab::worst::Probes>,
}

fn render_file(patch_path: &str) -> Result<Rendered, String> {
    render_file_at(patch_path, Quality::NoCompromises, None, false)
}

/// [`render_file`] at a quality mode, the pitch and contour paths held at `hold` if given,
/// with each sample's probes if asked.
fn render_file_at(
    patch_path: &str,
    quality: Quality,
    hold: Option<Quality>,
    with_probes: bool,
) -> Result<Rendered, String> {
    let text = std::fs::read_to_string(patch_path).map_err(|e| format!("{patch_path}: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{patch_path}: {e}"))?;
    render_patch(&json, quality, hold, with_probes).map_err(|e| format!("{patch_path}: {e}"))
}

/// Renders a patch: {"panel": {...}, "notes": [[start s, length s, MIDI note], ...],
/// "moves": [[time s, key, value], ...], "input": {"hz", "amp"} (a sine at the EXTERNAL
/// INPUT jack, as a fraction of its 5 V), "seconds": total}. A failed solve anywhere is an
/// error.
fn render_patch(
    json: &serde_json::Value,
    quality: Quality,
    hold: Option<Quality>,
    with_probes: bool,
) -> Result<Rendered, String> {
    use ca72::voice::{Panel, Voice};
    let mut panel = Panel::default();
    for (k, v) in json["panel"].as_object().into_iter().flatten() {
        set_control(&mut panel, k, v)?;
    }
    // Moves: [[time s, key, value], ...] set a control as the patch plays: the wheels, a
    // sweep, a switch.
    let mut moves: Vec<(f64, String, serde_json::Value)> = Vec::new();
    for m in json["moves"].as_array().into_iter().flatten() {
        let (Some(t), Some(k)) = (m[0].as_f64(), m[1].as_str()) else {
            return Err(format!("a move is [time s, key, value], not {m}"));
        };
        set_control(&mut panel.clone(), k, &m[2]).map_err(|e| format!("move {m}: {e}"))?;
        moves.push((t, k.to_string(), m[2].clone()));
    }
    moves.sort_by(|a, b| a.0.total_cmp(&b.0));
    let seconds = json["seconds"].as_f64().unwrap_or(4.0);
    let (in_hz, in_amp) = (
        json["input"]["hz"].as_f64().unwrap_or(0.0),
        json["input"]["amp"].as_f64().unwrap_or(0.0),
    );
    let mut events: Vec<(f64, i32, bool)> = Vec::new();
    for n in json["notes"].as_array().ok_or("no notes")? {
        let (Some(t), Some(len), Some(note)) = (n[0].as_f64(), n[1].as_f64(), n[2].as_i64()) else {
            return Err(format!("a note is [start s, length s, MIDI note], not {n}"));
        };
        events.push((t, note as i32, true));
        events.push((t + len, note as i32, false));
    }
    // At equal times a release comes before a press (a repeated note is played again).
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
    let sr = 48_000.0;
    let t0 = std::time::Instant::now();
    let mut v = Voice::new(sr, panel);
    v.panel.quality = quality;
    v.hold_control_quality(hold);
    let built = t0.elapsed().as_secs_f64();
    let mut probes = Vec::new();
    ca72::prof::take();
    let n = (seconds * sr) as usize;
    let mut samples = Vec::with_capacity(n);
    let (mut next, mut next_move) = (0, 0);
    let t1 = std::time::Instant::now();
    for i in 0..n {
        let t = i as f64 / sr;
        while next < events.len() && events[next].0 <= t {
            v.note(events[next].1, events[next].2);
            next += 1;
        }
        while next_move < moves.len() && moves[next_move].0 <= t {
            let (_, k, x) = &moves[next_move];
            set_control(&mut v.panel, k, x)?;
            next_move += 1;
        }
        let ext = if in_amp != 0.0 {
            in_amp * (2.0 * std::f64::consts::PI * in_hz * t).sin()
        } else {
            0.0
        };
        samples.push(v.tick_in(ext));
        if with_probes {
            probes.push(ca72_lab::worst::probes(&v));
        }
    }
    let ran = t1.elapsed().as_secs_f64();
    let failed = v.keyboard().failed + v.revsaw().failed + v.preamp_failed();
    if failed > 0 {
        return Err(format!(
            "{failed} solves failed (keyboard, reverse sawtooth, preamplifier)"
        ));
    }
    Ok(Rendered {
        samples,
        built,
        ran,
        probes,
    })
}

/// A quality mode's name on the command line.
fn quality(name: &str) -> Result<Quality, String> {
    match name {
        "no-compromises" | "nc" => Ok(Quality::NoCompromises),
        "high-fidelity" | "hifi" | "high" => Ok(Quality::HighFidelity),
        "potato" => Ok(Quality::Potato),
        other => Err(format!(
            "no quality mode {other} (no-compromises, high-fidelity, potato)"
        )),
    }
}

/// `ca72-lab hifi [--quality high-fidelity|potato] [--scenario NAME]`: a quality mode
/// against No Compromises on the performance scenarios (`ca72-lab perf`'s), each limit on its
/// own path (history.md, "High Fidelity: how it is measured"). Pitch: the keyboard's
/// voltage (1 V an octave) and each oscillator's timing current (its frequency follows it)
/// within 0.01 cent of No Compromises', allowing up to a sample of shift (each sample
/// within the range No Compromises' took from the sample before to the one after, widened
/// by the limit). Contours: within 1e-5 V (-120 dB of 10 V), with the same allowance of a
/// sample. Audio: the mode's render with the pitch and contour paths held at No
/// Compromises within 1e-6 of full scale (-120 dBFS) of No Compromises' in every sample.
/// Each render's speed in turn is shown. Any limit crossed fails the check; Potato, which
/// has no limits, is reported against them, not judged.
fn hifi(rest: &[String]) -> Res {
    let mut q = Quality::HighFidelity;
    let mut only: Option<String> = None;
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--quality" => q = quality(value()?)?,
            "--scenario" => only = Some(value()?.clone()),
            other => return Err(format!("ca72-lab hifi: unknown option {other}")),
        }
    }
    let patches = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("patches");
    let names = [
        "perf-idle",
        "v0-bass",
        "v0-three",
        "v0-board3",
        "perf-ext",
        "perf-a440",
        "perf-screech",
        "worst",
    ];
    let render = |name: &str, quality: Quality, hold: Option<Quality>, probes: bool| {
        if name == "worst" {
            let t0 = std::time::Instant::now();
            let (samples, probes) =
                ca72_lab::worst::render_at(48_000.0, 3.0, quality, hold, probes);
            Ok(Rendered {
                samples,
                built: 0.0,
                ran: t0.elapsed().as_secs_f64(),
                probes,
            })
        } else {
            render_file_at(
                &patches.join(format!("{name}.json")).to_string_lossy(),
                quality,
                hold,
                probes,
            )
        }
    };
    let mut failed: Vec<String> = Vec::new();
    for name in names {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        ca72::unconverged::take();
        let nc = render(name, Quality::NoCompromises, None, true)?;
        let nc_unconverged = ca72::unconverged::take();
        let hq = render(name, q, None, true)?;
        let q_unconverged = ca72::unconverged::take();
        let held = render(name, q, Some(Quality::NoCompromises), false)?;
        ca72::unconverged::take();
        // Solves stopped at their cap without converging (none expected).
        if !nc_unconverged.is_empty() || !q_unconverged.is_empty() {
            println!(
                "  {name}: solves stopped short: No Compromises {nc_unconverged:?}, {q:?} {q_unconverged:?}"
            );
            if q != Quality::Potato || !nc_unconverged.is_empty() {
                failed.push(format!("{name}: solves stopped short"));
            }
        }
        let m = ca72_lab::quality::measure(&nc.probes, &hq.probes, &nc.samples, &held.samples);
        let at = |i: usize| i as f64 / 48_000.0;
        let secs = nc.samples.len() as f64 / 48_000.0;
        println!(
            "{name}: pitch: keyboard {:.2e} cent at {:.4} s (beyond 0.01 cent {:.1} ms in all, {:.1} ms at most), oscillators {:.2e}, {:.2e}, {:.2e} cent; contours {:.2e}, {:.2e} V; audio {:.2e} ({:.1} dBFS) at {:.4} s; {:.3} of real time against {:.3}",
            m.keyboard.0,
            at(m.keyboard.1),
            m.keyboard_beyond.0 as f64 / 48.0,
            m.keyboard_beyond.1 as f64 / 48.0,
            m.oscillators[0].0,
            m.oscillators[1].0,
            m.oscillators[2].0,
            m.contours[0].0,
            m.contours[1].0,
            m.audio.0,
            20.0 * m.audio.0.max(1e-300).log10(),
            at(m.audio.1),
            hq.ran / secs,
            nc.ran / secs,
        );
        // (The audio allowing for a delay, and by its spectra: for modes whose resamplers
        // differ, where a plain subtraction measures the shift.)
        let (lag, dd) = ca72_lab::quality::delayed_difference(&held.samples, &nc.samples, 32);
        let spectra = ca72_lab::quality::spectral_difference(&held.samples, &nc.samples);
        println!(
            "  {name}: the audio {lag:+} samples late, then within {dd:.2e} ({:.1} dBFS); spectra {}",
            20.0 * dd.max(1e-300).log10(),
            match spectra {
                Some((median, (worst, w))) => format!(
                    "{median:.1} dB from No Compromises' in the median window, {worst:.1} dB in the worst (at {:.2} s)",
                    w as f64 / 48_000.0
                ),
                None => "silent".into(),
            }
        );
        failed.extend(m.crossed(name, 48_000.0));
    }
    if q == Quality::Potato {
        // Potato has no limits: its distances are its fidelity, as high as it can keep it.
        println!(
            "Potato: {} of the High Fidelity limits crossed (reported, not judged)",
            failed.len()
        );
        Ok(())
    } else if failed.is_empty() {
        println!("PASS: every scenario within {q:?}'s limits");
        Ok(())
    } else {
        Err(format!("FAIL:\n{}", failed.join("\n")))
    }
}

/// `ca72-lab perf record|check <dir>`: the performance scenarios (`patches/perf-*.json` and
/// the v0 patches) rendered, timed and compared sample by sample with a recording made
/// before an optimization: `record` writes each render (`<name>.f64`, little-endian) and
/// the times, and renders each again twice with every elementary function's result moved
/// one ulp up, then down (`ca72::ulp`; `record` needs the `twins` feature): the
/// twins' largest difference from it in each quarter second is the render's own spread
/// under rounding (`<name>.spread.json`). `check` renders again and
/// reports the largest difference and the speed-up, and judges it by the owner's rule
/// (history.md, "Three quality modes" and "No Compromises"): within 1e-7 of full scale
/// in every quarter second, or, in a chaotic render (one whose twins part by more), within
/// the twins' spread there; a render outside it fails the check (with `MM_PERF_WINDOWS`
/// set, each quarter second's difference and allowance are shown; built with the
/// `profile` feature, where the time went part by part).
fn perf(rest: &[String]) -> Res {
    let mode = rest
        .first()
        .map(String::as_str)
        .ok_or("ca72-lab perf record|check <dir>")?;
    let dir = std::path::PathBuf::from(rest.get(1).ok_or("ca72-lab perf record|check <dir>")?);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let patches = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("patches");
    let names = [
        "perf-idle",
        "v0-bass",
        "v0-three",
        "v0-board3",
        "perf-ext",
        "perf-a440",
        "perf-screech",
        // The worst case's load (`ca72-lab worst`) in turn, three seconds: every part moving,
        // every jack plugged.
        "worst",
    ];
    let times_path = dir.join("times.json");
    let old: serde_json::Value = std::fs::read_to_string(&times_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::json!({}));
    let mut times = serde_json::Map::new();
    let (mut total_old, mut total_new, mut worst_all) = (0.0, 0.0, 0.0f64);
    let mut outside: Vec<String> = Vec::new();
    // Each quarter second's largest difference between two renders.
    let windows = |a: &[f64], b: &[f64]| -> Vec<f64> {
        a.chunks(12_000)
            .zip(b.chunks(12_000))
            .map(|(x, y)| {
                x.iter()
                    .zip(y)
                    .fold(0.0f64, |m, (p, q)| m.max((p - q).abs()))
            })
            .collect()
    };
    for name in names {
        ca72::unconverged::take();
        let r = if name == "worst" {
            ca72::prof::take();
            let t0 = std::time::Instant::now();
            let samples = ca72_lab::worst::render(48_000.0, 3.0);
            Rendered {
                samples,
                built: 0.0,
                ran: t0.elapsed().as_secs_f64(),
                probes: Vec::new(),
            }
        } else {
            render_file(&patches.join(format!("{name}.json")).to_string_lossy())?
        };
        // A render whose solves stopped short of converging is neither a reference nor
        // within the rule (`ca72::unconverged`).
        let stopped = ca72::unconverged::take();
        if !stopped.is_empty() {
            outside.push(format!(
                "{name}: solves stopped short of converging {stopped:?}"
            ));
        }
        let secs = r.samples.len() as f64 / 48_000.0;
        if cfg!(feature = "profile") {
            // Where the time went (the build's laps; the contour's parts are within its
            // own), us a sample.
            let n = r.samples.len() as f64;
            let totals = ca72::prof::take();
            let parts: Vec<String> = ca72::prof::NAMES
                .iter()
                .zip(totals)
                .map(|(name, t)| format!("{name} {:.2}", t as f64 / n / 1000.0))
                .collect();
            println!("  {name}, us a sample: {}", parts.join(", "));
        }
        let file = dir.join(format!("{name}.f64"));
        times.insert(
            name.into(),
            serde_json::json!({"ran": r.ran, "built": r.built, "seconds": secs}),
        );
        match mode {
            "record" => {
                if !cfg!(feature = "twins") {
                    return Err(
                        "ca72-lab perf record: build with --features twins (a recording carries its twins' spread)"
                            .into(),
                    );
                }
                let bytes: Vec<u8> = r.samples.iter().flat_map(|y| y.to_le_bytes()).collect();
                std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
                // The twins: the same render with the elementary functions an ulp up, then
                // down.
                let mut spread = vec![0.0f64; r.samples.len().div_ceil(12_000)];
                for dir in [1, -1] {
                    ca72::ulp::set(dir);
                    let twin = if name == "worst" {
                        ca72_lab::worst::render(48_000.0, 3.0)
                    } else {
                        render_file(&patches.join(format!("{name}.json")).to_string_lossy())?
                            .samples
                    };
                    ca72::ulp::set(0);
                    for (s, d) in spread.iter_mut().zip(windows(&twin, &r.samples)) {
                        *s = s.max(d);
                    }
                }
                let chaotic = spread.iter().any(|&d| d > 1e-7);
                std::fs::write(
                    dir.join(format!("{name}.spread.json")),
                    serde_json::json!({"windows": spread, "chaotic": chaotic}).to_string(),
                )
                .map_err(|e| e.to_string())?;
                println!(
                    "{name}: its twins' largest spread {:.1e}{}",
                    spread.iter().fold(0.0f64, |a, &b| a.max(b)),
                    if chaotic { " (chaotic)" } else { "" }
                );
                println!(
                    "{name}: {secs:.1} s of audio in {:.3} s ({:.3} of real time); built in {:.2} s",
                    r.ran,
                    r.ran / secs,
                    r.built
                );
            }
            "check" => {
                let bytes = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                let reference: Vec<f64> = bytes
                    .chunks_exact(8)
                    .map(|c| f64::from_le_bytes(c.try_into().expect("8 bytes")))
                    .collect();
                if reference.len() != r.samples.len() {
                    return Err(format!(
                        "{name}: {} samples against the recording's {}",
                        r.samples.len(),
                        reference.len()
                    ));
                }
                let (mut worst, mut at, mut identical) = (0.0f64, 0usize, 0usize);
                for (i, (a, b)) in r.samples.iter().zip(&reference).enumerate() {
                    let d = (a - b).abs();
                    if a.to_bits() == b.to_bits() {
                        identical += 1;
                    }
                    if d > worst {
                        (worst, at) = (d, i);
                    }
                }
                // The rule: 1e-7 in every quarter second, or the twins' spread in a chaotic
                // render.
                // (A recording without its twins' spread is judged by 1e-7 throughout.)
                let spread: serde_json::Value =
                    std::fs::read_to_string(dir.join(format!("{name}.spread.json")))
                        .ok()
                        .and_then(|t| serde_json::from_str(&t).ok())
                        .unwrap_or_else(|| {
                            println!(
                                "  {name}: no twins' spread recorded: judged by 1e-7 throughout"
                            );
                            serde_json::json!({"windows": [], "chaotic": false})
                        });
                let chaotic = spread["chaotic"].as_bool().unwrap_or(false);
                let diffs = windows(&r.samples, &reference);
                let allowed: Vec<f64> = (0..diffs.len())
                    .map(|w| {
                        let s = spread["windows"][w].as_f64().unwrap_or(0.0);
                        if chaotic { s.max(1e-7) } else { 1e-7 }
                    })
                    .collect();
                if let Some(w) = (0..diffs.len()).find(|&w| diffs[w] > allowed[w]) {
                    outside.push(format!(
                        "{name}: {:.2e} in the quarter second from {:.2} s, allowed {:.2e}",
                        diffs[w],
                        w as f64 * 0.25,
                        allowed[w]
                    ));
                }
                if std::env::var_os("MM_PERF_WINDOWS").is_some() {
                    let line: Vec<String> = diffs
                        .iter()
                        .zip(&allowed)
                        .map(|(d, a)| format!("{d:.1e}/{a:.0e}"))
                        .collect();
                    println!(
                        "  {name} per 0.25 s (difference/allowed{}): {}",
                        if chaotic { ", chaotic" } else { "" },
                        line.join(" ")
                    );
                }
                let before = old[name]["ran"].as_f64().unwrap_or(f64::NAN);
                total_old += before;
                total_new += r.ran;
                worst_all = worst_all.max(worst);
                println!(
                    "{name}: {:.3} of real time (was {:.3}: {:.1}x); built in {:.2} s; largest difference {worst:.3e} ({:.1} dBFS) at {:.4} s; {:.4} % of samples bit-identical",
                    r.ran / secs,
                    before / secs,
                    before / r.ran,
                    r.built,
                    20.0 * worst.max(1e-300).log10(),
                    at as f64 / 48_000.0,
                    100.0 * identical as f64 / reference.len() as f64
                );
            }
            _ => return Err("ca72-lab perf record|check <dir>".into()),
        }
    }
    if mode == "record" {
        if !outside.is_empty() {
            return Err(format!("not recorded: {}", outside.join("; ")));
        }
        std::fs::write(
            &times_path,
            serde_json::to_string_pretty(&times).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    } else {
        println!(
            "all: {total_new:.2} s against {total_old:.2} s ({:.1}x); largest difference {worst_all:.3e}",
            total_old / total_new
        );
        if !outside.is_empty() {
            return Err(format!("FAIL: outside the rule:\n{}", outside.join("\n")));
        }
        println!("PASS: every render within the rule");
    }
    Ok(())
}

/// Sets a panel control from its patch key, refusing an unknown key or a value of the wrong
/// type or out of range (a misspelt key would otherwise leave the control at its default
/// without a word). Knobs run 0..1 for the panel's 0..10 (CUTOFF's -5..+5; the pitch wheel
/// -1..1, 0 its detent); switches are booleans; `noise` is "white" or "pink". The
/// oscillators' are `osc1_range` .. `osc3_freq`: ranges "lo", "32", "16", "8", "4", "2";
/// waveforms "triangle", "sharktooth" (oscillators 1 and 2), "reverse" (oscillator 3),
/// "sawtooth", "square", "wide", "narrow"; `on`, `volume`, and `freq` (oscillators 2, 3).
fn set_control(p: &mut ca72::voice::Panel, k: &str, v: &serde_json::Value) -> Res {
    use ca72::tuning::Range;
    use ca72::voice::Waveform;
    let bad = || format!("panel key {k:?} has the wrong type ({v})");
    let knob = |range: (f64, f64)| -> Result<f64, String> {
        let x = v.as_f64().ok_or_else(bad)?;
        if (range.0..=range.1).contains(&x) {
            Ok(x)
        } else {
            Err(format!(
                "panel key {k:?}: {x} is outside {}..{}",
                range.0, range.1
            ))
        }
    };
    let unit = (0.0, 1.0);
    let switch = || v.as_bool().ok_or_else(bad);
    if k == "osc3_control" {
        p.osc3_control = switch()?;
        return Ok(());
    }
    let osc = (|| {
        let rest = k.strip_prefix("osc")?;
        let n = rest.chars().next()?.to_digit(10)? as usize;
        let field = rest.get(1..)?.strip_prefix('_')?;
        (1..=3).contains(&n).then_some((n - 1, field))
    })();
    if let Some((n, field)) = osc {
        let o = &mut p.osc[n];
        match field {
            "range" => {
                o.range = match v.as_str().ok_or_else(bad)? {
                    "lo" => Range::Lo,
                    "32" => Range::R32,
                    "16" => Range::R16,
                    "8" => Range::R8,
                    "4" => Range::R4,
                    "2" => Range::R2,
                    x => return Err(format!("{k}: {x} (the ranges are lo, 32, 16, 8, 4, 2)")),
                }
            }
            "waveform" => {
                o.waveform = match (v.as_str().ok_or_else(bad)?, n) {
                    ("triangle", _) => Waveform::Triangle,
                    ("sharktooth", 0 | 1) => Waveform::SharkTooth,
                    ("reverse", 2) => Waveform::ReverseSawtooth,
                    ("sawtooth", _) => Waveform::Sawtooth,
                    ("square", _) => Waveform::Square,
                    ("wide", _) => Waveform::WideRectangle,
                    ("narrow", _) => Waveform::NarrowRectangle,
                    (x, _) => {
                        return Err(format!(
                            "{k}: {x} (oscillator {} has no such position)",
                            n + 1
                        ));
                    }
                }
            }
            "on" => o.on = switch()?,
            "volume" => o.volume = knob(unit)?,
            "freq" if n > 0 => o.freq = knob(unit)?,
            _ => return Err(format!("unknown panel key {k:?}")),
        }
        return Ok(());
    }
    match k {
        "keyboard_control_1" => p.keyboard_control_1 = switch()?,
        "keyboard_control_2" => p.keyboard_control_2 = switch()?,
        "glide_on" => p.glide_on = switch()?,
        "decay" => p.decay = switch()?,
        "noise_on" => p.noise_on = switch()?,
        "ext_on" => p.ext_on = switch()?,
        "a440" => p.a440 = switch()?,
        "ext_volume" => p.ext_volume = knob(unit)?,
        "tune" => p.tune = knob((-1.0, 1.0))?,
        "osc_mod" => p.osc_mod = switch()?,
        "filter_mod" => p.filter_mod = switch()?,
        "noise" => {
            p.noise_pink = match v.as_str() {
                Some("white") => false,
                Some("pink") => true,
                _ => {
                    return Err(format!(
                        "panel key \"noise\" is \"white\" or \"pink\", not {v}"
                    ));
                }
            }
        }
        "cutoff" => p.cutoff = knob(unit)?,
        "emphasis" => p.emphasis = knob(unit)?,
        "contour_amount" => p.contour_amount = knob(unit)?,
        "glide" => p.glide = knob(unit)?,
        "noise_volume" => p.noise_volume = knob(unit)?,
        "mod_mix" => p.mod_mix = knob(unit)?,
        "mod_wheel" => p.mod_wheel = knob(unit)?,
        "pitch_wheel" => p.pitch_wheel = knob((-1.0, 1.0))?,
        "filter_attack" => p.filter_contour.attack = knob(unit)?,
        "filter_decay" => p.filter_contour.decay = knob(unit)?,
        "filter_sustain" => p.filter_contour.sustain = knob(unit)?,
        "loudness_attack" => p.loudness_contour.attack = knob(unit)?,
        "loudness_decay" => p.loudness_contour.decay = knob(unit)?,
        "loudness_sustain" => p.loudness_contour.sustain = knob(unit)?,
        _ => return Err(format!("unknown panel key {k:?}")),
    }
    Ok(())
}

fn wav_f32(channels: u16, rate: u32, data: &[u8]) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + data.len());
    let block = channels * 4;
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&3u16.to_le_bytes());
    w.extend_from_slice(&channels.to_le_bytes());
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    w.extend_from_slice(&block.to_le_bytes());
    w.extend_from_slice(&32u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(data.len() as u32).to_le_bytes());
    w.extend_from_slice(data);
    w
}

/// Whether the benchmark's threads take the audio threads' priority: only when it is
/// paced (`--paced`), as a voice's workers are while an audio thread plays it. Unpaced it
/// runs as fast as it can, and at real-time priority it would hold its CPUs for the whole
/// run (the system's watchdog, `ca72_rt`, would then make it ordinary part way).
static REALTIME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Each thread of the worst-case benchmark first: the audio threads' priority where the
/// system allows it, as a host gives them, when paced ([`REALTIME`]); with
/// `MM_PIN=c0,c1,...` (Linux) the caller on logical CPU c0 and each worker on the next (for
/// measuring without the scheduler's placement, which can put two busy threads on one
/// core's two hardware threads).
fn audio_priority() {
    let r = if REALTIME.load(std::sync::atomic::Ordering::Relaxed) {
        ca72_rt::promote(
            std::time::Duration::from_micros(2667),
            std::time::Duration::from_micros(1333),
        )
    } else {
        Err("not paced: ordinary priority".into())
    };
    if let Ok(pin) = std::env::var("MM_PIN") {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let cpus: Vec<usize> = pin.split(',').filter_map(|x| x.parse().ok()).collect();
        let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // The caller (first) is pinned by the last worker: threads start with their
        // spawner's CPUs, where the caller would spin at real-time priority before they
        // had run.
        static CALLER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
        if k == 0 {
            CALLER.store(ca72_rt::tid(), std::sync::atomic::Ordering::Relaxed);
            eprintln!("caller: priority {r:?}");
        } else if let Some(&cpu) = cpus.get(k) {
            eprintln!(
                "thread {k} on cpu {cpu}: {:?}; priority {r:?}",
                ca72_rt::pin(0, cpu)
            );
            if k + 1 == cpus.len() {
                let caller = CALLER.load(std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "caller on cpu {}: {:?}",
                    cpus[0],
                    ca72_rt::pin(caller, cpus[0])
                );
            }
        }
    }
}

/// `ca72-lab crowd [--voices N] [--threads T] [--block N] [--seconds S] [--quality MODE]
/// [--paced]`: the crowd benchmark (`ca72_lab::worst::crowd`; Potato): N voices
/// (default 100) under the worst case's load, a fraction of a second apart in it, on T
/// threads (default the machine's less two), each thread running its voices in turn each
/// block;
/// fails when a block takes longer than it lasts.
fn crowd(rest: &[String]) -> Res {
    use ca72_lab::worst::{Crowd, crowd as run_crowd};
    let mut o = Crowd {
        voices: 100,
        // Two hardware threads left: one for the caller, one for the system.
        threads: std::thread::available_parallelism()
            .map_or(8, |n| n.get().saturating_sub(2).max(1)),
        rate: 48_000.0,
        block: 256,
        seconds: 20.0,
        quality: Quality::Potato,
        paced: false,
        pool: false,
    };
    let mut threads = None;
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--pool" => o.pool = true,
            "--voices" => o.voices = value()?.parse().map_err(|e| format!("--voices: {e}"))?,
            "--threads" => threads = Some(value()?.parse().map_err(|e| format!("--threads: {e}"))?),
            "--block" => o.block = value()?.parse().map_err(|e| format!("--block: {e}"))?,
            "--seconds" => o.seconds = value()?.parse().map_err(|e| format!("--seconds: {e}"))?,
            "--quality" => o.quality = quality(value()?)?,
            "--paced" => o.paced = true,
            other => return Err(format!("ca72-lab crowd: unknown option {other}")),
        }
    }
    // In the pool the calling thread works too: one hardware thread left for the system.
    if o.pool {
        o.threads += 1;
    }
    if let Some(t) = threads {
        o.threads = t;
    }
    REALTIME.store(o.paced, std::sync::atomic::Ordering::Relaxed);
    let r = run_crowd(o, audio_priority);
    println!(
        "{} voices on {} threads{} ({:?}), {} s: {}",
        o.voices,
        o.threads,
        if o.pool { " as the engine's pool" } else { "" },
        o.quality,
        o.seconds,
        r.summary()
    );
    if r.over() > 0 {
        return Err(format!(
            "FAIL: {} of {} blocks took longer than they last",
            r.over(),
            r.blocks.len()
        ));
    }
    println!("PASS");
    Ok(())
}

/// `ca72-lab worst [--block N] [--seconds S] [--serial] [--quality MODE] [--paced]`: the
/// worst-case benchmark (`ca72_lab::worst`): every control moving at once, the
/// external input on, fast notes with GLIDE, in blocks of N samples (default 256, the
/// quality modes' target), in a quality mode (default no-compromises); with `--paced` each
/// block starts at its time, as an audio callback does, not as soon as the last ends, and
/// the threads take the audio threads' priority (unpaced they stay ordinary); fails when a
/// block takes longer than it lasts.
/// `ca72-lab probe <file>`: the worst case's load with the voice's probes each sample (six
/// doubles a sample, little-endian: see `worst::render_probed`), for finding where two
/// builds' renders first part.
fn probe(rest: &[String]) -> Res {
    let file = rest.first().ok_or("ca72-lab probe <file>")?;
    // (With the `twins` feature, MM_ULP=1 or -1 renders a twin.)
    if let Some(d) = std::env::var("MM_ULP").ok().and_then(|x| x.parse().ok()) {
        ca72::ulp::set(d);
    }
    let bytes: Vec<u8> = ca72_lab::worst::render_probed(48_000.0, 3.0)
        .iter()
        .flatten()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    std::fs::write(file, bytes).map_err(|e| e.to_string())
}

/// `ca72-lab solvers`: the nodal circuits under the worst case's load (three seconds, in
/// turn): each one's size, steps and Newton iterations a sample, and the time it all took.
fn solvers() -> Res {
    use ca72::voice::Voice;
    let rate = 48_000.0;
    let seconds = 3.0;
    let mut v = Voice::prototype(rate);
    let t0 = std::time::Instant::now();
    ca72_lab::worst::render_voice(&mut v, rate, seconds);
    let took = t0.elapsed().as_secs_f64();
    let n = seconds * rate;
    let (amp, lamp) = v.preamp().circuits();
    for (name, c) in [
        ("keyboard", v.keyboard().circuit()),
        ("preamplifier", amp),
        ("lamp driver", lamp),
    ] {
        let (nodes, parts) = c.extent();
        println!(
            "  {name}'s parts: {:?}; {} solved nodes touched only by linear parts",
            c.census(),
            c.linear_nodes()
        );
        println!("  {name}'s connected components: {:?}", c.components());
        println!(
            "  {name}'s eliminations worked out again (a pivot moved): {:.2} % of its linear solves, from column {:.1} on average",
            100.0 * c.lu_recorded as f64 / c.total_iterations.max(1) as f64,
            c.lu_recorded_from as f64 / c.lu_recorded.max(1) as f64
        );
        println!(
            "  {name}'s solves by Newton iterations (1, 2, 3, 4, 5-8, 9-16, 17-32, more or failed): {:?}",
            c.iteration_counts
        );
        println!(
            "{name}: {nodes} solved nodes, {parts} parts; {:.2} solves and {:.2} Newton iterations a sample ({:.2} a solve){}",
            c.solves as f64 / n,
            c.total_iterations as f64 / n,
            c.total_iterations as f64 / c.solves.max(1) as f64,
            if cfg!(feature = "profile") {
                let it = c.total_iterations.max(1) as f64;
                format!(
                    "; {:.2} us a sample, {:.3} us an iteration (load {:.3}, linear solve {:.3})",
                    c.nanos as f64 / n / 1e3,
                    c.nanos as f64 / it / 1e3,
                    c.nanos_load as f64 / it / 1e3,
                    c.nanos_lu as f64 / it / 1e3
                )
            } else {
                String::new()
            }
        );
    }
    println!(
        "keyboard steps subdivided (Newton failed): {}, failed at 1/1024: {}",
        v.keyboard().subdivided,
        v.keyboard().failed
    );
    println!(
        "{seconds} s of the worst case in {took:.2} s ({:.2} of real time)",
        took / seconds
    );
    println!(
        "solves stopped at their cap without converging: {:?}",
        ca72::unconverged::take()
    );
    Ok(())
}

fn worst(rest: &[String]) -> Res {
    use ca72_lab::worst::{Options, run};
    let mut o = Options {
        rate: 48_000.0,
        block: 256,
        seconds: 60.0,
        serial: false,
        quality: Quality::NoCompromises,
        paced: false,
    };
    let mut args = rest.iter();
    while let Some(a) = args.next() {
        let mut value = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--block" => o.block = value()?.parse().map_err(|e| format!("--block: {e}"))?,
            "--seconds" => o.seconds = value()?.parse().map_err(|e| format!("--seconds: {e}"))?,
            "--serial" => o.serial = true,
            "--quality" => o.quality = quality(value()?)?,
            "--paced" => o.paced = true,
            other => return Err(format!("ca72-lab worst: unknown option {other}")),
        }
    }
    ca72::prof::take();
    REALTIME.store(o.paced, std::sync::atomic::Ordering::Relaxed);
    let r = run(o, audio_priority);
    // Built with `count`: the elementary functions' calls a sample by call site (serial:
    // the voice runs on this thread; the setup before the first block is included).
    let calls = ca72::ulp::calls();
    if !calls.is_empty() {
        let n = o.seconds * o.rate;
        for (at, c) in calls.iter().take(40) {
            println!("{:8.3} a sample: {at}", *c as f64 / n);
        }
    }
    if cfg!(feature = "profile") {
        // Where the time went, us a sample (the parts on the workers run beside the rest).
        let n = o.seconds * o.rate;
        // (The run took each block's times; what is left is since.)
        let mut totals = ca72::prof::take();
        for block in &r.parts {
            for (t, b) in totals.iter_mut().zip(block) {
                *t += b;
            }
        }
        let parts: Vec<String> = ca72::prof::NAMES
            .iter()
            .zip(totals)
            .filter(|(_, t)| *t > 0)
            .map(|(name, t)| format!("{name} {:.2}", t as f64 / n / 1000.0))
            .collect();
        println!("us a sample: {}", parts.join(", "));
    }
    if cfg!(feature = "profile") && !o.serial {
        // The slowest blocks, each thread's work in them (us a sample): the block takes the
        // longest's.
        let mut order: Vec<usize> = (0..r.blocks.len()).collect();
        order.sort_by(|&a, &b| r.blocks[b].total_cmp(&r.blocks[a]));
        let names = ca72::prof::NAMES;
        let at = |t: &[u64; ca72::prof::PARTS], n: &str| {
            t[names.iter().position(|x| *x == n).unwrap_or(0)]
        };
        let group = |t: &[u64; ca72::prof::PARTS], parts: &[&str]| {
            parts.iter().map(|n| at(t, n)).sum::<u64>() as f64 / o.block as f64 / 1000.0
        };
        // Each thread over all blocks: its work's 99th percentile and worst (us a sample),
        // and the blocks whose work alone took longer than the block lasts.
        let threads: [(&str, &[&str]); 6] = [
            ("keyboard", &["keyboard"]),
            ("contours", &["contours", "noise"]),
            ("the VCA's bias", &["vca bias"]),
            ("preamp", &["preamp"]),
            ("front", &["modulation", "oscillators", "control node"]),
            ("back", &["filter", "vca"]),
        ];
        // (A sample lasts 1e6 / rate us.)
        let budget = 1e6 / o.rate;
        for (name, parts) in threads {
            let mut x: Vec<f64> = r.parts.iter().map(|t| group(t, parts)).collect();
            x.sort_by(f64::total_cmp);
            let over = x.iter().filter(|&&y| y > budget).count();
            println!(
                "{name}: us a sample, 99th percentile {:.1}, worst {:.1}; blocks it alone overran: {over}",
                x[(x.len() * 99 / 100).min(x.len() - 1)],
                x.last().copied().unwrap_or(0.0)
            );
        }
        for &b in order.iter().take(8) {
            let t = &r.parts[b];
            println!(
                "block {b} ({:.3} ms), us a sample: keyboard {:.1}, contours {:.1}, the VCA's bias {:.1}, preamp {:.1}, front {:.1}, back {:.1} (filter {:.1}, vca {:.1})",
                r.blocks[b],
                group(t, &["keyboard"]),
                group(t, &["contours", "noise"]),
                group(t, &["vca bias"]),
                group(t, &["preamp"]),
                group(t, &["modulation", "oscillators", "control node"]),
                group(t, &["filter", "vca"]),
                group(t, &["filter"]),
                group(t, &["vca"]),
            );
        }
    }
    if o.serial {
        // The circuits' Newton iterations a block (the same on any machine): the most, the
        // 99th percentile, the mean.
        for (name, counts) in [("keyboard", &r.kbd), ("preamplifier", &r.pre)] {
            let mut its: Vec<u64> = counts.iter().map(|c| c.1).collect();
            its.sort_unstable();
            if let Some(&most) = its.last() {
                println!(
                    "{name}: Newton iterations a block: most {most}, 99th percentile {}, mean {:.0}",
                    its[its.len() * 99 / 100],
                    its.iter().sum::<u64>() as f64 / its.len() as f64
                );
            }
        }
    }
    if cfg!(feature = "profile") && o.serial {
        // The slowest blocks, part by part (us a sample).
        let mut order: Vec<usize> = (0..r.blocks.len()).collect();
        order.sort_by(|&a, &b| r.blocks[b].total_cmp(&r.blocks[a]));
        // (The slowest for the preamplifier too: its work in a block.)
        let mut by_pre: Vec<usize> = (0..r.blocks.len()).collect();
        let pi = ca72::prof::NAMES
            .iter()
            .position(|x| *x == "preamp")
            .unwrap_or(0);
        by_pre.sort_by_key(|&b| std::cmp::Reverse(r.parts[b][pi]));
        order.truncate(3);
        order.extend(by_pre.into_iter().take(3));
        for &b in order.iter() {
            let parts: Vec<String> = ca72::prof::NAMES
                .iter()
                .zip(r.parts[b])
                .filter(|(_, t)| *t > 0)
                .map(|(name, t)| format!("{name} {:.1}", t as f64 / o.block as f64 / 1000.0))
                .collect();
            let (steps, its) = r.kbd.get(b).copied().unwrap_or_default();
            let (psteps, pits) = r.pre.get(b).copied().unwrap_or_default();
            println!(
                "block {b} ({:.3} ms): keyboard {steps} solves, {its} iterations; preamplifier {psteps} solves, {pits} iterations; us a sample: {}",
                r.blocks[b],
                parts.join(", ")
            );
        }
    }
    println!(
        "{} voice, {} s: {}",
        if o.serial { "serial" } else { "threaded" },
        o.seconds,
        r.summary()
    );
    if r.over() > 0 || r.failed > 0 {
        return Err(format!(
            "FAIL: {} of {} blocks took longer than they last",
            r.over(),
            r.blocks.len()
        ));
    }
    println!("PASS");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::set_control;
    use ca72::voice::Panel;
    use serde_json::json;

    /// Every key the patch format documents is taken, each patch in `patches/` parses, and a
    /// misspelt key, a wrong type or a knob out of its range is refused.
    #[test]
    fn patch_keys() {
        let mut p = Panel::default();
        let good = json!({
            "osc1_range": "lo", "osc1_waveform": "sharktooth", "osc1_on": true, "osc1_volume": 0.5,
            "osc2_range": "4", "osc2_waveform": "wide", "osc2_on": true, "osc2_volume": 0.5, "osc2_freq": 0.4,
            "osc3_range": "2", "osc3_waveform": "reverse", "osc3_on": true, "osc3_volume": 0.5, "osc3_freq": 0.6,
            "osc3_control": false, "keyboard_control_1": true, "keyboard_control_2": true,
            "glide_on": true, "decay": false, "noise_on": true, "osc_mod": true, "filter_mod": true,
            "noise": "pink", "cutoff": 0.5, "emphasis": 0.5, "contour_amount": 0.5, "glide": 0.5,
            "noise_volume": 0.5, "mod_mix": 0.5, "mod_wheel": 0.5, "pitch_wheel": -0.5,
            "ext_on": true, "a440": true, "ext_volume": 0.3, "tune": 0.2,
            "filter_attack": 0.1, "filter_decay": 0.2, "filter_sustain": 0.3,
            "loudness_attack": 0.4, "loudness_decay": 0.5, "loudness_sustain": 0.6
        });
        for (k, v) in good.as_object().unwrap() {
            set_control(&mut p, k, v).unwrap_or_else(|e| panic!("{k}: {e}"));
        }
        assert!(!p.osc3_control && p.noise_pink && p.osc_mod && p.filter_mod);
        assert_eq!(
            (p.pitch_wheel, p.mod_wheel, p.loudness_contour.sustain),
            (-0.5, 0.5, 0.6)
        );
        for (k, v) in [
            ("osc3_contrl", json!(true)),
            ("osc1_freq", json!(0.5)),
            ("osc1_waveform", json!("reverse")),
            ("osc3_waveform", json!("sharktooth")),
            ("osc4_on", json!(true)),
            ("noise", json!("red")),
            ("noise_on", json!(1)),
            ("cutoff", json!("high")),
            ("mod_wheel", json!(1.5)),
            ("pitch_wheel", json!(-1.2)),
            ("mod_whel", json!(0.5)),
        ] {
            assert!(set_control(&mut p, k, &v).is_err(), "{k} = {v} was taken");
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("patches");
        for e in std::fs::read_dir(&dir).expect("patches") {
            let path = e.expect("entry").path();
            let j: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
            let mut p = Panel::default();
            for (k, v) in j["panel"].as_object().into_iter().flatten() {
                set_control(&mut p, k, v).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
            for m in j["moves"].as_array().into_iter().flatten() {
                set_control(&mut p, m[1].as_str().expect("key"), &m[2])
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
    }
}
