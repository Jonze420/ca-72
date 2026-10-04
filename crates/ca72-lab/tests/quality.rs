//! High Fidelity against No Compromises on the worst case's load (every knob, key and jack
//! moving; `ca72-lab hifi` checks every scenario): within its limits on each path
//! (`ca72_lab::quality`), and lighter.

use ca72::voice::{Quality, Voice};
use ca72_lab::{quality, worst};

#[test]
fn high_fidelity_is_within_its_limits_and_lighter() {
    let (rate, seconds) = (48_000.0, 3.0);
    let mut counts = Vec::new();
    let mut renders = Vec::new();
    for q in [Quality::NoCompromises, Quality::HighFidelity] {
        let mut v = Voice::prototype(rate);
        renders.push(worst::render_loop(&mut v, rate, seconds, q, true));
        let (k, p) = (v.keyboard().circuit(), v.preamp().circuits().0);
        counts.push((k.total_iterations, p.total_iterations));
    }
    let mut held = Voice::prototype(rate);
    held.hold_control_quality(Some(Quality::NoCompromises));
    let (held, _) = worst::render_loop(&mut held, rate, seconds, Quality::HighFidelity, false);
    let m = quality::measure(&renders[0].1, &renders[1].1, &renders[0].0, &held);
    let crossed = m.crossed("worst", rate);
    assert!(crossed.is_empty(), "{crossed:?}");
    // Lighter: the keyboard's and the preamplifier's Newton iterations.
    let ratio = |a: u64, b: u64| b as f64 / a as f64;
    let (kr, pr) = (
        ratio(counts[0].0, counts[1].0),
        ratio(counts[0].1, counts[1].1),
    );
    println!(
        "{m:?}; High Fidelity's Newton iterations against No Compromises': keyboard {kr:.3}, preamplifier {pr:.3}"
    );
    assert!(
        kr < 0.97 && pr < 0.9,
        "keyboard {kr:.3}, preamplifier {pr:.3}"
    );
}
