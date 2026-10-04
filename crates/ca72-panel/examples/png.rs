//! Renders the panel to a PNG: `cargo run -p ca72-panel --example png -- out.png [scale]`
//! (pixels a panel unit, 0.5 by default).

use ca72_panel::{CONTROLS, Renderer, Scene};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "panel.png".into());
    let scale: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0.5);
    let t0 = std::time::Instant::now();
    let mut r = Renderer::new(scale, 1.0);
    let built = t0.elapsed();
    let mut scene = Scene::default();
    // The plugin's defaults: as the mock-up was drawn.
    // `CA72_ALL=0.5`: every control at one value instead.
    let all: Option<f64> = std::env::var("CA72_ALL").ok().and_then(|s| s.parse().ok());
    for (i, c) in CONTROLS.iter().enumerate() {
        scene.values[i] = all.unwrap_or_else(|| default_of(c.param));
    }
    let t1 = std::time::Instant::now();
    r.render(&scene);
    let first = t1.elapsed();
    scene.values[0] = 0.8;
    let t2 = std::time::Instant::now();
    r.render(&scene);
    let one = t2.elapsed();
    scene.values[0] = default_of("tune");
    r.render(&scene);
    eprintln!("background {built:?}, every layer {first:?}, one knob {one:?}");
    if let Err(e) = r.frame().save_png(&out) {
        eprintln!("{out}: {e}");
        std::process::exit(1);
    }
}

/// The plugin's default for a parameter, normalized.
fn default_of(param: &str) -> f64 {
    match param {
        "tune" | "osc2_frequency" | "osc3_frequency" | "cutoff" | "pitch_wheel" => 0.5,
        "osc1_range" | "osc2_range" | "osc3_range" => 0.6,
        "osc1_waveform" | "osc2_waveform" | "osc3_waveform" => 0.4,
        "osc1_volume" | "osc2_volume" | "osc3_volume" => 0.7,
        "osc1_on" | "osc2_on" | "osc3_on" | "main_output" | "keyboard_control_1"
        | "keyboard_control_2" | "osc_mod" => 1.0,
        "emphasis" | "glide" | "mod_mix" | "mod_wheel" | "ext_volume" | "noise_volume" => 0.0,
        "contour_amount" | "filter_sustain" | "loudness_sustain" | "volume" => 0.5,
        "filter_attack" | "loudness_attack" => 0.1,
        "filter_decay" | "loudness_decay" => 0.4,
        _ => 0.0,
    }
}
