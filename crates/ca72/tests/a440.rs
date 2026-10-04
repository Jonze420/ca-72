//! The real-time A-440 (circuit No. 13) against its derived table: the pitch the factory
//! trims it to, the steady state's harmonics as the circuit gives them, and its start.

use ca72::a440::{A440, HZ};
use ca72::a440_table as t;

#[test]
fn the_a440_plays_its_circuits_waveform_at_440_hz() {
    let rate = 48_000.0;
    let mut a = A440::new(rate);
    assert_eq!(a.tick(false), 0.0);
    let n = (1.0 * rate) as usize;
    let x: Vec<f64> = (0..n).map(|_| a.tick(true)).collect();
    // The steady state over whole periods from 0.5 s.
    let start = (0.5 * rate) as usize;
    let periods = ((n - start) as f64 * HZ / rate).floor();
    let end = start + (periods * rate / HZ).round() as usize;
    let comp = |k: usize| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in x[start..end].iter().enumerate() {
            let w = 2.0 * std::f64::consts::PI * k as f64 * HZ * i as f64 / rate;
            re += v * w.cos();
            im -= v * w.sin();
        }
        2.0 * (re * re + im * im).sqrt() / (end - start) as f64
    };
    let mean = x[start..end].iter().sum::<f64>() / (end - start) as f64;
    let mut worst = 0.0f64;
    for k in 1..=t::AMP.len() {
        worst = worst.max((comp(k) - t::AMP[k - 1]).abs());
    }
    // Its start: the fundamental's amplitude at 20 ms and 100 ms against the table's.
    let at = |s: f64| {
        let c = (s * rate) as usize;
        let w = (rate / HZ) as usize;
        let seg = &x[c - w / 2..c - w / 2 + w];
        let m = seg.iter().sum::<f64>() / w as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in seg.iter().enumerate() {
            let ph = 2.0 * std::f64::consts::PI * HZ * i as f64 / rate;
            re += (v - m) * ph.cos();
            im -= (v - m) * ph.sin();
        }
        2.0 * (re * re + im * im).sqrt() / w as f64 / t::AMP[0]
    };
    let (e20, e100) = (at(0.02), at(0.1));
    eprintln!(
        "steady state: mean {mean:.4} V (table {:.4}), harmonics within {worst:.2e} V; fundamental at 20 ms {e20:.3} (table {:.3}), at 100 ms {e100:.3} (table {:.3})",
        t::DC,
        t::START_AMP[20],
        t::START_AMP[100]
    );
    assert!((mean - t::DC).abs() < 1e-6);
    assert!(worst < 1e-5, "{worst}");
    assert!((e20 - t::START_AMP[20]).abs() < 0.02 && (e100 - t::START_AMP[100]).abs() < 0.02);
}
