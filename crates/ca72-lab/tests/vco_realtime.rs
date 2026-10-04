//! The real-time oscillator against the circuit reference: both tuned by Folkman's 1973
//! procedure, then compared note by note (circuits/boards/reference/vco1-core.json,
//! from `ca72-lab core`: ngspice's circuit tuned by the same procedure).

use ca72::tuning::{self, Range, Tuning, measure_hz, osc1_inputs};
use ca72::vco::Vco;
use ca72_lab::circuits_dir;

fn range_of(name: &str) -> Range {
    match name {
        "Lo" => Range::Lo,
        "R32" => Range::R32,
        "R16" => Range::R16,
        "R8" => Range::R8,
        "R4" => Range::R4,
        _ => Range::R2,
    }
}

#[test]
fn tuned_realtime_oscillator_tracks_the_tuned_circuit() {
    let sr = 48_000.0;
    let mut vco = Vco::new(sr, 4);
    let t0 = std::time::Instant::now();
    let (t, plays) = tuning::folkman_1973(&mut vco, sr, Tuning::default());
    eprintln!(
        "tuned in {plays} notes, {:.2} s: {t:?}",
        t0.elapsed().as_secs_f64()
    );
    let path = circuits_dir().join("boards/reference/vco1-core.json");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("reference")).expect("json");
    let mut worst = (0.0f64, 0.0f64);
    for p in json["points"].as_array().expect("points") {
        let range = range_of(p["range"].as_str().expect("range"));
        let key = p["key"].as_u64().expect("key") as f64;
        let spice_hz = p["hz"].as_f64().expect("hz");
        let i_in = vco.expo.input_current(&osc1_inputs(&t, key, range));
        let hz = measure_hz(&mut vco, sr, i_in, 6);
        let cents = 1200.0 * (hz / spice_hz).log2();
        eprintln!(
            "{range:?} key {key}: ngspice {spice_hz:.4} Hz, real-time {hz:.4} Hz, {cents:+.3} cents"
        );
        if range == Range::Lo {
            worst.1 = worst.1.max(cents.abs());
        } else {
            worst.0 = worst.0.max(cents.abs());
        }
    }
    eprintln!(
        "worst 32'..2' {:.3} cents, LO {:.3} cents",
        worst.0, worst.1
    );
    assert!(worst.0 < 0.2 && worst.1 < 1.0, "{worst:?}");
}

/// Harmonic amplitudes of one period of a waveform given at increasing times (one period
/// from `t0`), up to `n` harmonics: |c_k| of its Fourier series (peak amplitudes).
fn harmonics_of_period(t: &[f64], y: &[f64], t0: f64, period: f64, n: usize) -> Vec<f64> {
    let m = 1 << 14;
    let ys = ca72_spice::resample(t, y, t0, m as f64 / period, m);
    (1..=n)
        .map(|k| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in ys.iter().enumerate() {
                let ph = 2.0 * std::f64::consts::PI * k as f64 * i as f64 / m as f64;
                re += v * ph.cos();
                im -= v * ph.sin();
            }
            2.0 * (re * re + im * im).sqrt() / m as f64
        })
        .collect()
}

/// Harmonic amplitudes of a rendered signal at a known frequency (projection over a whole
/// number of periods), and its largest spectral peak below 20 kHz that is not a harmonic,
/// relative to the fundamental in dB (aliasing), from a Blackman-Harris windowed FFT.
fn harmonics_of_render(x: &[f64], sr: f64, hz: f64, n: usize) -> (Vec<f64>, f64) {
    let periods = (x.len() as f64 * hz / sr).floor();
    let len = (periods * sr / hz).floor() as usize;
    let xs = &x[..len];
    let mut amps = Vec::new();
    for k in 1..=n {
        let w = 2.0 * std::f64::consts::PI * k as f64 * hz / sr;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in xs.iter().enumerate() {
            re += v * (w * i as f64).cos();
            im += v * (w * i as f64).sin();
        }
        amps.push(2.0 * (re * re + im * im).sqrt() / len as f64);
    }
    let m = 1usize << 16;
    let mut re: Vec<f64> = x[..m.min(x.len())].to_vec();
    re.resize(m, 0.0);
    let mean = re.iter().sum::<f64>() / m as f64;
    for (i, v) in re.iter_mut().enumerate() {
        let p = 2.0 * std::f64::consts::PI * i as f64 / (m - 1) as f64;
        let w = 0.35875 - 0.48829 * p.cos() + 0.14128 * (2.0 * p).cos() - 0.01168 * (3.0 * p).cos();
        *v = (*v - mean) * w;
    }
    let mut im = vec![0.0; m];
    ca72_analysis::fft::fft(&mut re, &mut im);
    let mag: Vec<f64> = (0..m / 2)
        .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt())
        .collect();
    let bin = sr / m as f64;
    let fundamental = mag[(hz / bin).round() as usize - 3..(hz / bin).round() as usize + 4]
        .iter()
        .cloned()
        .fold(0.0, f64::max);
    let mut alias = 0.0f64;
    for (i, &v) in mag.iter().enumerate().skip(1) {
        let f = i as f64 * bin;
        if f > 20_000.0 {
            break;
        }
        let h = (f / hz).round();
        // Away from every harmonic by more than the window's main lobe (4 bins) and 20 Hz.
        if (f - h * hz).abs() > (4.0 * bin).max(20.0) {
            alias = alias.max(v);
        }
    }
    (amps, 20.0 * (alias / fundamental).log10())
}

/// The waveforms' harmonics against the circuit's (every harmonic below 20 kHz within 1 %
/// of the strongest), and the aliasing left in the output (below -65 dB).
#[test]
fn realtime_waveforms_match_the_circuit_below_20_khz() {
    let Some(spice) = ca72_spice::for_test("realtime_waveforms_match_the_circuit") else {
        return;
    };
    use ca72_lab::bench::Solver;
    use ca72_lab::vco as lab;
    let sr = 48_000.0;
    let t = Tuning {
        r11: 183.65,
        a8: 0.13518,
        octave_step: 0.29889,
        tune: 7.5315,
    };
    let trims = lab::Trims {
        r11: t.r11,
        a8: t.a8,
        octave_step: t.octave_step,
        ..lab::Trims::default()
    };
    let mut report = String::new();
    let mut failures = Vec::new();
    for (range, lrange, key) in [
        (Range::R8, lab::Range::R8, 4u32),
        (Range::R4, lab::Range::R4, 24),
        (Range::R2, lab::Range::R2, 42),
    ] {
        let ctl = lab::Controls {
            kbd: lab::key_volts(f64::from(key)),
            range: lrange,
            tune: t.tune,
            ..lab::Controls::default()
        };
        let solver = Solver {
            steps_per_period: 4000.0,
            ..Solver::default()
        };
        let m = lab::measure(
            &spice,
            &ca72_lab::work_dir(&format!("waves-{range:?}-{key}")),
            &trims,
            &ctl,
            solver,
            4,
        )
        .expect("ngspice");
        let tt = m.plot.vec("time");
        let period = 1.0 / m.hz;
        let t0 = tt[tt.len() - 1] - 1.5 * period;
        let n = (20_000.0 / m.hz).floor() as usize;
        // The real-time oscillator at the same trims, 0.5 s.
        let mut vco = Vco::new(sr, 4);
        vco.expo.r11 = t.r11;
        vco.expo.a8 = t.a8;
        let i_in = vco
            .expo
            .input_current(&osc1_inputs(&t, f64::from(key), range));
        let hz = measure_hz(&mut vco, sr, i_in, 8);
        vco.reset();
        let len = (sr * 1.5) as usize;
        let skip = 2400;
        let (mut saw, mut tri, mut rect) = (Vec::new(), Vec::new(), Vec::new());
        for i in 0..len + skip {
            let o = vco.tick(i_in, 0.0);
            if i >= skip {
                saw.push(o.saw);
                tri.push(o.tri);
                rect.push(o.rect);
            }
        }
        for (name, node, x) in [
            ("saw", "saw", &saw),
            ("tri", "tri", &tri),
            ("rect", "rect", &rect),
        ] {
            let theirs = harmonics_of_period(tt, m.plot.vec(node), t0, period, n);
            let (ours, alias_db) = harmonics_of_render(x, sr, hz, n);
            // Each harmonic's error relative to the strongest harmonic: what the difference
            // weighs against the waveform, so harmonics near the spectrum's nulls do not
            // count more than they sound.
            let peak = theirs.iter().cloned().fold(0.0, f64::max);
            let mut worst = (0usize, f64::MIN);
            for k in 0..n {
                let d = 20.0 * ((ours[k] - theirs[k]).abs() / peak).log10();
                if d > worst.1 {
                    worst = (k + 1, d);
                }
            }
            report.push_str(&format!(
                "{range:?} key {key} ({:.1} Hz) {name}: {n} harmonics, worst error {:.1} dB of the peak at #{}, largest alias {alias_db:.1} dB\n",
                m.hz, worst.1, worst.0
            ));
            if worst.1 > -40.0 || alias_db > -65.0 {
                failures.push(format!("{range:?} {key} {name}"));
            }
        }
    }
    eprintln!("{report}");
    assert!(failures.is_empty(), "{failures:?}\n{report}");
}
