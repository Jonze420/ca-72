//! The real-time crate's derived data still matches its sources: the triangle and output
//! stage tables regenerated from ngspice, and the transistor constants against
//! `mm-devices.lib`.

use ca72_lab::bench::Solver;
use ca72_lab::{circuits_dir, vco, work_dir};

#[test]
fn tables_match_the_circuit() {
    let Some(spice) = ca72_spice::for_test("tables_match_the_circuit") else {
        return;
    };
    let tri =
        vco::triangle_table(&spice, &work_dir("tables-check"), Solver::default()).expect("sweep");
    assert_eq!(tri.len(), ca72::tables::TRI.len());
    assert_eq!(vco::TRI_BUF_MIN, ca72::tables::TRI_MIN);
    assert_eq!(vco::TRI_BUF_STEP, ca72::tables::TRI_STEP);
    for (i, (a, b)) in tri.iter().zip(ca72::tables::TRI.iter()).enumerate() {
        assert!(
            (a - b).abs() < 2e-6,
            "TRI[{i}]: ngspice {a}, committed {b}: run `ca72-lab tables`"
        );
    }
    let out =
        ca72_lab::vcf::output_stage_table(&spice, &work_dir("tables-check-out"), Solver::default())
            .expect("sweep");
    use ca72::tables::{OUT_DI, OUT_DI_MIN, OUT_DI_STEP, OUT_DIB, OUT_DIB_MIN, OUT_DIB_STEP};
    use ca72_lab::vcf::{OUT_W_MIN, OUT_W_STEP};
    assert_eq!((OUT_W_MIN, OUT_W_STEP), (OUT_DI_MIN, OUT_DI_STEP));
    assert_eq!((OUT_W_MIN, OUT_W_STEP), (OUT_DIB_MIN, OUT_DIB_STEP));
    for (name, fresh, committed) in [
        ("OUT_DI", &out.di, &OUT_DI[..]),
        ("OUT_DIB", &out.dib, &OUT_DIB[..]),
    ] {
        assert_eq!(fresh.len(), committed.len(), "{name}");
        for (i, (a, b)) in fresh.iter().zip(committed).enumerate() {
            assert!(
                (a - b).abs() <= 1e-15 + 1e-7 * b.abs(),
                "{name}[{i}]: ngspice {a}, committed {b}: run `ca72-lab tables`"
            );
        }
    }
}

#[test]
fn the_a440_table_matches_the_circuit() {
    let Some(spice) = ca72_spice::for_test("the_a440_table_matches_the_circuit") else {
        return;
    };
    use ca72::a440_table as t;
    use ca72_lab::board4ext::{A440Bench, a440_table};
    let fresh = a440_table(
        &spice,
        &work_dir("tables-check-a440"),
        &A440Bench::default(),
        Solver::default(),
    )
    .expect("a440");
    let close = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol;
    assert!(
        close(fresh.a68, t::A68, 1e-6)
            && close(fresh.hz, t::HZ, 1e-6)
            && close(fresh.dc, t::DC, 1e-6),
        "run `ca72-lab tables`"
    );
    for (k, (&(a, p), (&ca, &cp))) in fresh
        .harmonics
        .iter()
        .zip(t::AMP.iter().zip(&t::PHASE))
        .enumerate()
    {
        assert!(
            close(a, ca, 1e-7) && (a < 1e-4 || close(p, cp, 1e-5)),
            "harmonic {}: run `ca72-lab tables`",
            k + 1
        );
    }
    for (i, (&d, &a)) in fresh.start_dc.iter().zip(&fresh.start_amp).enumerate() {
        assert!(
            close(d, t::START_DC[i], 1e-7) && close(a, t::START_AMP[i], 1e-7),
            "start {i}: run `ca72-lab tables`"
        );
    }
}

/// The value of `param` in the `.model name` card of a SPICE library.
fn model_param(lib: &str, model: &str, param: &str) -> f64 {
    let start = lib
        .to_ascii_lowercase()
        .find(&format!(".model {}", model.to_ascii_lowercase()))
        .unwrap_or_else(|| panic!("no model {model}"));
    let mut card = String::new();
    for (i, line) in lib[start..].lines().enumerate() {
        if i > 0 && !line.trim_start().starts_with('+') {
            break;
        }
        card.push(' ');
        card.push_str(line.trim_start_matches('+'));
    }
    let key = format!(" {}=", param.to_ascii_lowercase());
    let lower = card.to_ascii_lowercase().replace(['(', ')'], " ");
    let at = lower
        .find(&key)
        .unwrap_or_else(|| panic!("{model} has no {param}"));
    let v = lower[at + key.len()..]
        .split_whitespace()
        .next()
        .unwrap_or("");
    let (num, scale) = match v.chars().last() {
        Some('p') => (&v[..v.len() - 1], 1e-12),
        Some('n') => (&v[..v.len() - 1], 1e-9),
        Some('u') => (&v[..v.len() - 1], 1e-6),
        Some('m') => (&v[..v.len() - 1], 1e-3),
        Some('k') => (&v[..v.len() - 1], 1e3),
        _ => (v, 1.0),
    };
    num.parse::<f64>()
        .unwrap_or_else(|_| panic!("{model} {param} = {v}"))
        * scale
}

/// The value of `param`, or `None` if the card does not set it.
fn model_param_opt(lib: &str, model: &str, param: &str) -> Option<f64> {
    let start = lib
        .to_ascii_lowercase()
        .find(&format!(".model {}", model.to_ascii_lowercase()))?;
    let mut card = String::new();
    for (i, line) in lib[start..].lines().enumerate() {
        if i > 0 && !line.trim_start().starts_with('+') {
            break;
        }
        card.push(' ');
        card.push_str(line.trim_start_matches('+'));
    }
    let lower = card.to_ascii_lowercase().replace(['(', ')'], " ");
    lower
        .contains(&format!(" {}=", param.to_ascii_lowercase()))
        .then(|| model_param(lib, model, param))
}

#[test]
fn device_constants_match_the_model_library() {
    use ca72::devices::{Bjt, CA3046};
    use ca72::vca::Q2N4058;
    use ca72::vcf::{TIS92, TIS93, TIS97};
    let lib = std::fs::read_to_string(circuits_dir().join("models/mm-devices.lib")).expect("lib");
    let check = |model: &str, c: Bjt| {
        for (p, v, default) in [
            ("is", c.is, None),
            ("bf", c.bf, None),
            ("ise", c.ise, None),
            ("ne", c.ne, None),
            ("vaf", c.vaf, None),
            ("ikf", c.ikf, Some(f64::INFINITY)),
            ("br", c.br, Some(1.0)),
            ("rb", c.rb, None),
            ("re", c.re, None),
            ("rc", c.rc, Some(0.0)),
            ("cje", c.cje, Some(0.0)),
            ("vje", c.vje, Some(0.75)),
            ("mje", c.mje, Some(0.33)),
            ("cjc", c.cjc, Some(0.0)),
            ("vjc", c.vjc, Some(0.75)),
            ("mjc", c.mjc, Some(0.33)),
            ("tf", c.tf, Some(0.0)),
            ("xti", c.xti, None),
            ("xtb", c.xtb, None),
            ("eg", c.eg, None),
            ("tnom", c.tnom, None),
        ] {
            let lib_v = match (model_param_opt(&lib, model, p), default) {
                (Some(v), _) => v,
                (None, Some(d)) => d,
                (None, None) => panic!("{model} has no {p}"),
            };
            assert!(
                lib_v == v || (lib_v - v).abs() <= 1e-12 * v.abs().max(1e-30),
                "{model} {p}: lib {lib_v}, Rust {v}"
            );
        }
    };
    check("CA3046_NPN", CA3046);
    check("QTIS97", TIS97);
    check("QTIS92", TIS92);
    check("QTIS93", TIS93);
    check("Q2N4058", Q2N4058);
    check("Q2N3392", ca72::contour::Q2N3392);
    use ca72::contour::{D1N34A, D1N4004};
    for (model, d) in [("D1N34A", D1N34A), ("D1N4004", D1N4004)] {
        for (p, v, default) in [
            ("is", d.is, None),
            ("n", d.n, Some(1.0)),
            ("rs", d.rs, Some(0.0)),
            ("eg", d.eg, Some(1.11)),
            ("xti", d.xti, Some(3.0)),
        ] {
            let lib_v = match (model_param_opt(&lib, model, p), default) {
                (Some(v), _) => v,
                (None, Some(x)) => x,
                (None, None) => panic!("{model} has no {p}"),
            };
            assert!(
                (lib_v - v).abs() <= 1e-12 * v.abs().max(1e-30),
                "{model} {p}: lib {lib_v}, Rust {v}"
            );
        }
    }
    let param = |model: &str, p: &str, default: Option<f64>| match (
        model_param_opt(&lib, model, p),
        default,
    ) {
        (Some(v), _) => v,
        (None, Some(x)) => x,
        (None, None) => panic!("{model} has no {p}"),
    };
    let same = |model: &str, p: &str, lib_v: f64, v: f64| {
        assert!(
            (lib_v - v).abs() <= 1e-12 * v.abs().max(1e-30),
            "{model} {p}: lib {lib_v}, Rust {v}"
        );
    };
    // The keyboard's CR5: its charges (VJ and FC at ngspice's defaults, 1 and 0.5).
    use ca72::keyboard::{CR5_CAP, CR5_TT};
    for (p, v, default) in [
        ("cjo", CR5_CAP.cj0, None),
        ("m", CR5_CAP.m, None),
        ("vj", CR5_CAP.vj, Some(1.0)),
        ("fc", CR5_CAP.fc, Some(0.5)),
        ("tt", CR5_TT, None),
    ] {
        same("D1N4004", p, param("D1N4004", p, default), v);
    }
    // The 2N4303 (ngspice's JFET level 1; N and FC at their defaults when absent).
    let j = ca72::mna::J2N4303;
    for (p, v, default) in [
        ("vto", j.vto, None),
        ("beta", j.beta, None),
        ("lambda", j.lambda, None),
        ("rd", j.rd, None),
        ("rs", j.rs, None),
        ("cgs", j.cgs, None),
        ("cgd", j.cgd, None),
        ("pb", j.pb, None),
        ("fc", j.fc, Some(0.5)),
        ("is", j.is, None),
        ("n", j.n, Some(1.0)),
    ] {
        same("J2N4303", p, param("J2N4303", p, default), v);
    }
}
