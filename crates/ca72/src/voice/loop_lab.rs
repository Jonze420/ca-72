//! FEEDBACK's loop, for finding the preamplifier's model in it (decisions.md R11): the circuit's
//! input recorded inside the loop, replayed into the models open-loop. Test-only.

#![allow(clippy::unwrap_used, dead_code)]

use super::*;
use crate::preamp::{Preamp, PreampCircuit, PreampOut};

pub const SR: f64 = 48_000.0;

/// The two FEEDBACK presets' panels (the factory's Pulse Strut and Undertow Growl), FEEDBACK
/// left out.
pub fn panel(name: &str) -> Panel {
    let osc = |range, waveform, on, volume| OscPanel {
        range,
        waveform,
        on,
        volume,
        freq: 0.5,
    };
    let k = |a, d, s| ContourKnobs {
        attack: a,
        decay: d,
        sustain: s,
    };
    match name {
        "Pulse Strut" => Panel {
            osc: [
                osc(Range::R2, Waveform::NarrowRectangle, true, 0.5),
                osc(Range::R16, Waveform::NarrowRectangle, true, 0.9),
                osc(Range::R16, Waveform::Sawtooth, true, 1.0),
            ],
            osc3_control: true,
            cutoff: 0.1,
            emphasis: 0.7,
            contour_amount: 0.6,
            keyboard_control_1: false,
            keyboard_control_2: true,
            filter_contour: k(0.0, 0.2, 0.4),
            loudness_contour: k(0.0, 0.5, 1.0),
            glide: 0.1,
            glide_on: true,
            decay: false,
            mod_mix: 1.0,
            osc_mod: true,
            filter_mod: true,
            ext_on: true,
            ext_volume: 0.7,
            quality: Quality::Potato,
            ..Panel::default()
        },
        _ => Panel {
            osc: [
                osc(Range::R32, Waveform::Sawtooth, true, 1.0),
                osc(Range::R16, Waveform::Sawtooth, true, 1.0),
                osc(Range::R32, Waveform::ReverseSawtooth, true, 1.0),
            ],
            osc3_control: true,
            cutoff: 0.0,
            emphasis: 0.7,
            contour_amount: 0.8,
            keyboard_control_1: true,
            keyboard_control_2: true,
            filter_contour: k(0.0, 0.72, 0.0),
            loudness_contour: k(0.0, 0.72, 0.9),
            glide: 0.1,
            glide_on: false,
            decay: true,
            mod_mix: 0.0,
            mod_wheel: 1.0,
            filter_mod: true,
            ext_on: true,
            ext_volume: 0.9,
            quality: Quality::Potato,
            ..Panel::default()
        },
    }
}

/// The preamplifier's input (the volts after R9's divider) in each sample of a bass line
/// played with the circuit in the loop, and R9's source resistance.
pub fn record(p: Panel, feedback: f64, seconds: f64) -> (Vec<f64>, f64) {
    let mut v = Voice::prototype(SR);
    v.panel = p;
    v.set_seed(1);
    v.feedback = feedback;
    let (t, r) = ext_volume(p.ext_volume);
    let beat = (0.5 * SR) as usize;
    let roots = [33, 29, 31, 36, 28, 33, 38, 31];
    let n = (seconds * SR) as usize;
    let mut ins = Vec::with_capacity(n);
    let mut held = None;
    for i in 0..n {
        if i % beat == beat * 7 / 8
            && let Some(key) = held.take()
        {
            v.note(key, false);
        }
        if i % beat == 0 {
            let key = roots[(i / beat) % roots.len()];
            v.note(key, true);
            held = Some(key);
        }
        ins.push((0.0 + v.feedback * v.fed_back) * INPUT_VOLTS * t);
        v.tick_jacks(&Jacks::default());
    }
    (ins, r)
}

/// A preamplifier model played open-loop.
pub trait Model {
    fn tick(&mut self, v_in: f64) -> PreampOut;
}

impl Model for Preamp {
    fn tick(&mut self, v_in: f64) -> PreampOut {
        Preamp::tick(self, v_in)
    }
}

/// The circuit as the loop solves it (High Fidelity), from `r_src`.
pub fn circuit(r_src: f64) -> Preamp {
    let mut p = Preamp::new(PreampCircuit::default(), SR, r_src).unwrap();
    p.set_quality(Quality::HighFidelity);
    p
}

/// Potato's plain model, from `r_src`.
pub fn plain(r_src: f64) -> Preamp {
    let mut p = Preamp::new(PreampCircuit::default(), SR, r_src).unwrap();
    p.set_quality(Quality::Potato);
    p.set_source_resistance(r_src);
    p
}

pub fn run(m: &mut dyn Model, ins: &[f64]) -> Vec<PreampOut> {
    ins.iter().map(|&x| m.tick(x)).collect()
}

/// A model's outputs against the circuit's: the amplifier's and the bus current's error
/// RMS over the circuit's RMS (dB), and the lamp's largest difference.
pub fn compare(a: &[PreampOut], b: &[PreampOut]) -> (f64, f64, f64) {
    let from = a.len() / 8;
    let err = |f: &dyn Fn(&PreampOut) -> f64| {
        let (mut e, mut s) = (0.0, 0.0);
        for (x, y) in a[from..].iter().zip(&b[from..]) {
            e += (f(x) - f(y)).powi(2);
            s += f(x).powi(2);
        }
        10.0 * (e / s.max(1e-300)).log10()
    };
    let lamp = a[from..]
        .iter()
        .zip(&b[from..])
        .fold(0.0f64, |m, (x, y)| m.max((x.lamp - y.lamp).abs()));
    (err(&|o| o.amp), err(&|o| o.i_bus), lamp)
}

#[test]
#[ignore = "a lab, run by hand"]
fn the_preamplifiers_input_in_the_loop() {
    for name in ["Pulse Strut", "Undertow Growl"] {
        for knob in [0.3, 0.6, 1.0] {
            for vol in [0.5, 1.0] {
                let p = Panel {
                    ext_volume: vol,
                    ..panel(name)
                };
                let (ins, r) = record(p, feedback_law(knob), 2.0);
                let peak = ins.iter().fold(0.0f64, |m, x| m.max(x.abs()));
                let rms = (ins.iter().map(|x| x * x).sum::<f64>() / ins.len() as f64).sqrt();
                let a = run(&mut circuit(r), &ins);
                let b = run(&mut plain(r), &ins);
                let (ea, eb, el) = compare(&a, &b);
                let clipped = a.iter().filter(|o| o.amp > 9.5 || o.amp < -7.0).count() as f64
                    / a.len() as f64;
                let (hi, lo) = a.iter().fold((f64::MIN, f64::MAX), |(h, l), o| {
                    (h.max(o.amp), l.min(o.amp))
                });
                eprintln!(
                    "{name:<15} FB {:>2} VOL {:>2}: in peak {peak:.3} V rms {rms:.3}, r_src {r:.0}; circuit amp {lo:.2}..{hi:.2}, clipped {:.0} %; plain: amp {ea:.1} dB, bus {eb:.1} dB, lamp {el:.2}",
                    knob * 10.0,
                    vol * 10.0,
                    100.0 * clipped
                );
            }
        }
    }
}

#[test]
#[ignore = "a lab, run by hand"]
fn the_circuit_driven_hard() {
    for (amp_in, r_src) in [(0.5, 0.0), (3.0, 0.0), (0.3, 90e3)] {
        let mut c = circuit(r_src);
        let mut lo = [f64::MAX; 14];
        let mut hi = [f64::MIN; 14];
        let n = (0.5 * SR) as usize;
        let mut amps = Vec::new();
        for i in 0..n {
            let x = amp_in * (std::f64::consts::TAU * 200.0 * i as f64 / SR).sin();
            let o = Model::tick(&mut c, x);
            if i > n / 2 {
                let net = c.circuits().0;
                for k in 1..14 {
                    lo[k] = lo[k].min(net.v(k));
                    hi[k] = hi[k].max(net.v(k));
                }
                amps.push(o.amp);
            }
        }
        let names = [
            "", "P10", "N10", "SRC", "N78", "B27", "C27", "N64", "TAIL", "B32", "N61", "N62",
            "AMP", "OUT",
        ];
        let mut line = String::new();
        for k in 3..14 {
            line.push_str(&format!(" {} {:.3}..{:.3};", names[k], lo[k], hi[k]));
        }
        eprintln!("200 Hz {amp_in} V from {r_src}:{line}");
    }
}

/// The error's power in three bands (under 150 Hz, to 2 kHz, above) against the circuit's
/// whole power, dB: one-pole splits.
pub fn band_errors(a: &[PreampOut], b: &[PreampOut]) -> [f64; 3] {
    let from = a.len() / 8;
    let e: Vec<f64> = a[from..]
        .iter()
        .zip(&b[from..])
        .map(|(x, y)| x.i_bus - y.i_bus)
        .collect();
    let s: f64 = a[from..]
        .iter()
        .map(|x| x.i_bus * x.i_bus)
        .sum::<f64>()
        .max(1e-300);
    let lp = |x: &[f64], hz: f64| -> Vec<f64> {
        let k = (-std::f64::consts::TAU * hz / SR).exp();
        let mut y = 0.0;
        x.iter()
            .map(|v| {
                y = k * y + (1.0 - k) * v;
                y
            })
            .collect()
    };
    let low = lp(&e, 150.0);
    let mid_lp = lp(&e, 2000.0);
    let p = |v: &dyn Fn(usize) -> f64| {
        10.0 * ((0..e.len()).map(|i| v(i).powi(2)).sum::<f64>() / s).log10()
    };
    [
        p(&|i| low[i]),
        p(&|i| mid_lp[i] - low[i]),
        p(&|i| e[i] - mid_lp[i]),
    ]
}

#[test]
#[ignore = "a lab, run by hand"]
fn the_models_open_loop() {
    use crate::preamp::InLoop;
    let models = [InLoop::Plain, InLoop::Delayed];
    for name in ["Pulse Strut", "Undertow Growl"] {
        for knob in [0.3, 0.6, 1.0] {
            for vol in [0.5, 1.0] {
                let p = Panel {
                    ext_volume: vol,
                    ..panel(name)
                };
                let (ins, r) = record(p, feedback_law(knob), 3.0);
                let a = run(&mut circuit(r), &ins);
                let mut line = String::new();
                for m in models {
                    let mut pre = plain(r);
                    pre.set_delayed(m == InLoop::Delayed);
                    let b = run(&mut pre, &ins);
                    let (_, eb, _) = compare(&a, &b);
                    let [l, mi, h] = band_errors(&a, &b);
                    line.push_str(&format!("  {m:?} {eb:.1} ({l:.0}/{mi:.0}/{h:.0})"));
                }
                eprintln!(
                    "{name:<15} FB {:>2} VOL {:>2}:{line}",
                    knob * 10.0,
                    vol * 10.0
                );
            }
        }
    }
}
