//! Every iterative solve that should converge does, in every quality mode, under the worst
//! case's load (every knob, key and jack moving): none stops at its cap
//! (`ca72::unconverged`). No Compromises' contours once stopped short hundreds of
//! times a few seconds, their transistors' steps held too short to arrive.

use ca72::unconverged;
use ca72::voice::{Quality, Voice};
use ca72_lab::worst;

#[test]
fn every_solve_converges_in_every_mode() {
    let (rate, seconds) = (48_000.0, 3.0);
    let mut stopped = Vec::new();
    for q in [
        Quality::NoCompromises,
        Quality::HighFidelity,
        Quality::Potato,
    ] {
        let mut v = Voice::prototype(rate);
        unconverged::take();
        worst::render_loop(&mut v, rate, seconds, q, false);
        let counts = unconverged::take();
        println!("{q:?}: {counts:?}");
        if !counts.is_empty() || v.keyboard().failed > 0 || v.revsaw().failed > 0 {
            stopped.push(format!(
                "{q:?}: {counts:?}, keyboard {}, reverse sawtooth {}",
                v.keyboard().failed,
                v.revsaw().failed
            ));
        }
    }
    assert!(stopped.is_empty(), "{stopped:?}");
}
