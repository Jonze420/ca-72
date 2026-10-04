//! The panel as drawn matches the design the owner approved (`approved.png`: the mock-up at one
//! panel unit a pixel, every control at half its travel).

use ca72_panel::art::{PLATE_H, PLATE_X, PLATE_Y};
use ca72_panel::{CONTROLS, Renderer, Scene};
use resvg::tiny_skia::Pixmap;

/// The mock-up drew its name plate in a fallback face, so the plate is left out.
fn outside_the_plate(x: u32, y: u32) -> bool {
    let (x, y) = (f64::from(x), f64::from(y));
    !(PLATE_X - 8.0..PLATE_X + 290.0).contains(&x)
        || !(PLATE_Y - 8.0..PLATE_Y + PLATE_H + 12.0).contains(&y)
}

#[test]
fn the_panel_is_drawn_as_approved() {
    let approved = Pixmap::load_png(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/approved.png"))
        .expect("the approved panel");
    let mut r = Renderer::new(1.0, 1.0);
    r.render(&Scene {
        values: [0.5; CONTROLS.len()],
        ..Scene::default()
    });
    let drawn = r.frame();
    assert_eq!(
        (drawn.width(), drawn.height()),
        (approved.width(), approved.height())
    );

    let mut differences: Vec<u8> = Vec::new();
    for (i, (a, b)) in drawn.pixels().iter().zip(approved.pixels()).enumerate() {
        let (x, y) = (i as u32 % drawn.width(), i as u32 / drawn.width());
        if outside_the_plate(x, y) {
            let d = [
                a.red().abs_diff(b.red()),
                a.green().abs_diff(b.green()),
                a.blue().abs_diff(b.blue()),
            ];
            differences.push(d.into_iter().max().unwrap_or(0));
        }
    }
    let mean = differences.iter().map(|&d| f64::from(d)).sum::<f64>() / differences.len() as f64;
    differences.sort_unstable();
    let p99 = differences[differences.len() * 99 / 100];
    // The margin, for the record on each machine (`-- --nocapture`).
    println!(
        "the panel as drawn against approved.png: mean difference {mean:.3}, 99th percentile {p99} (passes under 1.5 and 32)"
    );
    // Text rasterises a little lighter here than in the browser that drew the mock-up.
    assert!(
        mean < 1.5 && p99 < 32,
        "mean difference {mean:.2}, 99th percentile {p99}"
    );
}
