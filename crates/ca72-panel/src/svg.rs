//! Writing SVG: numbers, escaping, the panel's colours and its gradients.

use std::fmt::{self, Display, Write};

/// A number as SVG wants it: at most three decimals, no trailing zeros.
#[derive(Clone, Copy, Debug)]
pub struct N(pub f64);

impl Display for N {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = (self.0 * 1000.0).round() / 1000.0;
        if v == 0.0 {
            f.write_str("0")
        } else {
            write!(f, "{v}")
        }
    }
}

/// Text made safe for an element's content or an attribute: what XML escapes, escaped, and
/// what XML 1.0 forbids (control characters but tab and the line ends; U+FFFE, U+FFFF)
/// left out. One in a preset's name or tag made the parser refuse the whole document, and
/// the drawer or the strip drew blank (decisions.md R18).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            '\u{0}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {}
            _ => out.push(c),
        }
    }
    out
}

/// An SVG document being written.
#[derive(Debug, Default)]
pub struct Svg(pub String);

impl Svg {
    /// Writes formatted text.
    pub fn put(&mut self, args: fmt::Arguments<'_>) {
        // Writing to a String cannot fail.
        let _ = self.0.write_fmt(args);
    }
}

/// Writes to an [`Svg`] as `write!` does.
macro_rules! put {
    ($s:expr, $($arg:tt)*) => { $s.put(format_args!($($arg)*)) };
}
pub(crate) use put;

/// The panel's colours (as the DAW the model was developed in drew its panel).
pub mod colour {
    pub const PANEL: &str = "#1d1b1a";
    pub const LEGEND: &str = "#ebe6d8";
    pub const WOOD: &str = "#6b4226";
    pub const WOOD_LIGHT: &str = "#8d5c37";
    pub const WOOD_DARK: &str = "#3b2213";
    pub const TRIM: &str = "#b5b7b4";
    pub const KNOB: &str = "#0c0b0b";
    pub const SCREW: &str = "#2a2622";
    pub const PLATE_TEXT: &str = "#c8c8c2";
    pub const KNOB_RIM: &str = "#46423e";
    pub const CAP: &str = "#c4c5c1";
    pub const CAP_LIGHT: &str = "#f7f7f2";
    pub const CAP_DARK: &str = "#77786f";
    pub const BLUE: &str = "#3d7fb5";
    pub const BLUE_LIGHT: &str = "#74acd9";
    pub const BLUE_DARK: &str = "#285a84";
    pub const RED: &str = "#dc3918";
    pub const RED_LIGHT: &str = "#f86c45";
    pub const RED_DARK: &str = "#9c2510";
    pub const IVORY: &str = "#ded5bc";
    pub const IVORY_DARK: &str = "#9b927a";
    pub const HOLE: &str = "#050404";
    /// Shadows are black at half opacity.
    pub const SHADOW: &str = "#000000";
    pub const SHADOW_OPACITY: f64 = 0.5;
    pub const LAMP: &str = "#ec2b1c";
    pub const LAMP_OFF: &str = "#4c1914";
    pub const OVERLOAD: &str = "#58231a";
    /// The hover tip's (that DAW's panel and text colours).
    pub const TIP: &str = "#1f2228";
    pub const TIP_BORDER: &str = "#30343c";
    pub const TIP_TEXT: &str = "#d7dae0";
}

/// The gradients every document of the panel may use, by `url(#name)`.
pub fn defs() -> String {
    use colour::*;
    let mut s = Svg::default();
    s.0.push_str("<defs>");
    let mut grad = |name: &str, kind: &str, attrs: &str, stops: &[(f64, &str, Option<f64>)]| {
        put!(s, "<{kind} id='{name}' {attrs}>");
        for (offset, colour, opacity) in stops {
            match opacity {
                Some(o) => put!(
                    s,
                    "<stop offset='{}' stop-color='{colour}' stop-opacity='{}'/>",
                    N(*offset),
                    N(*o)
                ),
                None => put!(s, "<stop offset='{}' stop-color='{colour}'/>", N(*offset)),
            }
        }
        put!(s, "</{kind}>");
    };
    let diagonal = "x1='0' y1='0' x2='1' y2='1'";
    let across = "x1='0' y1='0' x2='1' y2='0'";
    let down = "x1='0' y1='0' x2='0' y2='1'";
    grad(
        "cap",
        "linearGradient",
        diagonal,
        &[
            (0.0, CAP_LIGHT, None),
            (0.45, CAP, None),
            (1.0, CAP_DARK, None),
        ],
    );
    // Glossy near-black, one soft highlight on the shoulder at the upper left.
    grad(
        "skirt",
        "radialGradient",
        "cx='0.5' cy='0.5' r='0.5' fx='0.3' fy='0.25'",
        &[
            (0.0, KNOB, None),
            (0.55, KNOB_RIM, None),
            (0.8, KNOB, None),
            (1.0, KNOB, None),
        ],
    );
    grad(
        "gloss",
        "radialGradient",
        "",
        &[
            (0.0, CAP_LIGHT, Some(0.13)),
            (0.5, CAP_LIGHT, Some(0.05)),
            (1.0, CAP_LIGHT, Some(0.0)),
        ],
    );
    // A knob's cap: flat brushed aluminium, shaded only lightly from the upper left.
    grad("cap-flat", "linearGradient", "", &[(0.0, CAP, None)]);
    grad(
        "cap-shade",
        "linearGradient",
        diagonal,
        &[
            (0.0, CAP_LIGHT, Some(0.3)),
            (0.5, CAP, Some(0.0)),
            (1.0, CAP_DARK, Some(0.35)),
        ],
    );
    grad(
        "jewel",
        "radialGradient",
        "fx='0.38' fy='0.34'",
        &[
            (0.0, OVERLOAD, None),
            (0.7, OVERLOAD, None),
            (1.0, HOLE, None),
        ],
    );
    grad(
        "wood-v",
        "linearGradient",
        across,
        &[
            (0.0, WOOD_DARK, None),
            (0.25, WOOD_LIGHT, None),
            (0.7, WOOD, None),
            (1.0, WOOD_DARK, None),
        ],
    );
    grad(
        "wood-h",
        "linearGradient",
        down,
        &[
            (0.0, WOOD_LIGHT, None),
            (0.55, WOOD, None),
            (1.0, WOOD_DARK, None),
        ],
    );
    grad(
        "trim",
        "linearGradient",
        across,
        &[
            (0.0, CAP_DARK, None),
            (0.4, CAP_LIGHT, None),
            (1.0, TRIM, None),
        ],
    );
    grad(
        "ivory",
        "linearGradient",
        down,
        &[
            (0.0, IVORY_DARK, None),
            (0.5, IVORY, None),
            (1.0, IVORY_DARK, None),
        ],
    );
    grad(
        "ivory-sides",
        "linearGradient",
        across,
        &[
            (0.0, HOLE, Some(0.45)),
            (0.2, HOLE, Some(0.0)),
            (0.8, HOLE, Some(0.0)),
            (1.0, HOLE, Some(0.45)),
        ],
    );
    for (name, base, light, dark) in [
        ("blue", BLUE, BLUE_LIGHT, BLUE_DARK),
        ("red", RED, RED_LIGHT, RED_DARK),
        ("black", KNOB, KNOB_RIM, KNOB),
        ("ivory", IVORY, CAP_LIGHT, IVORY_DARK),
    ] {
        grad(
            &format!("rk-{name}-p"),
            "linearGradient",
            across,
            &[(0.0, dark, None), (0.35, dark, None), (1.0, base, None)],
        );
        // From the raised end's edge to the pivot: its curved crest in the light, then down
        // into the shadow at the pivot.
        grad(
            &format!("rk-{name}-r"),
            "linearGradient",
            across,
            &[
                (0.0, dark, None),
                (0.1, base, None),
                (0.27, light, None),
                (0.5, base, None),
                (0.86, base, None),
                (1.0, dark, None),
            ],
        );
    }
    s.0.push_str("</defs>");
    s.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What XML escapes, escaped; what XML 1.0 forbids, left out.
    #[test]
    fn text_is_made_safe_for_xml() {
        assert_eq!(
            escape("Tom's <Bass> & \"Lead\""),
            "Tom&apos;s &lt;Bass&gt; &amp; &quot;Lead&quot;"
        );
        assert_eq!(escape("A\u{1}B\u{7}\u{1b}C\u{fffe}\u{ffff}"), "ABC");
        assert_eq!(escape("a\tb\nc\u{7f}é\u{10000}"), "a\tb\nc\u{7f}é\u{10000}");
    }
}
