//! POLY's voices shared with workers give the same samples, to the bit, as every voice on
//! the host's thread (decisions.md R11): a factory preset and a FEEDBACK one, ten voices,
//! the chords of `preset_cost.rs`, 256-frame blocks, with no workers and with three; and so
//! with the host's thread flushing denormals to zero, as nih-plug has it in `process`
//! (the workers take its flush bits; R18).

#![allow(clippy::unwrap_used)]

mod common;

use ca72_plugin::engine::Engine;
use ca72_plugin::library::factory;
use common::{Chords, controls_of, play_block};

fn render(name: &str, workers: usize, flush: u32) -> Vec<f32> {
    let s = factory().iter().find(|s| s.name == name).unwrap();
    let mut c = controls_of(s);
    c.poly = true;
    c.voices = 10;
    let mut e = Engine::new();
    e.set(&c);
    e.prepare(48_000.0, 1);
    assert_eq!(e.start_workers(workers, None), workers);
    let mut chords = Chords::new(10, 48_000.0);
    let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
    let mut events = Vec::with_capacity(64);
    let mut out = Vec::new();
    let was = ca72_rt::flush_mode();
    ca72_rt::set_flush_mode(flush);
    for i in 0..(0.8 * 48_000.0 / 256.0) as usize {
        play_block(&mut e, &mut chords, i * 256, &mut l, &mut r, &mut events);
        out.extend(l.iter().chain(&r));
    }
    ca72_rt::set_flush_mode(was);
    out
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn workers_give_the_same_samples_as_one_thread() {
    for name in ["Stacked Fifths", "Pulse Strut"] {
        for flush in [0, ca72_rt::FLUSH] {
            let (a, b) = (render(name, 0, flush), render(name, 3, flush));
            assert!(a.iter().any(|x| x.abs() > 0.01), "{name}: silent");
            assert!(
                same(&a, &b),
                "{name}: the workers' samples differ (flush bits {flush:#x})"
            );
        }
    }
}
