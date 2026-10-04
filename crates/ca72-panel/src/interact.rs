//! What a pointer at a point of the drawing finds there, and what operating each kind of
//! control does to its value (normalized, 0..1).
//!
//! A knob turns with a vertical drag (fine: a tenth as fast), the mouse wheel nudges it and
//! a double click restores its default; RANGE and WAVEFORM step between their positions (a
//! click on a legend picks it, a click on the knob steps toward the side clicked); a switch
//! flips at a click anywhere on it; the wheels stay where they are left, the PITCH wheel
//! settling into its centre detent when let go near it (it has no spring).

use crate::art::{self, BIG, GRIP, Layout, POWER, POWER_BOUNDS, SIX, STD, WHEEL_SCALE};
use crate::controls::{CONTROLS, Kind};

/// What is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A control, by its index in [`CONTROLS`].
    Control(usize),
    /// A selector's legend: the selector's index and the legend's position.
    Legend(usize, usize),
    /// The POWER switch.
    Power,
    /// The editor's resize grip.
    Grip,
}

fn in_rect(x: f64, y: f64, r: [f64; 4]) -> bool {
    x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]
}

/// What is at (x, y) in the drawing.
pub fn hit(layout: &Layout, x: f64, y: f64) -> Option<Target> {
    for (i, c) in CONTROLS.iter().enumerate() {
        let (cx, cy) = c.centre();
        let (dx, dy) = (x - cx, y - cy);
        let found = match c.kind {
            Kind::Knob { big, .. } => dx.hypot(dy) <= if big { BIG.skirt } else { STD.skirt } + 8.0,
            Kind::Selector(set) => {
                for k in 0..SIX.len() {
                    let (lx, ly) = layout.legend_at(set, k);
                    if in_rect(dx - lx, dy - ly, [-18.0, -14.0, 18.0, 14.0]) {
                        return Some(Target::Legend(i, k));
                    }
                }
                dx.hypot(dy) <= 60.0
            }
            Kind::Rocker { w, h, .. } => in_rect(
                dx,
                dy,
                [-w / 2.0 - 4.0, -h / 2.0 - 4.0, w / 2.0 + 4.0, h / 2.0 + 4.0],
            ),
            Kind::Wheel { .. } => {
                let (hx, hy) = (36.0 * WHEEL_SCALE, 79.0 * WHEEL_SCALE);
                in_rect(dx, dy, [-hx, -hy, hx, hy])
            }
        };
        if found {
            return Some(Target::Control(i));
        }
    }
    let (px, py) = (art::COL + POWER.0, art::TOP + POWER.1);
    if in_rect(x - px, y - py, POWER_BOUNDS) {
        return Some(Target::Power);
    }
    if in_rect(x, y, GRIP) {
        return Some(Target::Grip);
    }
    None
}

/// Where a drag takes a control from `start`, the pointer moved by (dx, dy) logical pixels;
/// `None` for a switch, which a drag does not move.
pub fn drag(kind: &Kind, start: f64, dx: f64, dy: f64, fine: bool) -> Option<f64> {
    let rate = if fine { 0.1 } else { 1.0 };
    Some(match kind {
        Kind::Knob { .. } => (start - dy / 240.0 * rate).clamp(0.0, 1.0),
        Kind::Selector(_) => {
            let steps = ((dx - dy) / if fine { 60.0 } else { 30.0 }).round();
            (art::position(start) as f64 + steps).clamp(0.0, 5.0) / 5.0
        }
        Kind::Wheel { .. } => (start - dy / 160.0 * rate).clamp(0.0, 1.0),
        Kind::Rocker { .. } => return None,
    })
}

/// One notch of the mouse wheel or an arrow key, `up` or down.
pub fn step(kind: &Kind, v: f64, up: bool, fine: bool) -> f64 {
    let sign = if up { 1.0 } else { -1.0 };
    match kind {
        Kind::Knob { .. } => (v + sign * if fine { 0.002 } else { 0.02 }).clamp(0.0, 1.0),
        Kind::Selector(_) => (art::position(v) as f64 + sign).clamp(0.0, 5.0) / 5.0,
        Kind::Wheel { .. } => (v + sign * if fine { 0.005 } else { 0.05 }).clamp(0.0, 1.0),
        Kind::Rocker { .. } => {
            if up {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// A click that did not drag, `dx` right of the control's centre: a switch flips, a
/// selector steps toward the side clicked.
pub fn click(kind: &Kind, v: f64, dx: f64) -> Option<f64> {
    match kind {
        Kind::Rocker { .. } => Some(if v >= 0.5 { 0.0 } else { 1.0 }),
        Kind::Selector(_) => Some(
            (art::position(v) as f64 + if dx < 0.0 { -1.0 } else { 1.0 }).clamp(0.0, 5.0) / 5.0,
        ),
        _ => None,
    }
}

/// Where a control settles when let go: the PITCH wheel's centre detent catches it near it.
pub fn release(kind: &Kind, v: f64) -> f64 {
    match kind {
        Kind::Wheel { detent: true, .. } if (v - 0.5).abs() <= 0.04 => 0.5,
        _ => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::index;
    use crate::fonts::Fonts;

    fn layout() -> Layout {
        Layout::measure(&Fonts::new())
    }

    #[test]
    fn every_control_is_found_at_its_centre() {
        let l = layout();
        for (i, c) in CONTROLS.iter().enumerate() {
            let (x, y) = c.centre();
            assert_eq!(hit(&l, x, y), Some(Target::Control(i)), "{}", c.param);
        }
        assert_eq!(
            hit(&l, art::COL + POWER.0, art::TOP + POWER.1),
            Some(Target::Power)
        );
        // The name plate does nothing.
        assert_eq!(hit(&l, art::PLATE_X + 10.0, art::PLATE_Y + 10.0), None);
        assert_eq!(hit(&l, art::W - 5.0, art::H - 5.0), Some(Target::Grip));
        assert_eq!(hit(&l, 5.0, 5.0), None);
    }

    #[test]
    fn a_legend_is_found_where_it_is_printed() {
        let l = layout();
        let i = index("osc1_range").expect("a control");
        let (cx, cy) = CONTROLS[i].centre();
        let Kind::Selector(set) = CONTROLS[i].kind else {
            panic!("a selector")
        };
        for k in 0..6 {
            let (x, y) = l.legend_at(set, k);
            assert_eq!(hit(&l, cx + x, cy + y), Some(Target::Legend(i, k)));
        }
    }

    #[test]
    fn gestures_move_values_as_the_panel_does() {
        let knob = &CONTROLS[index("cutoff").expect("a control")].kind;
        assert_eq!(drag(knob, 0.5, 0.0, -120.0, false), Some(1.0));
        assert!((drag(knob, 0.5, 0.0, -120.0, true).unwrap_or(0.0) - 0.55).abs() < 1e-12);
        let sel = &CONTROLS[index("osc1_range").expect("a control")].kind;
        assert_eq!(drag(sel, 0.4, 30.0, 0.0, false), Some(0.6));
        assert_eq!(click(sel, 0.4, -5.0), Some(0.2));
        assert_eq!(step(sel, 1.0, true, false), 1.0);
        let rocker = &CONTROLS[index("osc1_on").expect("a control")].kind;
        assert_eq!(drag(rocker, 1.0, 0.0, 50.0, false), None);
        assert_eq!(click(rocker, 1.0, 0.0), Some(0.0));
        let pitch = &CONTROLS[index("pitch_wheel").expect("a control")].kind;
        assert_eq!(release(pitch, 0.53), 0.5);
        assert_eq!(release(pitch, 0.55), 0.55);
    }
}
