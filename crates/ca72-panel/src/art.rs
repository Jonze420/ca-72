//! The panel's art, as SVG, in panel units: the panel measured from a photograph of the
//! instrument's taken face on (Commons, "Minimoog panel.jpg"; the panel is 3108 by 795 of
//! them), under a wooden top strip, the name board below it, and the left hand controller
//! in a column of its own at the left.
//!
//! What never moves is drawn once ([`background`]); each control, lamp and the name plate
//! is a layer of its own, drawn over it ([`Layer`]).

use crate::controls::{CONTROLS, Colour, Control, Dial, Kind, Legends, Mark, Orient, Place};
use crate::fonts::{FAMILY, Fonts, Weight};
use crate::svg::{N, Svg, colour::*, defs, escape, put};

pub const PW: f64 = 3108.0;
pub const PH: f64 = 795.0;
pub const TOP: f64 = 130.0;
pub const BOARD: f64 = 132.0;
/// The column the left hand controller stands in, beside the panel.
pub const COL: f64 = 330.0;
/// The drawing's size.
pub const W: f64 = COL + PW;
pub const H: f64 = TOP + PH + BOARD;

/// Lettering sizes: the legends, the dials' numbers, the section titles.
const LEG: f64 = 19.0;
const NUM: f64 = 15.0;
/// The section titles' capitals are printed 38.5 units tall (the typeface's are 0.74 em).
const TITLE: f64 = 52.0;
/// The baseline of the section titles along the panel's foot.
const TITLE_Y: f64 = 727.0;
/// The panel's lettering is about 9 % wider than the typeface at the same capital height:
/// its legends are drawn stretched so (the section titles, set to their printed widths, and
/// the name plate are not).
const STRETCH: f64 = 1.09;

/// The name plate's name, the instrument's. Its maker's name is under it.
pub const PLATE_NAME: &str = "CA-72";
const PLATE_MAKER: &str = "IDLE FOUNDRY";
/// The name's margin from the plate's left end.
const PLATE_PAD: f64 = 24.0;
/// The plate's width: as when a click turned it over to a second, wider name, gone since
/// (the owner, 2026-10-03), so the panel stays as approved.
pub const PLATE_W: f64 = 261.0;
/// The plate's top left corner: its left end at the MODIFIERS|OUTPUT line.
pub const PLATE_X: f64 = COL + 2582.0;
pub const PLATE_H: f64 = 0.62 * BOARD;
pub const PLATE_Y: f64 = TOP + PH + (BOARD - PLATE_H) / 2.0;
const NAME_Y: f64 = 31.0;
const MAKER_Y: f64 = 64.0;

/// The left hand controller's face: its left edge in the drawing, its size.
pub const LH_X: f64 = 22.0;
const LPW: f64 = COL - 44.0;
const LPH: f64 = PH;
const LH_JACK: f64 = 46.0;
const LH_LABEL: f64 = 118.0;
pub(crate) const LH_ROCKER: f64 = 214.0;
const LH_SCREW_DX: f64 = 50.0;
pub(crate) const LH_ROWS: [f64; 2] = [150.0, 230.0];
pub(crate) const LH_WHEELS: [f64; 2] = [82.0, 204.0];
pub(crate) const WHEEL_Y: f64 = 515.0;
/// The wheels are drawn larger in the column than the instrument's.
pub const WHEEL_SCALE: f64 = 1.75;
/// A wheel's slot.
const SLOT_W: f64 = 60.0;
const SLOT_H: f64 = 146.0;

/// The POWER switch (the plugin's bypass) and its lamp, on the panel.
pub const POWER: (f64, f64) = (3023.0, 580.0);
const LAMP_AT: (f64, f64) = (3023.0, 426.0);
/// The OVERLOAD lamp, on the panel.
const OVERLOAD_AT: (f64, f64) = (1764.0, 274.0);

/// The editor's resize grip, in the name board's bottom right corner (left, top, right,
/// bottom).
pub const GRIP: [f64; 4] = [W - 54.0, H - 54.0, W, H];

/// The waveform pictograms, drawn 24 by 14 and printed about 18 by 10 (`GLYPH`).
const GLYPH: f64 = 0.75;
const TRIANGLE: &str = "M -11 7 L 0 -7 L 11 7";
const SHARK: &str = "M -11 7 L 3 -7 L 3 0 L 11 7";
const REVERSE: &str = "M -10 7 V -7 L 10 7";
const SAWTOOTH: &str = "M -10 7 L 10 -7 V 7";
const SQUARE: &str = "M -12 7 V -7 H 0 V 7 H 12";
const WIDE: &str = "M -12 7 V -7 H -3 V 7 H 12";
const NARROW: &str = "M -12 7 V -7 H -7 V 7 H 12";
const RANGES: [&str; 6] = ["LO", "32'", "16'", "8'", "4'", "2'"];
const WAVES12: [&str; 6] = [TRIANGLE, SHARK, SAWTOOTH, SQUARE, WIDE, NARROW];
const WAVES3: [&str; 6] = [TRIANGLE, REVERSE, SAWTOOTH, SQUARE, WIDE, NARROW];

/// A six-position selector's positions (RANGE, WAVEFORM): their angles from twelve
/// o'clock, 33.5 degrees apart, as the photograph's ticks and legends (on a circle of 89
/// about the axis) place them.
pub const SIX: [f64; 6] = [-83.75, -50.25, -16.75, 16.75, 50.25, 83.75];
/// The selectors' printed ticks (inner and outer radius, 4 wide) and their legends'
/// radius; a legend clears its tick by `SIX_CLEAR` at least.
const SIX_TICK: (f64, f64) = (61.0, 70.0);
const SIX_LEGEND: f64 = 89.0;
const SIX_CLEAR: f64 = 6.5;

/// A knob's size: its black skirt, its aluminium cap, its pointer dot's radius from the
/// centre and its own.
#[derive(Clone, Copy, Debug)]
pub struct Size {
    pub skirt: f64,
    cap: f64,
    dot: f64,
    dot_r: f64,
}

pub const STD: Size = Size {
    skirt: 54.0,
    cap: 36.0,
    dot: 40.0,
    dot_r: 4.0,
};
pub const BIG: Size = Size {
    skirt: 65.0,
    cap: 49.0,
    dot: 54.0,
    dot_r: 5.0,
};

fn rad(deg: f64) -> f64 {
    deg.to_radians()
}

/// The point at `r` from the centre at `deg` clockwise from twelve o'clock.
pub fn polar(r: f64, deg: f64) -> (f64, f64) {
    (r * rad(deg).sin(), -r * rad(deg).cos())
}

/// The lettering's kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Legend,
    Title,
    Plate,
}

/// Text centred on (x, y) (`y` the middle of its capitals); `start`: begun at `x`.
#[allow(clippy::too_many_arguments)]
fn text(s: &mut Svg, x: f64, y: f64, t: &str, size: f64, style: Style, start: bool, extra: &str) {
    let k = if style == Style::Legend { STRETCH } else { 1.0 };
    let look = match style {
        Style::Legend => format!(
            "fill='{LEGEND}' font-weight='700' stroke='{LEGEND}' stroke-width='0.7' paint-order='stroke'"
        ),
        Style::Title => format!("fill='{LEGEND}' font-weight='700'"),
        Style::Plate => format!("fill='{PLATE_TEXT}' font-weight='700'"),
    };
    let anchor = if start { "start" } else { "middle" };
    put!(
        s,
        "<text x='0' y='{}' transform='translate({} {}) scale({} 1)' text-anchor='{anchor}' font-size='{}' {look} {extra}>{}</text>",
        N(size * 0.36),
        N(x),
        N(y),
        N(k),
        N(size),
        escape(t)
    );
}

fn legend(s: &mut Svg, x: f64, y: f64, t: &str) {
    text(s, x, y, t, LEG, Style::Legend, false, "");
}

fn lettered(s: &mut Svg, x: f64, y: f64, t: &str, size: f64) {
    text(s, x, y, t, size, Style::Legend, false, "");
}

fn line(s: &mut Svg, x1: f64, y1: f64, x2: f64, y2: f64, w: f64) {
    put!(
        s,
        "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{LEGEND}' stroke-width='{}' stroke-linecap='round'/>",
        N(x1),
        N(y1),
        N(x2),
        N(y2),
        N(w)
    );
}

fn shadow(s: &mut Svg, cx: f64, cy: f64, r: f64) {
    put!(
        s,
        "<circle cx='{}' cy='{}' r='{}' fill='{SHADOW}' fill-opacity='{SHADOW_OPACITY}'/>",
        N(cx),
        N(cy),
        N(r)
    );
}

/// A small dark screw head with its slot.
fn screw(s: &mut Svg, x: f64, y: f64, r: f64) {
    put!(
        s,
        "<circle cx='{}' cy='{}' r='{}' fill='{SCREW}'/>",
        N(x),
        N(y),
        N(r)
    );
    put!(
        s,
        "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{HOLE}' stroke-width='2'/>",
        N(x - r * 0.7),
        N(y - r * 0.2),
        N(x + r * 0.7),
        N(y + r * 0.2)
    );
}

/// A dial's printed marks: ticks (angle, inner and outer radius, width) and numbers (angle,
/// text, and a radius where the number's printed place is not the dial's), and captions.
struct DialMarks {
    ticks: Vec<(f64, f64, f64, f64)>,
    labels: Vec<(f64, &'static str, Option<f64>)>,
    r: f64,
    captions: Vec<(f64, f64, &'static str)>,
}

fn eleven(r0: f64, r1: f64) -> Vec<(f64, f64, f64, f64)> {
    (0..11)
        .map(|k| (-150.0 + 30.0 * f64::from(k), r0, r1, 4.0))
        .collect()
}

fn marks(dial: Dial) -> DialMarks {
    let ten = || DialMarks {
        ticks: eleven(58.0, 71.0),
        labels: ["0", "2", "4", "6", "8", "10"]
            .iter()
            .enumerate()
            .map(|(i, s)| (-150.0 + 60.0 * i as f64, *s, None))
            .collect(),
        r: 86.0,
        captions: vec![],
    };
    match dial {
        Dial::Ten => ten(),
        Dial::ModMix => DialMarks {
            captions: vec![(-50.0, 100.0, "OSC. 3"), (49.0, 100.0, "NOISE")],
            ..ten()
        },
        Dial::Tune => DialMarks {
            ticks: eleven(58.0, 71.0),
            labels: vec![
                (-120.0, "–2", None),
                (-60.0, "–1", None),
                (0.0, "0", None),
                (60.0, "1", None),
                (120.0, "2", None),
            ],
            r: 86.0,
            captions: vec![],
        },
        Dial::Cutoff => DialMarks {
            ticks: eleven(58.0, 71.0),
            labels: vec![
                (-120.0, "–4", None),
                (-60.0, "–2", None),
                (0.0, "0", None),
                (60.0, "2", None),
                (120.0, "4", None),
            ],
            r: 86.0,
            captions: vec![],
        },
        // A tick a semitone, 20 degrees apart, from –7 to +7 (the odd ones numbered, 0
        // long), and an end mark past each end.
        Dial::Freq => DialMarks {
            ticks: (0..15)
                .map(|k| {
                    if k == 7 {
                        (0.0, 71.0, 93.0, 7.0)
                    } else {
                        (f64::from(k - 7) * 20.0, 72.0, 85.0, 4.0)
                    }
                })
                .chain([(-156.0, 72.0, 85.0, 4.0), (156.0, 72.0, 85.0, 4.0)])
                .collect(),
            labels: [
                (-7, "–7"),
                (-5, "–5"),
                (-3, "–3"),
                (-1, "–1"),
                (1, "1"),
                (3, "3"),
                (5, "5"),
                (7, "7"),
            ]
            .iter()
            .map(|(n, s)| (f64::from(*n) * 20.0, *s, None))
            .collect(),
            r: 98.0,
            captions: vec![],
        },
        // Marked in time, not evenly (as printed); the 200 ms mark is printed long ("200—").
        Dial::Time => DialMarks {
            ticks: [
                -150.0, -120.0, -105.0, -90.0, -60.0, -30.0, 0.0, 30.0, 49.0, 67.0, 86.0, 108.0,
                148.0,
            ]
            .iter()
            .map(|&a| (a, 58.0, if a == -90.0 { 78.0 } else { 71.0 }, 4.0))
            .collect(),
            labels: vec![
                (-120.0, "10", None),
                (-90.0, "200", None),
                (-30.0, "600", Some(80.0)),
                (30.0, "1", Some(80.0)),
                (67.0, "5", None),
                (108.0, "10", None),
            ],
            r: 86.0,
            captions: vec![(-61.0, 75.0, "M-SEC."), (50.0, 75.0, "SEC.")],
        },
    }
}

fn draw_dial(s: &mut Svg, cx: f64, cy: f64, dial: Dial) {
    let d = marks(dial);
    put!(s, "<g transform='translate({} {})'>", N(cx), N(cy));
    for (a, r0, r1, w) in d.ticks {
        let (x0, y0) = polar(r0, a);
        let (x1, y1) = polar(r1, a);
        line(s, x0, y0, x1, y1, w);
    }
    for (a, t, at) in d.labels {
        // Wider numbers sit a little further out at the sides.
        let r = at.unwrap_or(
            d.r + 0.3 * NUM * STRETCH * (t.chars().count() as f64 - 1.0) * rad(a).sin().abs(),
        );
        let (x, y) = polar(r, a);
        lettered(s, x, y, t, NUM);
    }
    for (x, y, t) in d.captions {
        lettered(s, x, y, t, NUM - 1.0);
    }
    s.0.push_str("</g>");
}

fn legends(set: Legends) -> (&'static [&'static str; 6], bool) {
    match set {
        Legends::Ranges => (&RANGES, false),
        Legends::Waves12 => (&WAVES12, true),
        Legends::Waves3 => (&WAVES3, true),
    }
}

fn set_index(set: Legends) -> usize {
    match set {
        Legends::Ranges => 0,
        Legends::Waves12 => 1,
        Legends::Waves3 => 2,
    }
}

/// What the lettering's measures decide: where each selector legend sits, and which titles
/// are spaced out to their printed widths.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Each legend's radius about its selector's axis, by legend set and position.
    legend_r: [[f64; 6]; 3],
    /// Each title's width if spaced out to its printed one.
    titles: [Option<f64>; 5],
}

/// The section titles: their printed extent along the foot, text and baseline.
const TITLES: [(f64, f64, &str, f64); 5] = [
    (45.0, 406.0, "CONTROLLERS", TITLE_Y),
    (532.0, 1028.0, "OSCILLATOR BANK", TITLE_Y),
    (1442.0, 1591.0, "MIXER", 724.0),
    (2111.0, 2382.0, "MODIFIERS", 724.0),
    (2670.0, 2868.0, "OUTPUT", 723.0),
];

/// The extent of a pictogram's path (its commands are absolute M, L, H and V).
fn path_box(d: &str) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let (mut x, mut y) = (0.0, 0.0);
    let mut words = d.split_whitespace();
    while let Some(cmd) = words.next() {
        let mut num = || {
            words
                .next()
                .and_then(|w| w.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        match cmd {
            "M" | "L" => {
                x = num();
                y = num();
            }
            "H" => x = num(),
            "V" => y = num(),
            _ => continue,
        }
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    b
}

impl Layout {
    /// Measures the lettering, as the mock-up did once the page's fonts had loaded.
    pub fn measure(fonts: &Fonts) -> Self {
        let (ascent, descent) = fonts.extent(Weight::Bold);
        let mut legend_r = [[SIX_LEGEND; 6]; 3];
        for set in [Legends::Ranges, Legends::Waves12, Legends::Waves3] {
            let (texts, glyphs) = legends(set);
            for (i, &a) in SIX.iter().enumerate() {
                // The ink's box about the legend's centre (through its own scale), with its
                // stroke: a text's from its start to its advance, ascent to descent.
                let b = if glyphs {
                    let p = path_box(texts[i]);
                    [
                        p[0] * GLYPH - 1.5,
                        p[1] * GLYPH - 1.5,
                        p[2] * GLYPH + 1.5,
                        p[3] * GLYPH + 1.5,
                    ]
                } else {
                    let adv = fonts.advance(texts[i], NUM, Weight::Bold, 0.0);
                    let y0 = NUM * 0.36 - ascent * NUM;
                    [
                        -adv / 2.0 * STRETCH - 0.35,
                        y0 - 0.35,
                        adv / 2.0 * STRETCH + 0.35,
                        y0 + (ascent + descent) * NUM + 0.35,
                    ]
                };
                let (x1, y1) = polar(SIX_TICK.0, a);
                let (x2, y2) = polar(SIX_TICK.1, a);
                let clear = |(cx, cy): (f64, f64)| {
                    let mut least = f64::INFINITY;
                    for k in 0..=50 {
                        let t = f64::from(k) * 0.02;
                        let x = x1 + (x2 - x1) * t - cx;
                        let y = y1 + (y2 - y1) * t - cy;
                        let dx = (b[0] - x).max(0.0).max(x - b[2]);
                        let dy = (b[1] - y).max(0.0).max(y - b[3]);
                        least = least.min(dx.hypot(dy) - 2.0);
                    }
                    least
                };
                let mut r = SIX_LEGEND;
                while clear(polar(r, a)) < SIX_CLEAR && r < SIX_LEGEND + 20.0 {
                    r += 0.5;
                }
                legend_r[set_index(set)][i] = r;
            }
        }
        // A title spans its printed width, spaced out to it, never squeezed: one wider by
        // nature is set at its own width, centred.
        let titles = TITLES.map(|(x0, x1, t, _)| {
            (fonts.advance(t, TITLE, Weight::Bold, 0.0) <= x1 - x0).then_some(x1 - x0)
        });
        Self { legend_r, titles }
    }

    /// Where a selector's legend `i` is, about its axis.
    pub fn legend_at(&self, set: Legends, i: usize) -> (f64, f64) {
        let (x, y) = polar(self.legend_r[set_index(set)][i], SIX[i]);
        ((x * 100.0).round() / 100.0, (y * 100.0).round() / 100.0)
    }
}

/// An SVG document showing `view` (x, y, width, height, in panel units) at `px` by `py`
/// pixels.
pub fn document(view: [f64; 4], px: u32, py: u32, body: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{} {} {} {}' width='{px}' height='{py}' font-family='{FAMILY}'>{}{body}</svg>",
        N(view[0]),
        N(view[1]),
        N(view[2]),
        N(view[3]),
        defs()
    )
}

/// Everything that does not move: the wood, the panel's face and lettering, the dials'
/// marks, and the parts with no parameter (PHONES, the controller's jacks), dimmed.
pub fn background(layout: &Layout) -> String {
    let mut s = Svg::default();
    wood(&mut s);
    panel(&mut s, layout);
    column(&mut s);
    s.0
}

fn wood(s: &mut Svg) {
    // The top strip, the name board and the controller's column. No cheeks: the ends are
    // square.
    put!(
        s,
        "<rect x='0' y='0' width='{}' height='{}' fill='url(#wood-h)'/>",
        N(W),
        N(TOP)
    );
    put!(
        s,
        "<rect x='0' y='{}' width='{}' height='{}' fill='url(#wood-h)'/>",
        N(TOP + PH),
        N(W),
        N(BOARD)
    );
    put!(
        s,
        "<rect x='0' y='{}' width='{}' height='{}' fill='url(#wood-v)'/>",
        N(TOP - 2.0),
        N(COL),
        N(PH + 4.0)
    );
    // Grain: long, gently wavering lines (the same every time).
    let mut seed: u64 = 7;
    let mut rnd = || {
        seed = seed * 16807 % 2_147_483_647;
        seed as f64 / 2_147_483_647.0
    };
    let mut grain = |s: &mut Svg, x0: f64, y0: f64, w: f64, h: f64, across: bool, n: u32| {
        for i in 0..n {
            let t = (f64::from(i) + 0.2 + rnd() * 0.6) / f64::from(n);
            let mut pts = Vec::with_capacity(9);
            for k in 0..=8 {
                let u = f64::from(k) / 8.0;
                let wob = (rnd() - 0.5) * if across { h } else { w } * 0.05;
                pts.push(if across {
                    format!("{:.1},{:.1}", x0 + u * w, y0 + t * h + wob)
                } else {
                    format!("{:.1},{:.1}", x0 + t * w + wob, y0 + u * h)
                });
            }
            put!(
                s,
                "<polyline points='{}' fill='none' stroke='{WOOD_DARK}' stroke-width='{}' stroke-opacity='0.3'/>",
                pts.join(" "),
                N(1.5 + rnd() * 2.5)
            );
        }
    };
    grain(s, 0.0, 6.0, W, TOP - 12.0, true, 7);
    grain(s, 0.0, TOP + PH + 8.0, W, BOARD - 16.0, true, 8);
    grain(s, 4.0, TOP, COL - 8.0, PH, false, 6);
    for x in [0.05, 0.35, 0.65, 0.95] {
        screw(s, COL + x * PW, TOP - 22.0, 7.0);
    }
    for k in 1..=3 {
        let d = 12.0 * f64::from(k);
        put!(
            s,
            "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{LEGEND}' stroke-opacity='0.3' stroke-width='3' stroke-linecap='round'/>",
            N(W - 10.0 - d),
            N(H - 10.0),
            N(W - 10.0),
            N(H - 10.0 - d)
        );
    }
}

fn panel(s: &mut Svg, layout: &Layout) {
    put!(s, "<g transform='translate({} {})'>", N(COL), N(TOP));
    put!(
        s,
        "<rect x='0' y='0' width='{}' height='{}' fill='{PANEL}'/>",
        N(PW),
        N(PH)
    );
    put!(
        s,
        "<rect x='0' y='0' width='{}' height='7' fill='{SHADOW}' fill-opacity='{SHADOW_OPACITY}'/>",
        N(PW)
    );
    put!(
        s,
        "<rect x='0' y='0' width='15' height='{}' fill='url(#trim)'/>",
        N(PH)
    );
    put!(
        s,
        "<rect x='{}' y='0' width='28' height='{}' fill='url(#trim)'/>",
        N(PW - 28.0),
        N(PH)
    );
    put!(
        s,
        "<rect x='0' y='{}' width='{}' height='6' fill='url(#trim)'/>",
        N(PH - 6.0),
        N(PW)
    );
    // Section dividers (broken where a label or a switch sits across one).
    let divider = |s: &mut Svg, x: f64, gaps: &[(f64, f64)]| {
        let mut y = 9.0;
        for &(a, b) in gaps.iter().chain([(PH - 7.0, PH)].iter()) {
            line(s, x, y, x, a, 4.0);
            y = b;
        }
    };
    divider(s, 445.0, &[(198.0, 297.0)]);
    divider(s, 1122.0, &[]);
    divider(s, 1908.0, &[(98.0, 197.0), (234.0, 292.0), (298.0, 392.0)]);
    divider(s, 2578.0, &[]);
    divider(s, 2956.0, &[]);
    line(s, 1908.0, 447.0, 2578.0, 447.0, 4.0);
    // Section titles along the foot, as wide as printed.
    for (i, (x0, x1, t, y)) in TITLES.iter().enumerate() {
        let spaced = layout.titles[i].map_or(String::new(), |w| {
            format!("textLength='{}' lengthAdjust='spacing'", N(w))
        });
        text(
            s,
            (x0 + x1) / 2.0,
            y - TITLE * 0.36,
            t,
            TITLE,
            Style::Title,
            false,
            &spaced,
        );
    }
    // The dials' marks and the selectors' ticks and legends.
    for c in CONTROLS.iter().filter(|c| c.place == Place::Panel) {
        match c.kind {
            Kind::Knob { dial, .. } => draw_dial(s, c.x, c.y, dial),
            Kind::Selector(set) => selector_marks(s, c, set, layout),
            _ => {}
        }
    }
    // CONTROLLERS.
    legend(s, 229.0, 162.0, "TUNE");
    legend(s, 446.0, 212.0, "OSCILLATOR");
    legend(s, 446.0, 232.0, "MODULATION");
    legend(s, 492.0, 313.0, "ON");
    legend(s, 128.0, 381.0, "GLIDE");
    legend(s, 332.0, 381.0, "MODULATION MIX");
    // OSCILLATOR BANK.
    legend(s, 618.0, 57.0, "RANGE");
    legend(s, 821.0, 57.0, "OSCILLATOR –1");
    legend(s, 821.0, 81.0, "FREQUENCY");
    legend(s, 1022.0, 57.0, "WAVEFORM");
    legend(s, 822.0, 260.0, "OSCILLATOR –2");
    legend(s, 822.0, 471.0, "OSCILLATOR –3");
    // Printed smaller than its neighbours.
    lettered(s, 488.0, 490.0, "OSC. 3", 16.0);
    lettered(s, 488.0, 509.0, "CONTROL", 16.0);
    // MIXER: five rows, the oscillators' VOLUME joined to their rockers by a line.
    legend(s, 1221.0, 59.0, "VOLUME");
    for (i, y) in [170.0, 274.0, 377.0, 481.0, 583.0].into_iter().enumerate() {
        if i % 2 == 0 {
            line(s, 1316.0, y, 1361.0, y, 4.0);
        } else {
            line(s, 1477.0, y, 1522.0, y, 4.0);
        }
        legend(s, 1467.0, y + 40.0, "ON");
    }
    legend(s, 1620.0, 144.0, "EXTERNAL");
    legend(s, 1622.0, 164.0, "INPUT VOLUME");
    legend(s, 1622.0, 385.0, "NOISE VOLUME");
    legend(s, 1763.0, 226.0, "OVERLOAD");
    legend(s, 1764.0, 407.0, "WHITE");
    legend(s, 1764.0, 558.0, "PINK");
    legend(s, 1910.0, 114.0, "FILTER");
    legend(s, 1910.0, 134.0, "MODULATION");
    legend(s, 1954.0, 207.0, "ON");
    legend(s, 1954.0, 228.0, "ON");
    legend(s, 1837.0, 263.0, "1");
    legend(s, 1910.0, 309.0, "KEYBOARD");
    legend(s, 1910.0, 326.0, "CONTROL");
    legend(s, 1837.0, 363.0, "2");
    legend(s, 1954.0, 401.0, "ON");
    // MODIFIERS.
    lettered(s, 2282.0, 31.0, "FILTER", 31.0);
    legend(s, 2081.0, 55.0, "CUTOFF FREQUENCY");
    legend(s, 2280.0, 55.0, "EMPHASIS");
    legend(s, 2482.0, 32.0, "AMOUNT");
    legend(s, 2482.0, 54.0, "OF CONTOUR");
    for ly in [266.0, 486.0] {
        legend(s, 2081.0, ly, "ATTACK TIME");
        legend(s, 2281.0, ly, "DECAY TIME");
        legend(s, 2483.0, ly, "SUSTAIN LEVEL");
    }
    lettered(s, 2284.0, 462.0, "LOUDNESS CONTOUR", 27.0);
    // OUTPUT.
    legend(s, 2681.0, 51.0, "VOLUME");
    legend(s, 2854.0, 51.0, "MAIN OUTPUT");
    legend(s, 2902.0, 200.0, "ON");
    legend(s, 2682.0, 336.0, "A–440");
    legend(s, 2728.0, 410.0, "ON");
    // The phones' VOLUME knob is FEEDBACK (a control: decisions.md R8).
    legend(s, 2682.0, 485.0, "FEEDBACK");
    legend(s, 2856.0, 485.0, "PHONES");
    // The PHONES jack has no parameter: drawn for the panel's sake, dimmed.
    put!(s, "<g transform='translate(2855 581)' opacity='0.5'>");
    jack(s, 34.0, false);
    s.0.push_str("</g>");
    // POWER: the plugin's bypass. The lamp is lit while it plays.
    legend(s, 3023.0, 465.0, "POWER");
    legend(s, 3023.0, 500.0, "ON");
    s.0.push_str("</g>");
}

fn selector_marks(s: &mut Svg, c: &Control, set: Legends, layout: &Layout) {
    put!(s, "<g transform='translate({} {})'>", N(c.x), N(c.y));
    for a in SIX {
        let (x0, y0) = polar(SIX_TICK.0, a);
        let (x1, y1) = polar(SIX_TICK.1, a);
        put!(
            s,
            "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{LEGEND}' stroke-width='4'/>",
            N(x0),
            N(y0),
            N(x1),
            N(y1)
        );
    }
    let (texts, glyphs) = legends(set);
    for (i, t) in texts.iter().enumerate() {
        let (x, y) = layout.legend_at(set, i);
        put!(s, "<g transform='translate({} {})'>", N(x), N(y));
        if glyphs {
            put!(
                s,
                "<path d='{t}' transform='scale({GLYPH})' fill='none' stroke='{LEGEND}' stroke-width='{}' stroke-linecap='round' stroke-linejoin='round'/>",
                N(3.0 / GLYPH)
            );
        } else {
            lettered(s, 0.0, 0.0, t, NUM);
        }
        s.0.push_str("</g>");
    }
    s.0.push_str("</g>");
}

/// A jack's chrome ring; `nut`: with the hex nut that holds it.
fn jack(s: &mut Svg, r: f64, nut: bool) {
    if nut {
        let pts: Vec<String> = (0..6)
            .map(|k| {
                let (x, y) = polar(r + 6.0, 30.0 + 60.0 * f64::from(k));
                format!("{x:.2},{y:.2}")
            })
            .collect();
        put!(
            s,
            "<polygon points='{}' fill='url(#cap)' stroke='{CAP_DARK}' stroke-width='1.5'/>",
            pts.join(" ")
        );
    }
    shadow(s, 3.0, 4.0, r + 1.0);
    put!(
        s,
        "<circle r='{}' fill='url(#cap)' stroke='{CAP_DARK}' stroke-width='1.5'/>",
        N(r)
    );
    put!(
        s,
        "<circle r='{}' fill='none' stroke='{CAP_DARK}' stroke-width='3'/>",
        N(r * 0.66)
    );
    put!(s, "<circle r='{}' fill='{HOLE}'/>", N(r * 0.36));
}

/// The left hand controller: GLIDE and DECAY, then the PITCH and MODULATION wheels, upright
/// in its column, as tall as the panel.
fn column(s: &mut Svg) {
    put!(s, "<g transform='translate({} {})'>", N(LH_X), N(TOP));
    put!(
        s,
        "<rect x='0' y='0' width='{}' height='{}' rx='6' fill='{PANEL}'/>",
        N(LPW),
        N(LPH)
    );
    put!(
        s,
        "<rect x='0' y='0' width='{}' height='7' fill='{SHADOW}' fill-opacity='{SHADOW_OPACITY}'/>",
        N(LPW)
    );
    for (x, y) in [
        (16.0, 16.0),
        (LPW - 16.0, 16.0),
        (16.0, LPH - 16.0),
        (LPW - 16.0, LPH - 16.0),
    ] {
        screw(s, x, y, 8.0);
    }
    // The GLIDE and DECAY jacks have no parameter: drawn, dimmed.
    for y in LH_ROWS {
        put!(
            s,
            "<g transform='translate({} {})' opacity='0.5'>",
            N(LH_JACK),
            N(y)
        );
        jack(s, 22.0, true);
        s.0.push_str("</g>");
    }
    // The switches' and the wheels' mounting screws.
    for y in LH_ROWS {
        for x in [LH_ROCKER - LH_SCREW_DX, LH_ROCKER + LH_SCREW_DX] {
            screw(s, x, y, 5.0);
        }
    }
    for x in LH_WHEELS {
        for dy in [-64.0, 64.0] {
            screw(
                s,
                x - (30.0 * WHEEL_SCALE + 10.0),
                WHEEL_Y + dy * WHEEL_SCALE,
                5.0,
            );
        }
    }
    legend(s, LH_LABEL, LH_ROWS[0], "GLIDE");
    legend(s, LH_LABEL, LH_ROWS[1], "DECAY");
    lettered(s, LH_ROCKER + 36.0, LH_ROWS[0] + 34.0, "ON", NUM);
    lettered(s, LH_ROCKER + 36.0, LH_ROWS[1] + 34.0, "ON", NUM);
    let label_y = WHEEL_Y + 73.0 * WHEEL_SCALE + 16.0;
    legend(s, LH_WHEELS[0], label_y, "PITCH");
    legend(s, LH_WHEELS[1], label_y, "MOD.");
    s.0.push_str("</g>");
}

// ---- The parts that move.

/// The fluted black body seen from above: nine broad, shallow, smooth flutes.
fn fluted(s: &mut Svg, r: f64) {
    let lobes = 9.0;
    let n = 9 * 16;
    s.0.push_str("<path d='");
    for k in 0..n {
        let a = f64::from(k) / f64::from(n) * 360.0;
        let (x, y) = polar(r - 3.5 * (1.0 - rad(a * lobes).cos()), a);
        put!(s, "{} {x:.2} {y:.2} ", if k == 0 { "M" } else { "L" });
    }
    put!(
        s,
        "Z' fill='url(#skirt)' stroke='{KNOB_RIM}' stroke-opacity='0.6' stroke-width='1.2'/>"
    );
}

/// The light on a black knob's shoulder (between its cap at `r0` and its edge at `r1`): one
/// soft highlight at the upper left, where the light is (it does not turn).
fn shine(s: &mut Svg, r0: f64, r1: f64) {
    let (x, y) = polar((r0 + r1) / 2.0, -45.0);
    put!(
        s,
        "<ellipse cx='{}' cy='{}' rx='{}' ry='{}' transform='rotate(-45 {} {})' fill='url(#gloss)'/>",
        N(x),
        N(y),
        N((r1 - r0) * 0.75),
        N((r1 - r0) * 0.38),
        N(x),
        N(y)
    );
}

/// A spun aluminium cap: its two faint wedges.
fn aluminium(s: &mut Svg, r: f64) {
    put!(
        s,
        "<circle r='{}' fill='url(#cap-flat)' stroke='{CAP_DARK}' stroke-width='1.5'/>",
        N(r)
    );
    put!(s, "<circle r='{}' fill='url(#cap-shade)'/>", N(r));
    let (ax, ay) = polar(r, -40.0);
    let (bx, by) = polar(r, -20.0);
    let (cx, cy) = polar(r, 140.0);
    let (dx, dy) = polar(r, 160.0);
    let r = N(r);
    put!(
        s,
        "<path d='M 0 0 L {} {} A {r} {r} 0 0 1 {} {} Z M 0 0 L {} {} A {r} {r} 0 0 1 {} {} Z' fill='{CAP_LIGHT}' fill-opacity='0.16'/>",
        N(ax),
        N(ay),
        N(bx),
        N(by),
        N(cx),
        N(cy),
        N(dx),
        N(dy)
    );
}

/// A knob at the origin turned `deg` from twelve o'clock: glossy black, its base's
/// silhouette fluted, its pointer a white dot on the shoulder outside the cap.
fn knob_body(s: &mut Svg, size: Size, deg: f64) {
    shadow(s, 3.0, 5.0, size.skirt + 1.0);
    put!(s, "<g transform='rotate({deg:.2})'>");
    fluted(s, size.skirt);
    put!(
        s,
        "<circle cx='0' cy='{}' r='{}' fill='{LEGEND}'/>",
        N(-size.dot),
        N(size.dot_r)
    );
    s.0.push_str("</g>");
    shine(s, size.cap, size.skirt);
    aluminium(s, size.cap);
}

/// A pointer knob at position `i` of six: the fluted black body, a fin along its pointer
/// carrying a white line from the cap to its tip, a spun aluminium cap.
fn selector_body(s: &mut Svg, i: usize) {
    shadow(s, 3.0, 5.0, 57.0);
    put!(s, "<g transform='rotate({})'>", N(SIX[i]));
    fluted(s, 56.0);
    put!(
        s,
        "<path d='M -17 12 L -17 -20 L -8 -46 Q 0 -52 8 -46 L 17 -20 L 17 12 Q 0 22 -17 12 Z' transform='scale(1.15)' fill='url(#skirt)' stroke='{KNOB_RIM}' stroke-opacity='0.6' stroke-width='1'/>"
    );
    put!(
        s,
        "<line x1='0' y1='-28' x2='0' y2='-56' stroke='{LEGEND}' stroke-width='5' stroke-linecap='round'/>"
    );
    s.0.push_str("</g>");
    shine(s, 26.0, 56.0);
    aluminium(s, 26.0);
}

fn colour_name(c: Colour) -> &'static str {
    match c {
        Colour::Blue => "blue",
        Colour::Red => "red",
        Colour::Black => "black",
        Colour::Ivory => "ivory",
    }
}

/// A rocker's paddle, `w` by `h`, its "on" end at +x and pressed when `on`, else mirrored.
fn paddle(s: &mut Svg, w: f64, h: f64, colour: Colour, on: bool) {
    let c = colour_name(colour);
    put!(
        s,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='3' fill='{HOLE}'/>",
        N(-w / 2.0 - 1.5),
        N(-h / 2.0 - 1.5),
        N(w + 3.0),
        N(h + 3.0)
    );
    s.0.push_str(if on {
        "<g>"
    } else {
        "<g transform='scale(-1 1)'>"
    });
    put!(
        s,
        "<rect x='0' y='{}' width='{}' height='{}' rx='3' fill='url(#rk-{c}-p)'/>",
        N(-h / 2.0 + 3.0),
        N(w / 2.0),
        N(h - 3.0)
    );
    // The raised half, its outer end rounded over.
    let (top, bottom, end, r) = (-h / 2.0 - 3.0, h / 2.0, -w / 2.0, (h * 0.2).min(8.0));
    put!(
        s,
        "<path d='M 2 {} H {} Q {} {} {} {} V {} Q {} {} {} {} H 2 Z' fill='url(#rk-{c}-r)'/>",
        N(top),
        N(end + r),
        N(end),
        N(top),
        N(end),
        N(top + r),
        N(bottom - r),
        N(end),
        N(bottom),
        N(end + r),
        N(bottom)
    );
    s.0.push_str("</g>");
}

/// A wheel seen from above in its slot, at `shown` (0..1) of its travel: its ribs and mark
/// roll with it.
fn wheel(s: &mut Svg, mark: Mark, span: f64, shown: f64) {
    let sh = SLOT_H;
    put!(s, "<g transform='scale({WHEEL_SCALE})'>");
    put!(
        s,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='7' fill='{HOLE}'/>",
        N(-SLOT_W / 2.0),
        N(-sh / 2.0),
        N(SLOT_W),
        N(sh)
    );
    put!(
        s,
        "<rect x='-24' y='{}' width='48' height='{}' rx='10' fill='url(#ivory)'/>",
        N(-sh / 2.0 + 6.0),
        N(sh - 12.0)
    );
    // The wheel's turn: `span` degrees over its travel, centred.
    let turn = (shown - 0.5) * span;
    let rib = 8.0;
    let r = 84.0;
    let mut a = -64.0 + turn.rem_euclid(rib);
    while a <= 64.0 {
        let y = -r * rad(a).sin();
        if y.abs() <= sh / 2.0 - 9.0 {
            put!(
                s,
                "<line x1='-22' y1='{}' x2='22' y2='{}' stroke='{IVORY_DARK}' stroke-width='2.4' stroke-opacity='{:.2}'/>",
                N(y),
                N(y),
                rad(a).cos()
            );
        }
        a += rib;
    }
    let at = if mark == Mark::Line { turn } else { turn - 7.0 };
    let y = -r * rad(at).sin();
    if at.abs() < 60.0 && y.abs() < sh / 2.0 - 9.0 {
        match mark {
            Mark::Line => put!(
                s,
                "<line x1='-22' y1='{}' x2='22' y2='{}' stroke='{KNOB}' stroke-width='4'/>",
                N(y),
                N(y)
            ),
            Mark::Dot => put!(s, "<circle cx='0' cy='{}' r='5' fill='{KNOB}'/>", N(y)),
        }
    }
    put!(
        s,
        "<rect x='-24' y='{}' width='48' height='{}' rx='10' fill='url(#ivory-sides)'/>",
        N(-sh / 2.0 + 6.0),
        N(sh - 12.0)
    );
    s.0.push_str("</g>");
}

/// A part of the drawing that moves: its art about `origin` (in the drawing), within `bounds`
/// of it (left, top, right, bottom).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub origin: (f64, f64),
    pub bounds: [f64; 4],
    pub body: String,
}

/// A control's extent about its centre (left, top, right, bottom), its shadow included.
pub fn bounds(kind: &Kind) -> [f64; 4] {
    match *kind {
        Kind::Knob { big, .. } => {
            let r = if big { BIG.skirt } else { STD.skirt };
            [-(r + 3.0), -(r + 3.0), r + 6.0, r + 8.0]
        }
        Kind::Selector(_) => [-62.0, -62.0, 63.0, 64.0],
        Kind::Rocker { w, h, .. } => [-w / 2.0 - 4.0, -h / 2.0 - 4.0, w / 2.0 + 4.0, h / 2.0 + 4.0],
        Kind::Wheel { .. } => {
            let (hx, hy) = (
                SLOT_W / 2.0 * WHEEL_SCALE + 2.0,
                SLOT_H / 2.0 * WHEEL_SCALE + 2.0,
            );
            [-hx, -hy, hx, hy]
        }
    }
}

/// Where a wheel is drawn: its parameter `v`, moved on by a MIDI keyboard's wheel `midi`
/// (the bend adds, 0.5 centred; the modulation wheel goes as far as the greater).
pub fn wheel_shown(detent: bool, v: f64, midi: f64) -> f64 {
    if detent {
        (v + midi - 0.5).clamp(0.0, 1.0)
    } else {
        v.max(midi)
    }
}

/// A control at its normalized value `v` (a wheel moved on by `midi`).
pub fn control(c: &Control, v: f64, midi: f64) -> Layer {
    let mut s = Svg::default();
    let v = v.clamp(0.0, 1.0);
    match c.kind {
        Kind::Knob { big, .. } => {
            knob_body(&mut s, if big { BIG } else { STD }, -150.0 + 300.0 * v)
        }
        Kind::Selector(_) => selector_body(&mut s, position(v)),
        Kind::Rocker {
            w,
            h,
            colour,
            orient,
        } => {
            let (turn, pw, ph) = match orient {
                Orient::Right => ("", w, h),
                Orient::Top => ("rotate(-90)", h, w),
                Orient::Bottom => ("rotate(90)", h, w),
            };
            put!(s, "<g transform='{turn}'>");
            paddle(&mut s, pw, ph, colour, v >= 0.5);
            s.0.push_str("</g>");
        }
        Kind::Wheel { mark, detent, span } => {
            wheel(&mut s, mark, span, wheel_shown(detent, v, midi))
        }
    }
    Layer {
        origin: c.centre(),
        bounds: bounds(&c.kind),
        body: s.0,
    }
}

/// A selector's position (0..5) at its normalized value.
pub fn position(v: f64) -> usize {
    (v.clamp(0.0, 1.0) * 5.0).round() as usize
}

/// The POWER switch's extent about its centre.
pub const POWER_BOUNDS: [f64; 4] = [-26.0, -64.0, 26.0, 64.0];

/// The POWER switch, on (playing) or off (bypassed).
pub fn power(on: bool) -> Layer {
    let mut s = Svg::default();
    s.0.push_str("<g transform='rotate(-90)'>");
    paddle(&mut s, 119.0, 43.0, Colour::Black, on);
    s.0.push_str("</g>");
    Layer {
        origin: (COL + POWER.0, TOP + POWER.1),
        bounds: POWER_BOUNDS,
        body: s.0,
    }
}

/// The POWER lamp, lit while the plugin plays.
pub fn lamp(on: bool) -> Layer {
    let mut s = Svg::default();
    shadow(&mut s, 3.0, 4.0, 19.0);
    put!(
        s,
        "<circle r='18' fill='url(#cap)' stroke='{CAP_DARK}' stroke-width='1.5'/>"
    );
    put!(
        s,
        "<circle r='13' fill='{}'/>",
        if on { LAMP } else { LAMP_OFF }
    );
    put!(s, "<circle cx='-4' cy='-4' r='3.2' fill='{CAP_LIGHT}'/>");
    Layer {
        origin: (COL + LAMP_AT.0, TOP + LAMP_AT.1),
        bounds: [-20.0, -20.0, 23.0, 24.0],
        body: s.0,
    }
}

/// The OVERLOAD lamp at `level` (0 dark, 1 fully lit): a plain dark jewel, smooth, domed,
/// with a small highlight; lit, it glows red through.
pub fn overload(level: f64) -> Layer {
    let level = (level.clamp(0.0, 1.0) * 100.0).round() / 100.0;
    let mut s = Svg::default();
    shadow(&mut s, 3.0, 4.0, 23.0);
    put!(
        s,
        "<circle r='22' fill='url(#cap)' stroke='{CAP_DARK}' stroke-width='1.5'/>"
    );
    put!(s, "<circle r='15' fill='url(#jewel)'/>");
    if level >= 0.05 {
        put!(
            s,
            "<filter id='lit' x='-1' y='-1' width='3' height='3'><feDropShadow dx='0' dy='0' stdDeviation='3' flood-color='{LAMP}'/></filter>"
        );
        put!(
            s,
            "<circle r='15' fill='{LAMP}' fill-opacity='{}' filter='url(#lit)'/>",
            N(level)
        );
    }
    put!(
        s,
        "<circle cx='-5' cy='-5.5' r='2.8' fill='{CAP_LIGHT}' fill-opacity='0.75'/>"
    );
    Layer {
        origin: (COL + OVERLOAD_AT.0, TOP + OVERLOAD_AT.1),
        bounds: [-34.0, -34.0, 34.0, 34.0],
        body: s.0,
    }
}

/// The name plate: the instrument's name, its maker's under it.
pub fn plate() -> Layer {
    let (w, h, r) = (PLATE_W, PLATE_H, 3.0);
    let mut s = Svg::default();
    put!(
        s,
        "<path d='M {r} 0 H {} A {r} {r} 0 0 1 {} {r} V {} A {r} {r} 0 0 1 {} {} H {r} A {r} {r} 0 0 1 0 {} V {r} A {r} {r} 0 0 1 {r} 0 Z' fill='{KNOB}'/>",
        N(w - r),
        N(w),
        N(h - r),
        N(w - r),
        N(h),
        N(h - r)
    );
    text(
        &mut s,
        PLATE_PAD,
        NAME_Y,
        PLATE_NAME,
        38.0,
        Style::Plate,
        true,
        "",
    );
    text(
        &mut s,
        PLATE_PAD + 2.0,
        MAKER_Y,
        PLATE_MAKER,
        12.0,
        Style::Plate,
        true,
        "letter-spacing='4.2'",
    );
    Layer {
        origin: (PLATE_X, PLATE_Y),
        bounds: [-24.0, -10.0, w + 24.0, h + 10.0],
        body: s.0,
    }
}

/// A hover tip: `text` in a box of the tip's colours (`TIP`, `TIP_BORDER`), `size` its
/// lettering's height in panel units, centred above (x, y).
pub fn tip(fonts: &Fonts, x: f64, y: f64, t: &str, size: f64) -> Layer {
    let pad = size * 0.4;
    let w = fonts.advance(t, size, Weight::Regular, 0.0) + 2.0 * pad;
    let h = size * 1.5;
    let gap = size * 0.3;
    let (x0, y0) = (-w / 2.0, -h - gap);
    let mut s = Svg::default();
    put!(
        s,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='{TIP}' stroke='{TIP_BORDER}' stroke-width='{}'/>",
        N(x0),
        N(y0),
        N(w),
        N(h),
        N(size * 0.25),
        N(size / 12.0)
    );
    put!(
        s,
        "<text x='0' y='{}' text-anchor='middle' font-size='{}' fill='{TIP_TEXT}'>{}</text>",
        N(y0 + h / 2.0 + size * 0.36),
        N(size),
        escape(t)
    );
    let m = size / 6.0;
    Layer {
        origin: (x, y),
        bounds: [x0 - m, y0 - m, -x0 + m, m],
        body: s.0,
    }
}
