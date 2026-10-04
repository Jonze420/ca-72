//! The runner against circuits with known answers, and its error catching.

use ca72_spice::{crossings, for_test};

#[test]
fn rc_step_matches_its_exponential() {
    let Some(spice) = for_test("rc_step_matches_its_exponential") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    // 10k and 100n: tau = 1 ms. A step at t = 0.
    let net = "rc step\nv1 in 0 pwl(0 0 1n 1)\nr1 in out 10k\nc1 out 0 100n\n\
               .options reltol=1e-6\n";
    let plots = spice
        .run(net, &["op", "tran 1u 5m 0 1u"], dir.path())
        .expect("runs");
    assert_eq!(plots[0].name, "Operating Point");
    assert!(plots[0].scalar("out").abs() < 1e-9);
    let t = plots[1].vec("time");
    let v = plots[1].vec("out");
    for (&ti, &vi) in t.iter().zip(v).filter(|(t, _)| **t > 1e-5) {
        let want = 1.0 - (-(ti - 1e-9) / 1e-3).exp();
        assert!((vi - want).abs() < 2e-4, "t {ti}: {vi} vs {want}");
    }
    let half = crossings(t, v, 0.5, true);
    assert_eq!(half.len(), 1);
    assert!((half[0] - 1e-3 * 2f64.ln()).abs() < 2e-6, "{}", half[0]);
}

#[test]
fn simulator_errors_are_errors() {
    let Some(spice) = for_test("simulator_errors_are_errors") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    // A transistor whose model does not exist: ngspice must refuse, and so must we.
    let net = "bad model\nv1 c 0 5\nq1 c c 0 nosuchmodel\n";
    let e = spice.run(net, &["op"], dir.path()).expect_err("must fail");
    assert!(e.to_string().to_lowercase().contains("model"), "{e}");
    // A floating node: a singular matrix.
    let net = "floating\nv1 a 0 1\nc1 a b 1n\nc2 b 0 1n\ni1 0 b 1u\n";
    let r = spice.run(net, &["op"], dir.path());
    assert!(
        r.is_err(),
        "a floating node's operating point must fail: {r:?}"
    );
}

#[test]
fn work_directories_with_spaces_work() {
    let Some(spice) = for_test("work_directories_with_spaces_work") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    let work = dir.path().join("a dir with spaces");
    let plots = spice
        .run("divider\nv1 a 0 2\nr1 a b 1k\nr2 b 0 1k\n", &["op"], &work)
        .expect("runs");
    assert!((plots[0].scalar("b") - 1.0).abs() < 1e-9);
}
