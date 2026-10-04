//! The plug-in's own controls, drawn under the panel and always with it (decisions.md, "POLY"
//! and "ANALOG and SPREAD"; R12): at the row's left the presets' selector
//! ([`crate::presets::bar_svg`]), then POLY (a switch), VOICES (2 to 10, stepped), and ENTROPY
//! and SPREAD, each a slider (0 to 100 %) and a switch that turns it off and back on at its
//! amount. Drawn in the panel's colours and lettering, at the panel's scale, `STRIP_H` panel
//! units tall across its whole width.

use std::fmt;

use resvg::tiny_skia::{Color, Pixmap, Transform};
use resvg::usvg;

use crate::art::{self, W};
use crate::fonts::{FAMILY, Fonts};
use crate::presets::{BAR_END, BarScene, bar_svg};
use crate::svg::{N, Svg, colour, escape, put};

/// The strip's height, panel units.
pub const STRIP_H: f64 = 96.0;

/// One of the two amounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    Entropy,
    Spread,
}

/// What a pointer finds on the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripTarget {
    Poly,
    /// VOICES' step down and up.
    Fewer,
    More,
    /// An amount's slider, and its switch.
    Slider(Amount),
    Switch(Amount),
}

/// What the strip shows: its controls and the presets' selector.
#[derive(Clone, Debug, PartialEq)]
pub struct StripScene {
    pub poly: bool,
    pub voices: u32,
    /// ENTROPY and SPREAD, 0..1.
    pub entropy: f64,
    pub spread: f64,
    pub bar: BarScene,
    pub hover: Option<StripTarget>,
}

impl Default for StripScene {
    fn default() -> Self {
        StripScene {
            poly: false,
            voices: 4,
            entropy: 0.0,
            spread: 0.0,
            bar: BarScene::default(),
            hover: None,
        }
    }
}

// The layout, panel units across and strip units down (0 at the strip's top).
const MID: f64 = STRIP_H / 2.0;
const TEXT: f64 = 26.0;
/// POLY's switch (its label ends 20 before it), and where VOICES' label ends less 130 (its
/// steps follow): right of the presets' selector ([`BAR_END`]).
const POLY_X: f64 = 1240.0;
const SWITCH_W: f64 = 92.0;
const SWITCH_H: f64 = 44.0;
const VOICES_X: f64 = 1345.0;
const STEP_W: f64 = 46.0;
/// Each amount: its label's x, its slider's track from and to, its value's x, its switch's x.
const AMOUNTS: [(Amount, &str, f64, f64, f64, f64, f64); 2] = [
    (
        Amount::Entropy,
        "ENTROPY",
        1720.0,
        1880.0,
        2280.0,
        2350.0,
        2420.0,
    ),
    (
        Amount::Spread,
        "SPREAD",
        2620.0,
        2780.0,
        3180.0,
        3250.0,
        3320.0,
    ),
];
/// The track's thickness and the thumb's radius.
const TRACK: f64 = 8.0;
const THUMB: f64 = 17.0;

const DIM: &str = "#8a909c";
const RAISED: &str = "#272b32";
const BORDER: &str = "#4a4640";
const ACCENT: &str = "#f0a030";

/// An amount's slider track: from, to.
fn track(a: Amount) -> (f64, f64) {
    AMOUNTS
        .iter()
        .find(|t| t.0 == a)
        .map_or((0.0, 1.0), |t| (t.3, t.4))
}

/// What is at (`x`, `y`) on the strip (panel units across, strip units down).
pub fn hit(x: f64, y: f64) -> Option<StripTarget> {
    // (Left of `BAR_END` the row is the presets' selector's: `crate::presets::bar_hit`.)
    if !(0.0..STRIP_H).contains(&y) || x < BAR_END {
        return None;
    }
    let near = |y0: f64| (y - MID).abs() <= y0;
    if near(SWITCH_H / 2.0 + 6.0) && (POLY_X..POLY_X + SWITCH_W).contains(&x) {
        return Some(StripTarget::Poly);
    }
    let vx = VOICES_X + 150.0;
    if near(SWITCH_H / 2.0 + 6.0) && (vx..vx + STEP_W).contains(&x) {
        return Some(StripTarget::Fewer);
    }
    if near(SWITCH_H / 2.0 + 6.0) && (vx + STEP_W + 70.0..vx + 2.0 * STEP_W + 70.0).contains(&x) {
        return Some(StripTarget::More);
    }
    for (a, _, _, t0, t1, _, sw) in AMOUNTS {
        if near(THUMB + 8.0) && (t0 - THUMB..t1 + THUMB).contains(&x) {
            return Some(StripTarget::Slider(a));
        }
        if near(SWITCH_H / 2.0 + 6.0) && (sw..sw + SWITCH_W).contains(&x) {
            return Some(StripTarget::Switch(a));
        }
    }
    None
}

/// An amount's value (0..1) for a pointer at `x` along its slider.
pub fn slider_value(a: Amount, x: f64) -> f64 {
    let (t0, t1) = track(a);
    ((x - t0) / (t1 - t0)).clamp(0.0, 1.0)
}

/// The strip's SVG body, in panel units.
fn body(fonts: &Fonts, s: &StripScene) -> String {
    let mut out = Svg::default();
    put!(
        out,
        "<rect x='0' y='0' width='{}' height='{}' fill='{}'/>",
        N(W),
        N(STRIP_H),
        colour::PANEL
    );
    put!(
        out,
        "<line x1='0' y1='1' x2='{}' y2='1' stroke='{}' stroke-width='2'/>",
        N(W),
        colour::WOOD_DARK
    );
    let text = |out: &mut Svg, x: f64, t: &str, fill: &str, anchor: &str| {
        put!(
            out,
            "<text x='{}' y='{}' font-size='{}' fill='{fill}' text-anchor='{anchor}' letter-spacing='1'>{}</text>",
            N(x),
            N(MID + TEXT * 0.36),
            N(TEXT),
            escape(t)
        );
    };
    let switch = |out: &mut Svg, x: f64, on: bool, hover: bool| {
        put!(
            out,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='8' fill='{RAISED}' stroke='{}' stroke-width='{}'/>",
            N(x),
            N(MID - SWITCH_H / 2.0),
            N(SWITCH_W),
            N(SWITCH_H),
            if on { ACCENT } else { BORDER },
            if hover { 4 } else { 2 }
        );
        put!(
            out,
            "<text x='{}' y='{}' font-size='{}' fill='{}' text-anchor='middle'>{}</text>",
            N(x + SWITCH_W / 2.0),
            N(MID + 22.0 * 0.36),
            N(22.0),
            if on { colour::LEGEND } else { DIM },
            if on { "ON" } else { "OFF" }
        );
    };
    // The presets' selector at the left.
    out.0.push_str(&bar_svg(fonts, &s.bar));
    // POLY.
    text(&mut out, POLY_X - 20.0, "POLY", colour::LEGEND, "end");
    switch(&mut out, POLY_X, s.poly, s.hover == Some(StripTarget::Poly));
    // VOICES: the count between its steps.
    text(&mut out, VOICES_X + 130.0, "VOICES", colour::LEGEND, "end");
    let vx = VOICES_X + 150.0;
    for (x, t, target) in [
        (vx, "\u{2212}", StripTarget::Fewer),
        (vx + STEP_W + 70.0, "+", StripTarget::More),
    ] {
        put!(
            out,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='6' fill='{RAISED}' stroke='{BORDER}' stroke-width='{}'/>",
            N(x),
            N(MID - SWITCH_H / 2.0),
            N(STEP_W),
            N(SWITCH_H),
            if s.hover == Some(target) { 4 } else { 2 }
        );
        put!(
            out,
            "<text x='{}' y='{}' font-size='30' fill='{}' text-anchor='middle'>{t}</text>",
            N(x + STEP_W / 2.0),
            N(MID + 30.0 * 0.36),
            if s.poly { colour::LEGEND } else { DIM }
        );
    }
    let count = s.voices.to_string();
    text(
        &mut out,
        vx + STEP_W + 35.0,
        &count,
        if s.poly { colour::LEGEND } else { DIM },
        "middle",
    );
    // ENTROPY and SPREAD. SPREAD places POLY's voices: with POLY off the one voice is in the
    // centre, so it is dimmed (still operable, to set before POLY goes on).
    for (a, label, lx, t0, t1, vx, sw) in AMOUNTS {
        let v = match a {
            Amount::Entropy => s.entropy,
            Amount::Spread => s.spread,
        };
        let muted = a == Amount::Spread && !s.poly;
        if muted {
            put!(out, "<g opacity='0.5'>");
        }
        text(&mut out, lx, label, colour::LEGEND, "start");
        put!(
            out,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='{BORDER}'/>",
            N(t0),
            N(MID - TRACK / 2.0),
            N(t1 - t0),
            N(TRACK),
            N(TRACK / 2.0)
        );
        let x = t0 + (t1 - t0) * v.clamp(0.0, 1.0);
        put!(
            out,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='{ACCENT}'/>",
            N(t0),
            N(MID - TRACK / 2.0),
            N(x - t0),
            N(TRACK),
            N(TRACK / 2.0)
        );
        put!(
            out,
            "<circle cx='{}' cy='{}' r='{}' fill='{}' stroke='{}' stroke-width='{}'/>",
            N(x),
            N(MID),
            N(THUMB),
            colour::CAP,
            colour::CAP_DARK,
            if s.hover == Some(StripTarget::Slider(a)) {
                4
            } else {
                2
            }
        );
        text(
            &mut out,
            vx,
            &format!("{} %", (v * 100.0).round()),
            if v > 0.0 { colour::LEGEND } else { DIM },
            "middle",
        );
        switch(
            &mut out,
            sw,
            v > 0.0,
            s.hover == Some(StripTarget::Switch(a)),
        );
        if muted {
            put!(out, "</g>");
        }
    }
    out.0
}

/// The strip's renderer at a scale (the panel's: pixels a panel unit).
pub struct StripRenderer {
    fonts: Fonts,
    options: usvg::Options<'static>,
    scale: f64,
    frame: Pixmap,
    shown: Option<StripScene>,
}

impl fmt::Debug for StripRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StripRenderer")
            .field("scale", &self.scale)
            .finish()
    }
}

/// The strip's size at `scale` pixels a panel unit.
pub fn strip_size(scale: f64) -> (u32, u32) {
    (
        (W * scale).ceil().max(1.0) as u32,
        (STRIP_H * scale).ceil().max(1.0) as u32,
    )
}

impl StripRenderer {
    pub fn new(scale: f64) -> Self {
        let fonts = Fonts::new();
        let options = usvg::Options {
            fontdb: fonts.database(),
            font_family: FAMILY.into(),
            ..usvg::Options::default()
        };
        let (w, h) = strip_size(scale);
        StripRenderer {
            fonts,
            options,
            scale,
            frame: Pixmap::new(w, h).expect("a strip of at least a pixel"),
            shown: None,
        }
    }

    pub fn rescale(&mut self, scale: f64) {
        if scale != self.scale {
            let (w, h) = strip_size(scale);
            self.scale = scale;
            self.frame = Pixmap::new(w, h).expect("a strip of at least a pixel");
            self.shown = None;
        }
    }

    /// The frame last drawn (premultiplied RGBA, opaque).
    pub fn frame(&self) -> &Pixmap {
        &self.frame
    }

    /// Draws `scene`: whether the frame changed.
    pub fn render(&mut self, scene: &StripScene) -> bool {
        if self.shown.as_ref() == Some(scene) {
            return false;
        }
        self.shown = Some(scene.clone());
        let (w, h) = (self.frame.width(), self.frame.height());
        let view = [
            0.0,
            0.0,
            f64::from(w) / self.scale,
            f64::from(h) / self.scale,
        ];
        let doc = art::document(view, w, h, &body(&self.fonts, scene));
        self.frame.fill(Color::from_rgba8(0x1d, 0x1b, 0x1a, 0xff));
        if let Ok(tree) = usvg::Tree::from_str(&doc, &self.options) {
            resvg::render(&tree, Transform::identity(), &mut self.frame.as_mut());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::Weight;

    /// Every control is found where it is drawn, and nothing between them.
    #[test]
    fn each_control_is_found_where_it_is_drawn() {
        assert_eq!(hit(POLY_X + 10.0, MID), Some(StripTarget::Poly));
        assert_eq!(hit(VOICES_X + 160.0, MID), Some(StripTarget::Fewer));
        assert_eq!(
            hit(VOICES_X + 150.0 + STEP_W + 80.0, MID),
            Some(StripTarget::More)
        );
        for (a, _, _, t0, t1, _, sw) in AMOUNTS {
            assert_eq!(hit((t0 + t1) / 2.0, MID), Some(StripTarget::Slider(a)));
            assert_eq!(hit(sw + 10.0, MID), Some(StripTarget::Switch(a)));
            assert_eq!(slider_value(a, t0 - 50.0), 0.0);
            assert_eq!(slider_value(a, t1 + 50.0), 1.0);
            assert!((slider_value(a, (t0 + t1) / 2.0) - 0.5).abs() < 1e-12);
        }
        assert_eq!(hit(BAR_END / 2.0, MID), None);
        assert_eq!(hit(POLY_X + 10.0, STRIP_H + 1.0), None);
    }

    /// The presets' selector, POLY, VOICES, ENTROPY and SPREAD in that order across the row,
    /// none running into the next, the last inside the panel's width (each label measured in
    /// the strip's face).
    #[test]
    fn the_rows_parts_do_not_overlap() {
        let f = Fonts::new();
        let w = |t: &str| f.advance(t, TEXT, Weight::Regular, 1.0);
        let poly_label = POLY_X - 20.0 - w("POLY");
        assert!(poly_label >= BAR_END + 40.0, "POLY at {poly_label}");
        let voices_label = VOICES_X + 130.0 - w("VOICES");
        assert!(
            voices_label >= POLY_X + SWITCH_W + 40.0,
            "VOICES at {voices_label}"
        );
        let plus_end = VOICES_X + 150.0 + STEP_W + 70.0 + STEP_W;
        let mut at = plus_end;
        for (_, label, lx, t0, t1, vx, sw) in AMOUNTS {
            assert!(lx >= at + 40.0, "{label} at {lx}, after {at}");
            assert!(lx + w(label) + 20.0 <= t0 - THUMB, "{label} into its track");
            assert!(
                t1 + THUMB <= vx - w("100 %") / 2.0,
                "{label}'s value into its track"
            );
            assert!(
                vx + w("100 %") / 2.0 + 10.0 <= sw,
                "{label}'s value into its switch"
            );
            at = sw + SWITCH_W;
        }
        assert!(at <= W, "SPREAD ends at {at}");
    }

    /// The strip draws, and draws again only when its scene changes.
    #[test]
    fn it_draws_when_its_scene_changes() {
        let mut r = StripRenderer::new(0.4);
        let mut s = StripScene::default();
        assert!(r.render(&s));
        assert!(!r.render(&s));
        s.poly = true;
        s.voices = 6;
        assert!(r.render(&s));
        s.bar.name = "Undertow Growl".into();
        assert!(r.render(&s), "the selector's preset changed");
        assert_eq!(r.frame().width(), strip_size(0.4).0);
    }

    /// SPREAD is drawn dimmed while POLY is off, ENTROPY as it is.
    #[test]
    fn spread_dims_while_poly_is_off() {
        let scale = 0.5;
        let frame = |poly: bool| {
            let mut r = StripRenderer::new(scale);
            r.render(&StripScene {
                poly,
                entropy: 0.5,
                spread: 0.5,
                ..StripScene::default()
            });
            r.frame().clone()
        };
        let (off, on) = (frame(false), frame(true));
        // The pixels that differ in each amount's span (its label to its switch).
        let differ = |a: Amount| {
            let &(_, _, lx, _, _, _, sw) = AMOUNTS.iter().find(|x| x.0 == a).expect("an amount");
            let (x0, x1) = ((lx * scale) as u32, ((sw + SWITCH_W) * scale) as u32);
            off.pixels()
                .iter()
                .zip(on.pixels())
                .enumerate()
                .filter(|&(i, (p, q))| {
                    let x = i as u32 % off.width();
                    (x0..x1).contains(&x) && p != q
                })
                .count()
        };
        assert!(differ(Amount::Spread) > 200, "SPREAD not dimmed");
        assert_eq!(differ(Amount::Entropy), 0, "ENTROPY changed with POLY");
    }
}
