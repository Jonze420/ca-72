//! The presets' selector at the left of the strip and their drawer (decisions.md R10, R12;
//! the owner, 2026-10-02, asked for the presets to drop down from the bottom, in a drawer):
//! the selector names the preset the plug-in was last set to (marked • once changed), with
//! its favourite star, the previous and next preset and SAVE…, in the strip's row (the
//! owner, 2026-10-02, asked for it on the same row); the drawer: a search field, FAVOURITES
//! and MINE, the update check (R27), the tags in use as chips, the list (each row's star, its
//! name, EDITED or YOURS, its tags, and RENAME, TAGS, REVERT, DELETE), and under it the current
//! sound saved as a preset, named and tagged, and RESTORE FACTORY. The drawer opens below the
//! strip, the window growing for it (the owner, 2026-10-02, chose a drawer opening downwards,
//! extending the plug-in's window); where the host will not resize the window it slides up
//! over the panel's lower part instead. Drawn in the strip's colours and lettering, at the
//! panel's scale: the selector [`BAR_END`] panel units across the strip's row (the strip
//! draws it: [`bar_svg`]), the drawer [`DRAWER_H`] tall. What a pointer finds is [`bar_hit`]
//! and [`drawer_hit`]; the drawing and the finding share one layout.

use std::fmt;

use resvg::tiny_skia::{Color, Pixmap, Transform};
use resvg::usvg;

use crate::art::{self, W};
use crate::fonts::{FAMILY, Fonts, Weight};
use crate::svg::{N, Svg, colour, escape, put};

/// The selector's height, panel units: the strip's row it shares.
pub const BAR_H: f64 = crate::strip::STRIP_H;
/// Where the selector ends across the strip's row, panel units: the strip's own controls
/// are to the right of it.
pub const BAR_END: f64 = 1110.0;
/// The drawer's top, panel units down from the drawing's, and its height: the panel's lower
/// four fifths.
pub const DRAWER_TOP: f64 = art::H * 0.2;
pub const DRAWER_H: f64 = art::H - DRAWER_TOP;

const DIM: &str = "#8a909c";
const RAISED: &str = "#272b32";
const BORDER: &str = "#4a4640";
const ACCENT: &str = "#f0a030";
const WARN: &str = "#e5b94d";
const BACK: &str = "#1f2228";
/// The drawer's background, for what its frame does not cover while it slides.
pub fn background() -> Color {
    Color::from_rgba8(0x1f, 0x22, 0x28, 0xff)
}
const ROW_ON: &str = "#3a3022";
const ROW_HOVER: &str = "#262a31";

/// Text sizes: the bar's (the strip's), the drawer's and its small print.
const BAR_TEXT: f64 = 26.0;
const TEXT: f64 = 30.0;
const SMALL: f64 = 24.0;

/// A text field: its text and the caret, a count of characters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Field {
    pub text: String,
    pub caret: usize,
}

impl Field {
    pub fn new(text: &str) -> Self {
        Field {
            text: text.to_owned(),
            caret: text.chars().count(),
        }
    }

    fn byte(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(i, _)| i)
    }

    /// Typed at the caret.
    pub fn insert(&mut self, s: &str) {
        let at = self.byte(self.caret);
        let clean: String = s.chars().filter(|c| !c.is_control()).collect();
        self.text.insert_str(at, &clean);
        self.caret += clean.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.caret > 0 {
            let (a, b) = (self.byte(self.caret - 1), self.byte(self.caret));
            self.text.replace_range(a..b, "");
            self.caret -= 1;
        }
    }

    pub fn delete(&mut self) {
        if self.caret < self.text.chars().count() {
            let (a, b) = (self.byte(self.caret), self.byte(self.caret + 1));
            self.text.replace_range(a..b, "");
        }
    }

    pub fn left(&mut self) {
        self.caret = self.caret.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.caret = (self.caret + 1).min(self.text.chars().count());
    }

    pub fn home(&mut self) {
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.caret = self.text.chars().count();
    }

    fn before_caret(&self) -> &str {
        &self.text[..self.byte(self.caret)]
    }
}

/// The drawer's text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldId {
    Search,
    /// A row's name or tags being edited.
    Edit,
    SaveName,
    SaveTags,
}

/// What a row's buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    Rename,
    Tags,
    Revert,
    Delete,
    /// Delete, once asked.
    Confirm,
    Cancel,
}

/// A row being edited, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Rename,
    Tags,
    /// Asked before deleting (a factory preset's: hidden, Restore brings it back).
    Delete {
        factory: bool,
    },
}

/// What a pointer finds on the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarTarget {
    Star,
    Prev,
    Name,
    Next,
    Save,
}

/// What a pointer finds in the drawer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerTarget {
    Field(FieldId),
    Favourites,
    Mine,
    Close,
    Chip(usize),
    /// A row (by its index in the list), its star, one of its buttons.
    Row(usize),
    Star(usize),
    Action(usize, RowAction),
    SaveGo,
    Restore,
    /// The update check's button.
    Update,
    /// The drawer, where nothing else is.
    Back,
}

/// The colour of the update check's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Dim,
    /// A newer release.
    News,
    /// Something went wrong.
    Trouble,
}

/// The update check, at the tools' right before ×: what it says, and its button's label
/// (none drawn when empty).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UpdateScene {
    pub text: String,
    pub tone: Tone,
    pub button: String,
}

/// A row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub tags: String,
    /// `EDITED` (a factory preset the user's version stands in for) or `YOURS`.
    pub origin: Option<&'static str>,
    pub favorite: bool,
    /// The preset the plug-in was last set to.
    pub current: bool,
}

/// What the drawer shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DrawerScene {
    pub search: Field,
    pub favourites: bool,
    pub mine: bool,
    /// The tags in use, each picked as a filter or not.
    pub chips: Vec<(String, bool)>,
    pub rows: Vec<Row>,
    /// The first row shown.
    pub first: usize,
    pub editing: Option<(usize, Edit)>,
    pub edit: Field,
    pub save_name: Field,
    pub save_tags: Field,
    /// SAVE, or REPLACE when the name is a preset's.
    pub replace: bool,
    pub hint: String,
    pub focus: Option<FieldId>,
    pub hover: Option<DrawerTarget>,
    pub update: UpdateScene,
}

/// What the bar shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BarScene {
    /// The preset's name ("NO PRESET" when none).
    pub name: String,
    /// The preset is in the library (else the name is the last one set, gone since).
    pub found: bool,
    /// Its values changed since it was set.
    pub changed: bool,
    pub favorite: bool,
    pub open: bool,
    /// The drawer opens below the bar (else over the panel): which way the arrow points.
    pub below: bool,
    pub hover: Option<BarTarget>,
}

// ---- The selector's layout (panel units across, the strip's units down).

const BAR_MID: f64 = BAR_H / 2.0;
const BTN_H: f64 = 46.0;
const STAR_X: f64 = 40.0;
const STEP_W: f64 = 54.0;
const PREV_X: f64 = 110.0;
const NAME_X: f64 = 180.0;
const NAME_END: f64 = 860.0;
const NEXT_X: f64 = 876.0;
const SAVE_X: f64 = 946.0;
const SAVE_W: f64 = 150.0;

/// What is at (`x`, `y`) on the bar.
pub fn bar_hit(x: f64, y: f64) -> Option<BarTarget> {
    if (y - BAR_MID).abs() > BTN_H / 2.0 + 6.0 {
        return None;
    }
    let within = |x0: f64, w: f64| (x0..x0 + w).contains(&x);
    if within(STAR_X, STEP_W) {
        Some(BarTarget::Star)
    } else if within(PREV_X, STEP_W) {
        Some(BarTarget::Prev)
    } else if within(NAME_X, NAME_END - NAME_X) {
        Some(BarTarget::Name)
    } else if within(NEXT_X, STEP_W) {
        Some(BarTarget::Next)
    } else if within(SAVE_X, SAVE_W) {
        Some(BarTarget::Save)
    } else {
        None
    }
}

fn button(
    out: &mut Svg,
    (x, y, w, h): (f64, f64, f64, f64),
    label: &str,
    size: f64,
    on: bool,
    hover: bool,
    fill: &str,
) {
    put!(
        out,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='7' fill='{RAISED}' stroke='{}' stroke-width='{}'/>",
        N(x),
        N(y),
        N(w),
        N(h),
        if on { ACCENT } else { BORDER },
        if hover { 4 } else { 2 }
    );
    put!(
        out,
        "<text x='{}' y='{}' font-size='{}' fill='{fill}' text-anchor='middle' letter-spacing='1'>{}</text>",
        N(x + w / 2.0),
        N(y + h / 2.0 + size * 0.36),
        N(size),
        escape(label)
    );
}

fn text(out: &mut Svg, x: f64, y: f64, s: &str, size: f64, fill: &str, anchor: &str) {
    put!(
        out,
        "<text x='{}' y='{}' font-size='{}' fill='{fill}' text-anchor='{anchor}' letter-spacing='1'>{}</text>",
        N(x),
        N(y + size * 0.36),
        N(size),
        escape(s)
    );
}

/// A five-pointed star (the panel's typeface has none), filled for a favourite.
fn star(out: &mut Svg, cx: f64, cy: f64, r: f64, filled: bool, fill: &str) {
    let mut d = String::new();
    for k in 0..10 {
        let a = std::f64::consts::PI * (f64::from(k) / 5.0 - 0.5);
        let rr = if k % 2 == 0 { r } else { r * 0.42 };
        let (x, y) = (cx + rr * a.cos(), cy + rr * a.sin());
        d.push_str(&format!(
            "{}{} {} ",
            if k == 0 { "M" } else { "L" },
            N(x),
            N(y)
        ));
    }
    if filled {
        put!(out, "<path d='{d}Z' fill='{fill}'/>");
    } else {
        put!(
            out,
            "<path d='{d}Z' fill='none' stroke='{fill}' stroke-width='2.5' stroke-linejoin='round'/>"
        );
    }
}

/// A small triangle pointing up or down (the typeface has none).
fn triangle(out: &mut Svg, cx: f64, cy: f64, size: f64, up: bool, fill: &str) {
    let (h, w) = (size * 0.6, size * 0.7);
    let (tip, base) = if up {
        (cy - h / 2.0, cy + h / 2.0)
    } else {
        (cy + h / 2.0, cy - h / 2.0)
    };
    put!(
        out,
        "<path d='M {} {} L {} {} L {} {} Z' fill='{fill}'/>",
        N(cx),
        N(tip),
        N(cx - w / 2.0),
        N(base),
        N(cx + w / 2.0),
        N(base)
    );
}

/// The selector, drawn on the strip's row (which draws its background).
pub fn bar_svg(fonts: &Fonts, s: &BarScene) -> String {
    let mut out = Svg::default();
    let y = BAR_MID - BTN_H / 2.0;
    let hover = |t| s.hover == Some(t);
    let star_fill = if !s.found {
        BORDER
    } else if s.favorite {
        ACCENT
    } else {
        DIM
    };
    button(
        &mut out,
        (STAR_X, y, STEP_W, BTN_H),
        "",
        30.0,
        false,
        hover(BarTarget::Star),
        star_fill,
    );
    star(
        &mut out,
        STAR_X + STEP_W / 2.0,
        BAR_MID + 1.0,
        15.0,
        s.favorite,
        star_fill,
    );
    button(
        &mut out,
        (PREV_X, y, STEP_W, BTN_H),
        "‹",
        34.0,
        false,
        hover(BarTarget::Prev),
        colour::LEGEND,
    );
    button(
        &mut out,
        (NEXT_X, y, STEP_W, BTN_H),
        "›",
        34.0,
        false,
        hover(BarTarget::Next),
        colour::LEGEND,
    );
    // The name: left, clipped short of the arrow that opens the drawer.
    put!(
        out,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='7' fill='{RAISED}' stroke='{}' stroke-width='{}'/>",
        N(NAME_X),
        N(y),
        N(NAME_END - NAME_X),
        N(BTN_H),
        if s.open { ACCENT } else { BORDER },
        if hover(BarTarget::Name) { 4 } else { 2 }
    );
    let shown = if s.name.is_empty() {
        "NO PRESET".to_owned()
    } else if s.changed {
        format!("{} •", s.name)
    } else {
        s.name.clone()
    };
    let room = NAME_END - NAME_X - 90.0;
    let shown = fit(fonts, &shown, BAR_TEXT, room);
    let fill = if s.name.is_empty() || !s.found {
        DIM
    } else {
        colour::LEGEND
    };
    text(
        &mut out,
        NAME_X + 20.0,
        BAR_MID,
        &shown,
        BAR_TEXT,
        fill,
        "start",
    );
    // Pointing where the drawer goes: down to open below, up to close it again.
    triangle(
        &mut out,
        NAME_END - 34.0,
        BAR_MID,
        26.0,
        s.open == s.below,
        DIM,
    );
    button(
        &mut out,
        (SAVE_X, y, SAVE_W, BTN_H),
        "SAVE…",
        BAR_TEXT,
        false,
        hover(BarTarget::Save),
        colour::LEGEND,
    );
    out.0
}

/// `s` shortened with an ellipsis to fit `room` at `size`.
fn fit(fonts: &Fonts, s: &str, size: f64, room: f64) -> String {
    let w = |t: &str| fonts.advance(t, size, Weight::Regular, 1.0);
    if w(s) <= room {
        return s.to_owned();
    }
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let t: String = chars.iter().collect::<String>() + "…";
        if w(&t) <= room {
            return t;
        }
    }
    "…".into()
}

// ---- The drawer's layout (panel units across, drawer units down from its top).

const PAD: f64 = 30.0;
const TOOLS_Y: f64 = PAD;
const TOOLS_H: f64 = 60.0;
const SEARCH_END: f64 = 1900.0;
const FAVS_X: f64 = 1930.0;
const FAVS_W: f64 = 320.0;
const MINE_X: f64 = 2280.0;
const MINE_W: f64 = 180.0;
const CLOSE_W: f64 = 64.0;
/// The update check's button, before ×; its text ends short of the button.
const UPDATE_W: f64 = 380.0;
const UPDATE_X: f64 = W - PAD - CLOSE_W - 24.0 - UPDATE_W;
const UPDATE_TEXT_END: f64 = UPDATE_X - 24.0;
const UPDATE_LABEL: f64 = TEXT;
const CHIPS_Y: f64 = 112.0;
const CHIP_H: f64 = 44.0;
const LIST_Y: f64 = 176.0;
/// A row of the list's height, panel units.
pub const ROW_H: f64 = 62.0;
const SAVE_Y: f64 = DRAWER_H - 94.0;
const SAVE_H: f64 = 60.0;
const SAVE_NAME_X: f64 = 250.0;
const SAVE_NAME_END: f64 = 1150.0;
const SAVE_TAGS_X: f64 = 1180.0;
const SAVE_TAGS_END: f64 = 2050.0;
const SAVE_GO_X: f64 = 2080.0;
const SAVE_GO_W: f64 = 220.0;
const RESTORE_W: f64 = 480.0;
const ACTION_W: f64 = 190.0;
const ACTION_GAP: f64 = 14.0;
const ROW_NAME_X: f64 = PAD + 100.0;
const ROW_TAGS_X: f64 = 1250.0;

/// How many rows the list shows.
pub const ROWS_SHOWN: usize = ((SAVE_Y - 20.0 - LIST_Y) / ROW_H) as usize;

fn list_bottom() -> f64 {
    LIST_Y + ROWS_SHOWN as f64 * ROW_H
}

/// The chips' boxes (x, width), as they flow along their row (those past its end left out).
fn chip_boxes(fonts: &Fonts, chips: &[(String, bool)]) -> Vec<(f64, f64)> {
    let mut x = PAD;
    let mut out = Vec::new();
    for (t, _) in chips {
        let w = fonts.advance(t, SMALL, Weight::Regular, 1.0) + 40.0;
        if x + w > W - PAD {
            break;
        }
        out.push((x, w));
        x += w + 14.0;
    }
    out
}

/// A row's buttons, right to left from the row's end: what each does and says.
fn actions(row: &Row, editing: Option<Edit>) -> Vec<(RowAction, &'static str)> {
    match editing {
        Some(Edit::Rename | Edit::Tags) => Vec::new(),
        Some(Edit::Delete { .. }) => vec![
            (RowAction::Cancel, "CANCEL"),
            (RowAction::Confirm, "DELETE"),
        ],
        None => {
            let mut a = vec![(RowAction::Delete, "DELETE")];
            if row.origin == Some("EDITED") {
                a.push((RowAction::Revert, "REVERT"));
            }
            a.push((RowAction::Tags, "TAGS"));
            a.push((RowAction::Rename, "RENAME"));
            a
        }
    }
}

fn action_x(k: usize) -> f64 {
    W - PAD - 20.0 - (k as f64 + 1.0) * ACTION_W - k as f64 * ACTION_GAP
}

/// Where a field is: x from and to, y, height.
fn field_box(id: FieldId, row_y: Option<f64>) -> (f64, f64, f64, f64) {
    match id {
        FieldId::Search => (PAD, SEARCH_END, TOOLS_Y, TOOLS_H),
        FieldId::Edit => (
            ROW_NAME_X - 10.0,
            action_x(0) - 30.0,
            row_y.unwrap_or(LIST_Y) + 6.0,
            ROW_H - 12.0,
        ),
        FieldId::SaveName => (SAVE_NAME_X, SAVE_NAME_END, SAVE_Y, SAVE_H),
        FieldId::SaveTags => (SAVE_TAGS_X, SAVE_TAGS_END, SAVE_Y, SAVE_H),
    }
}

/// The y of the list's row `i`, if it is shown.
fn row_y(s: &DrawerScene, i: usize) -> Option<f64> {
    (i >= s.first && i < s.first + ROWS_SHOWN).then(|| LIST_Y + (i - s.first) as f64 * ROW_H)
}

/// The caret's place in a field for a pointer at `x` across it (the nearest gap).
pub fn caret_at(fonts: &Fonts, f: &Field, id: FieldId, x: f64) -> usize {
    let (x0, x1, _, _) = field_box(id, None);
    let shift = scroll_of(fonts, f, x1 - x0);
    let want = x - (x0 + 14.0) + shift;
    let mut best = (0usize, f64::INFINITY);
    let mut s = String::new();
    for (i, c) in std::iter::once(None)
        .chain(f.text.chars().map(Some))
        .enumerate()
    {
        if let Some(c) = c {
            s.push(c);
        }
        let d = (fonts.advance(&s, TEXT, Weight::Regular, 1.0) - want).abs();
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// How far a field's text is moved left so its caret shows.
fn scroll_of(fonts: &Fonts, f: &Field, width: f64) -> f64 {
    let caret = fonts.advance(f.before_caret(), TEXT, Weight::Regular, 1.0);
    (caret - (width - 40.0)).max(0.0)
}

/// What is at (`x`, `y`) in the drawer (drawer units).
pub fn drawer_hit(fonts: &Fonts, s: &DrawerScene, x: f64, y: f64) -> Option<DrawerTarget> {
    if !(0.0..DRAWER_H).contains(&y) || !(0.0..W).contains(&x) {
        return None;
    }
    let inside =
        |x0: f64, x1: f64, y0: f64, h: f64| (x0..x1).contains(&x) && (y0..y0 + h).contains(&y);
    if inside(PAD, SEARCH_END, TOOLS_Y, TOOLS_H) {
        return Some(DrawerTarget::Field(FieldId::Search));
    }
    if inside(FAVS_X, FAVS_X + FAVS_W, TOOLS_Y, TOOLS_H) {
        return Some(DrawerTarget::Favourites);
    }
    if inside(MINE_X, MINE_X + MINE_W, TOOLS_Y, TOOLS_H) {
        return Some(DrawerTarget::Mine);
    }
    if inside(W - PAD - CLOSE_W, W - PAD, TOOLS_Y, TOOLS_H) {
        return Some(DrawerTarget::Close);
    }
    if !s.update.button.is_empty() && inside(UPDATE_X, UPDATE_X + UPDATE_W, TOOLS_Y, TOOLS_H) {
        return Some(DrawerTarget::Update);
    }
    for (i, (cx, cw)) in chip_boxes(fonts, &s.chips).into_iter().enumerate() {
        if inside(cx, cx + cw, CHIPS_Y, CHIP_H) {
            return Some(DrawerTarget::Chip(i));
        }
    }
    if (LIST_Y..list_bottom()).contains(&y) {
        let i = s.first + ((y - LIST_Y) / ROW_H) as usize;
        if i >= s.rows.len() {
            return Some(DrawerTarget::Back);
        }
        let ry = LIST_Y + (i - s.first) as f64 * ROW_H;
        let editing = s.editing.filter(|(r, _)| *r == i).map(|(_, e)| e);
        if matches!(editing, Some(Edit::Rename | Edit::Tags)) {
            let (x0, x1, fy, fh) = field_box(FieldId::Edit, Some(ry));
            if inside(x0, x1, fy, fh) {
                return Some(DrawerTarget::Field(FieldId::Edit));
            }
        }
        for (k, (a, _)) in actions(&s.rows[i], editing).into_iter().enumerate() {
            let ax = action_x(k);
            if inside(ax, ax + ACTION_W, ry + 8.0, ROW_H - 16.0) {
                return Some(DrawerTarget::Action(i, a));
            }
        }
        if inside(PAD, PAD + 80.0, ry, ROW_H) {
            return Some(DrawerTarget::Star(i));
        }
        return Some(DrawerTarget::Row(i));
    }
    for id in [FieldId::SaveName, FieldId::SaveTags] {
        let (x0, x1, fy, fh) = field_box(id, None);
        if inside(x0, x1, fy, fh) {
            return Some(DrawerTarget::Field(id));
        }
    }
    if inside(SAVE_GO_X, SAVE_GO_X + SAVE_GO_W, SAVE_Y, SAVE_H) {
        return Some(DrawerTarget::SaveGo);
    }
    if inside(W - PAD - RESTORE_W, W - PAD, SAVE_Y, SAVE_H) {
        return Some(DrawerTarget::Restore);
    }
    Some(DrawerTarget::Back)
}

/// A text field, its caret when focused, its text moved left to keep the caret in view.
#[allow(clippy::too_many_arguments)]
fn field(
    out: &mut Svg,
    fonts: &Fonts,
    id: FieldId,
    n: usize,
    f: &Field,
    focused: bool,
    placeholder: &str,
    row: Option<f64>,
) {
    let (x0, x1, y, h) = field_box(id, row);
    put!(
        out,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='7' fill='{}' stroke='{}' stroke-width='{}'/>",
        N(x0),
        N(y),
        N(x1 - x0),
        N(h),
        colour::PANEL,
        if focused { ACCENT } else { BORDER },
        if focused { 3 } else { 2 }
    );
    put!(
        out,
        "<clipPath id='f{n}'><rect x='{}' y='{}' width='{}' height='{}'/></clipPath>",
        N(x0 + 4.0),
        N(y),
        N(x1 - x0 - 8.0),
        N(h)
    );
    let shift = if focused {
        scroll_of(fonts, f, x1 - x0)
    } else {
        0.0
    };
    put!(out, "<g clip-path='url(#f{n})'>");
    if f.text.is_empty() && !focused {
        text(out, x0 + 14.0, y + h / 2.0, placeholder, TEXT, DIM, "start");
    } else {
        text(
            out,
            x0 + 14.0 - shift,
            y + h / 2.0,
            &f.text,
            TEXT,
            colour::LEGEND,
            "start",
        );
    }
    if focused {
        let cx = x0 + 14.0 - shift + fonts.advance(f.before_caret(), TEXT, Weight::Regular, 1.0);
        put!(
            out,
            "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{ACCENT}' stroke-width='3'/>",
            N(cx),
            N(y + 10.0),
            N(cx),
            N(y + h - 10.0)
        );
    }
    put!(out, "</g>");
}

/// The drawer's SVG body.
fn drawer_body(fonts: &Fonts, s: &DrawerScene) -> String {
    let mut out = Svg::default();
    let hover = |t: DrawerTarget| s.hover == Some(t);
    put!(
        out,
        "<rect x='0' y='0' width='{}' height='{}' fill='{BACK}'/>",
        N(W),
        N(DRAWER_H)
    );
    put!(
        out,
        "<line x1='0' y1='2' x2='{}' y2='2' stroke='{ACCENT}' stroke-width='4'/>",
        N(W)
    );
    // The tools.
    field(
        &mut out,
        fonts,
        FieldId::Search,
        0,
        &s.search,
        s.focus == Some(FieldId::Search),
        "SEARCH: NAME, DESCRIPTION, TAGS",
        None,
    );
    button(
        &mut out,
        (FAVS_X, TOOLS_Y, FAVS_W, TOOLS_H),
        "   FAVOURITES",
        TEXT,
        s.favourites,
        hover(DrawerTarget::Favourites),
        colour::LEGEND,
    );
    star(
        &mut out,
        FAVS_X + 44.0,
        TOOLS_Y + TOOLS_H / 2.0,
        14.0,
        true,
        if s.favourites { ACCENT } else { DIM },
    );
    button(
        &mut out,
        (MINE_X, TOOLS_Y, MINE_W, TOOLS_H),
        "MINE",
        TEXT,
        s.mine,
        hover(DrawerTarget::Mine),
        colour::LEGEND,
    );
    button(
        &mut out,
        (W - PAD - CLOSE_W, TOOLS_Y, CLOSE_W, TOOLS_H),
        "×",
        40.0,
        false,
        hover(DrawerTarget::Close),
        colour::LEGEND,
    );
    // The update check.
    let u = &s.update;
    let tone = match u.tone {
        Tone::Dim => DIM,
        Tone::News => ACCENT,
        Tone::Trouble => WARN,
    };
    let room = UPDATE_TEXT_END - (MINE_X + MINE_W + 30.0);
    text(
        &mut out,
        UPDATE_TEXT_END,
        TOOLS_Y + TOOLS_H / 2.0,
        &fit(fonts, &u.text, SMALL, room),
        SMALL,
        tone,
        "end",
    );
    if !u.button.is_empty() {
        button(
            &mut out,
            (UPDATE_X, TOOLS_Y, UPDATE_W, TOOLS_H),
            &u.button,
            UPDATE_LABEL,
            u.tone == Tone::News,
            hover(DrawerTarget::Update),
            colour::LEGEND,
        );
    }
    // The chips.
    for (i, (x, w)) in chip_boxes(fonts, &s.chips).into_iter().enumerate() {
        let (t, on) = &s.chips[i];
        put!(
            out,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='{}' fill='{RAISED}' stroke='{}' stroke-width='{}'/>",
            N(x),
            N(CHIPS_Y),
            N(w),
            N(CHIP_H),
            N(CHIP_H / 2.0),
            if *on { ACCENT } else { BORDER },
            if hover(DrawerTarget::Chip(i)) { 4 } else { 2 }
        );
        text(
            &mut out,
            x + w / 2.0,
            CHIPS_Y + CHIP_H / 2.0,
            t,
            SMALL,
            if *on { colour::LEGEND } else { DIM },
            "middle",
        );
    }
    // The list.
    put!(
        out,
        "<rect x='{}' y='{}' width='{}' height='{}' rx='7' fill='{}' stroke='{BORDER}' stroke-width='2'/>",
        N(PAD),
        N(LIST_Y),
        N(W - 2.0 * PAD),
        N(list_bottom() - LIST_Y),
        colour::PANEL
    );
    if s.rows.is_empty() {
        text(
            &mut out,
            PAD + 30.0,
            LIST_Y + ROW_H / 2.0,
            "NO PRESET MATCHES.",
            TEXT,
            DIM,
            "start",
        );
    }
    for (i, r) in s.rows.iter().enumerate() {
        let Some(y) = row_y(s, i) else { continue };
        let editing = s.editing.filter(|(e, _)| *e == i).map(|(_, e)| e);
        let row_hovered = matches!(s.hover, Some(DrawerTarget::Row(h) | DrawerTarget::Star(h) | DrawerTarget::Action(h, _)) if h == i);
        if r.current || row_hovered {
            put!(
                out,
                "<rect x='{}' y='{}' width='{}' height='{}' fill='{}'/>",
                N(PAD + 2.0),
                N(y),
                N(W - 2.0 * PAD - 4.0),
                N(ROW_H),
                if r.current { ROW_ON } else { ROW_HOVER }
            );
        }
        let mid = y + ROW_H / 2.0;
        star(
            &mut out,
            PAD + 40.0,
            mid,
            15.0,
            r.favorite,
            if r.favorite { ACCENT } else { DIM },
        );
        match editing {
            Some(Edit::Rename | Edit::Tags) => {
                field(
                    &mut out,
                    fonts,
                    FieldId::Edit,
                    1,
                    &s.edit,
                    s.focus == Some(FieldId::Edit),
                    "",
                    Some(y),
                );
            }
            _ => {
                let name = fit(fonts, &r.name, TEXT, ROW_TAGS_X - ROW_NAME_X - 200.0);
                text(
                    &mut out,
                    ROW_NAME_X,
                    mid,
                    &name,
                    TEXT,
                    colour::LEGEND,
                    "start",
                );
                if let Some(o) = r.origin {
                    let x = ROW_NAME_X + fonts.advance(&name, TEXT, Weight::Regular, 1.0) + 24.0;
                    let w = fonts.advance(o, SMALL, Weight::Regular, 1.0) + 24.0;
                    put!(
                        out,
                        "<rect x='{}' y='{}' width='{}' height='{}' rx='5' fill='none' stroke='{BORDER}' stroke-width='2'/>",
                        N(x),
                        N(mid - 18.0),
                        N(w),
                        N(36.0)
                    );
                    text(&mut out, x + w / 2.0, mid, o, SMALL, DIM, "middle");
                }
                let shows_actions = row_hovered || r.current || editing.is_some();
                let tags_end = if shows_actions {
                    action_x(actions(r, editing).len().saturating_sub(1)) - 30.0
                } else {
                    W - PAD - 30.0
                };
                if let Some(Edit::Delete { factory }) = editing {
                    let ask = if factory {
                        "DELETE IT? RESTORE FACTORY BRINGS IT BACK"
                    } else {
                        "DELETE IT?"
                    };
                    text(&mut out, tags_end, mid, ask, SMALL, WARN, "end");
                } else {
                    let tags = fit(fonts, &r.tags, SMALL, (tags_end - ROW_TAGS_X).max(0.0));
                    text(&mut out, ROW_TAGS_X, mid, &tags, SMALL, DIM, "start");
                }
            }
        }
        let shows_actions = row_hovered || r.current || editing.is_some();
        if shows_actions {
            for (k, (a, label)) in actions(r, editing).into_iter().enumerate() {
                let fill = if a == RowAction::Confirm {
                    WARN
                } else {
                    colour::LEGEND
                };
                button(
                    &mut out,
                    (action_x(k), y + 8.0, ACTION_W, ROW_H - 16.0),
                    label,
                    SMALL,
                    a == RowAction::Confirm,
                    hover(DrawerTarget::Action(i, a)),
                    fill,
                );
            }
        }
    }
    // More above or below.
    if s.first > 0 {
        triangle(&mut out, W - PAD - 26.0, LIST_Y + 16.0, 22.0, true, DIM);
    }
    if s.first + ROWS_SHOWN < s.rows.len() {
        triangle(
            &mut out,
            W - PAD - 26.0,
            list_bottom() - 16.0,
            22.0,
            false,
            DIM,
        );
    }
    // Saving.
    text(
        &mut out,
        PAD,
        SAVE_Y + SAVE_H / 2.0,
        "SAVE AS",
        TEXT,
        colour::LEGEND,
        "start",
    );
    field(
        &mut out,
        fonts,
        FieldId::SaveName,
        2,
        &s.save_name,
        s.focus == Some(FieldId::SaveName),
        "NAME",
        None,
    );
    field(
        &mut out,
        fonts,
        FieldId::SaveTags,
        3,
        &s.save_tags,
        s.focus == Some(FieldId::SaveTags),
        "TAGS: BASS, DARK",
        None,
    );
    button(
        &mut out,
        (SAVE_GO_X, SAVE_Y, SAVE_GO_W, SAVE_H),
        if s.replace { "REPLACE" } else { "SAVE" },
        TEXT,
        s.replace,
        hover(DrawerTarget::SaveGo),
        colour::LEGEND,
    );
    let hint = fit(
        fonts,
        &s.hint,
        SMALL,
        W - PAD - RESTORE_W - 30.0 - (SAVE_GO_X + SAVE_GO_W + 30.0),
    );
    text(
        &mut out,
        SAVE_GO_X + SAVE_GO_W + 30.0,
        SAVE_Y + SAVE_H / 2.0,
        &hint,
        SMALL,
        DIM,
        "start",
    );
    button(
        &mut out,
        (W - PAD - RESTORE_W, SAVE_Y, RESTORE_W, SAVE_H),
        "RESTORE FACTORY",
        TEXT,
        false,
        hover(DrawerTarget::Restore),
        colour::LEGEND,
    );
    out.0
}

/// A renderer of an SVG body `w` by `h` panel units at a scale.
struct Layer {
    fonts: Fonts,
    options: usvg::Options<'static>,
    scale: f64,
    h: f64,
    frame: Pixmap,
}

fn layer_size(scale: f64, h: f64) -> (u32, u32) {
    (
        (W * scale).ceil().max(1.0) as u32,
        (h * scale).ceil().max(1.0) as u32,
    )
}

impl Layer {
    fn new(scale: f64, h: f64) -> Self {
        let fonts = Fonts::new();
        let options = usvg::Options {
            fontdb: fonts.database(),
            font_family: FAMILY.into(),
            ..usvg::Options::default()
        };
        let (pw, ph) = layer_size(scale, h);
        Layer {
            fonts,
            options,
            scale,
            h,
            frame: Pixmap::new(pw, ph).expect("a layer of at least a pixel"),
        }
    }

    fn rescale(&mut self, scale: f64) -> bool {
        if scale == self.scale {
            return false;
        }
        let (pw, ph) = layer_size(scale, self.h);
        self.scale = scale;
        self.frame = Pixmap::new(pw, ph).expect("a layer of at least a pixel");
        true
    }

    fn draw(&mut self, body: &str) {
        let (w, h) = (self.frame.width(), self.frame.height());
        let view = [
            0.0,
            0.0,
            f64::from(w) / self.scale,
            f64::from(h) / self.scale,
        ];
        let doc = art::document(view, w, h, body);
        self.frame.fill(background());
        if let Ok(tree) = usvg::Tree::from_str(&doc, &self.options) {
            resvg::render(&tree, Transform::identity(), &mut self.frame.as_mut());
        }
    }
}

/// The drawer's renderer at the panel's scale.
pub struct DrawerRenderer {
    layer: Layer,
    shown: Option<DrawerScene>,
}

impl fmt::Debug for DrawerRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrawerRenderer")
            .field("scale", &self.layer.scale)
            .finish()
    }
}

impl DrawerRenderer {
    pub fn new(scale: f64) -> Self {
        DrawerRenderer {
            layer: Layer::new(scale, DRAWER_H),
            shown: None,
        }
    }

    pub fn rescale(&mut self, scale: f64) {
        if self.layer.rescale(scale) {
            self.shown = None;
        }
    }

    pub fn frame(&self) -> &Pixmap {
        &self.layer.frame
    }

    /// The typeface the drawer measures with (for [`drawer_hit`] and [`caret_at`]).
    pub fn fonts(&self) -> &Fonts {
        &self.layer.fonts
    }

    /// Draws `scene`: whether the frame changed.
    pub fn render(&mut self, scene: &DrawerScene) -> bool {
        if self.shown.as_ref() == Some(scene) {
            return false;
        }
        let body = drawer_body(&self.layer.fonts, scene);
        self.layer.draw(&body);
        self.shown = Some(scene.clone());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> DrawerScene {
        DrawerScene {
            chips: vec![("bass".into(), false), ("lead".into(), true)],
            rows: (0..30)
                .map(|i| Row {
                    name: format!("Preset {i}"),
                    tags: "bass · dark".into(),
                    origin: (i == 2).then_some("EDITED"),
                    favorite: i % 3 == 0,
                    current: i == 1,
                })
                .collect(),
            ..DrawerScene::default()
        }
    }

    #[test]
    fn a_field_edits_at_its_caret() {
        let mut f = Field::new("Bàss");
        f.left();
        f.backspace();
        assert_eq!((f.text.as_str(), f.caret), ("Bàs", 2));
        f.insert("ü\u{7}");
        assert_eq!((f.text.as_str(), f.caret), ("Bàüs", 3));
        f.home();
        f.delete();
        f.end();
        f.insert("!");
        assert_eq!(f.text, "àüs!");
    }

    #[test]
    fn the_bar_finds_each_control_where_it_is_drawn() {
        assert_eq!(bar_hit(STAR_X + 10.0, BAR_MID), Some(BarTarget::Star));
        assert_eq!(bar_hit(PREV_X + 10.0, BAR_MID), Some(BarTarget::Prev));
        assert_eq!(
            bar_hit((NAME_X + NAME_END) / 2.0, BAR_MID),
            Some(BarTarget::Name)
        );
        assert_eq!(bar_hit(BAR_END + 10.0, BAR_MID), None);
        assert_eq!(bar_hit(NEXT_X + 10.0, BAR_MID), Some(BarTarget::Next));
        assert_eq!(bar_hit(SAVE_X + 10.0, BAR_MID), Some(BarTarget::Save));
        assert_eq!(bar_hit(100.0, BAR_MID), None);
        assert_eq!(bar_hit(1200.0, 2.0), None);
    }

    #[test]
    fn the_drawer_finds_each_control_where_it_is_drawn() {
        let fonts = Fonts::new();
        let s = scene();
        let at = |x, y| drawer_hit(&fonts, &s, x, y);
        assert_eq!(
            at(100.0, TOOLS_Y + 20.0),
            Some(DrawerTarget::Field(FieldId::Search))
        );
        assert_eq!(
            at(FAVS_X + 20.0, TOOLS_Y + 20.0),
            Some(DrawerTarget::Favourites)
        );
        assert_eq!(at(MINE_X + 20.0, TOOLS_Y + 20.0), Some(DrawerTarget::Mine));
        assert_eq!(
            at(W - PAD - 20.0, TOOLS_Y + 20.0),
            Some(DrawerTarget::Close)
        );
        // The update check's button, when it has one.
        let (ux, uy) = update_centre();
        assert_eq!(at(ux, uy), Some(DrawerTarget::Back));
        let mut s1 = scene();
        s1.update.button = "CHECK FOR UPDATES".into();
        assert_eq!(drawer_hit(&fonts, &s1, ux, uy), Some(DrawerTarget::Update));
        assert_eq!(
            drawer_hit(&fonts, &s1, UPDATE_X - 10.0, uy),
            Some(DrawerTarget::Back)
        );
        assert_eq!(at(PAD + 10.0, CHIPS_Y + 20.0), Some(DrawerTarget::Chip(0)));
        let row = |i: usize| LIST_Y + i as f64 * ROW_H + ROW_H / 2.0;
        assert_eq!(at(800.0, row(3)), Some(DrawerTarget::Row(3)));
        assert_eq!(at(PAD + 40.0, row(3)), Some(DrawerTarget::Star(3)));
        assert_eq!(
            at(action_x(0) + 20.0, row(3)),
            Some(DrawerTarget::Action(3, RowAction::Delete))
        );
        // An edited factory preset's row has REVERT.
        assert_eq!(
            at(action_x(1) + 20.0, row(2)),
            Some(DrawerTarget::Action(2, RowAction::Revert))
        );
        assert_eq!(
            at(action_x(1) + 20.0, row(3)),
            Some(DrawerTarget::Action(3, RowAction::Tags))
        );
        assert_eq!(
            at(SAVE_NAME_X + 20.0, SAVE_Y + 20.0),
            Some(DrawerTarget::Field(FieldId::SaveName))
        );
        assert_eq!(
            at(SAVE_TAGS_X + 20.0, SAVE_Y + 20.0),
            Some(DrawerTarget::Field(FieldId::SaveTags))
        );
        assert_eq!(
            at(SAVE_GO_X + 20.0, SAVE_Y + 20.0),
            Some(DrawerTarget::SaveGo)
        );
        assert_eq!(
            at(W - PAD - 20.0, SAVE_Y + 20.0),
            Some(DrawerTarget::Restore)
        );
        // Scrolled: the first row shown is the list's `first`.
        let mut s2 = scene();
        s2.first = 10;
        assert_eq!(
            drawer_hit(&fonts, &s2, 800.0, row(0)),
            Some(DrawerTarget::Row(10))
        );
        // A row asking before deleting shows DELETE and CANCEL; one being renamed, its field.
        s2.editing = Some((11, Edit::Delete { factory: false }));
        assert_eq!(
            drawer_hit(&fonts, &s2, action_x(1) + 20.0, row(1)),
            Some(DrawerTarget::Action(11, RowAction::Confirm))
        );
        s2.editing = Some((11, Edit::Rename));
        assert_eq!(
            drawer_hit(&fonts, &s2, 800.0, row(1)),
            Some(DrawerTarget::Field(FieldId::Edit))
        );
        const { assert!(ROWS_SHOWN >= 8) };
    }

    #[test]
    fn a_click_puts_the_caret_at_the_nearest_gap() {
        let fonts = Fonts::new();
        let f = Field::new("Deep Bass");
        let (x0, _, _, _) = field_box(FieldId::Search, None);
        assert_eq!(caret_at(&fonts, &f, FieldId::Search, x0), 0);
        let after_deep = x0 + 14.0 + fonts.advance("Deep", TEXT, Weight::Regular, 1.0);
        assert_eq!(caret_at(&fonts, &f, FieldId::Search, after_deep + 2.0), 4);
        assert_eq!(caret_at(&fonts, &f, FieldId::Search, x0 + 5000.0), 9);
    }

    /// The drawer draws, and draws again only when its scene changes.
    #[test]
    fn it_draws_when_its_scene_changes() {
        let mut d = DrawerRenderer::new(0.4);
        let mut s = scene();
        s.focus = Some(FieldId::Search);
        s.search = Field::new("bass");
        assert!(d.render(&s));
        assert!(!d.render(&s));
        s.first = 3;
        assert!(d.render(&s));
        // Something is drawn: the list's rows differ from the background.
        let px = d.frame().pixels();
        let distinct = px
            .iter()
            .map(|p| (p.red(), p.green(), p.blue()))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(distinct.len() > 20, "{} colours", distinct.len());
    }

    /// A control character in a preset's name or a tag (a file written elsewhere) is drawn
    /// as if it were not there: it had blanked the whole drawer, and the strip while that
    /// preset was the current one.
    #[test]
    fn a_control_character_in_a_name_blanks_nothing() {
        let drawer = |s: &DrawerScene| {
            let mut d = DrawerRenderer::new(0.4);
            d.render(s);
            d.frame().clone()
        };
        let mut odd = scene();
        odd.rows[0].name = "Preset\u{1} 0".into();
        odd.rows[1].tags = "bass\u{7} · dark".into();
        assert_eq!(drawer(&odd).data(), drawer(&scene()).data());
        let strip = |name: &str| {
            let mut r = crate::strip::StripRenderer::new(0.4);
            r.render(&crate::strip::StripScene {
                bar: BarScene {
                    name: name.into(),
                    found: true,
                    ..BarScene::default()
                },
                ..crate::strip::StripScene::default()
            });
            r.frame().clone()
        };
        assert_eq!(strip("Warped\u{1b} Pad").data(), strip("Warped Pad").data());
        assert_ne!(strip("Warped Pad").data(), strip("").data());
    }
}

/// The strip with the selector and a drawer drawn to PNG files for a look (`CA72_PRESETS_PNG=<dir> cargo test
/// -p ca72-panel -- --ignored presets_png`).
#[cfg(test)]
mod png {
    use super::*;

    #[test]
    #[ignore = "writes images for a look"]
    fn presets_png() {
        let Some(dir) = std::env::var_os("CA72_PRESETS_PNG") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let mut strip = crate::strip::StripRenderer::new(0.4);
        strip.render(&crate::strip::StripScene {
            poly: true,
            voices: 8,
            entropy: 0.3,
            spread: 0.5,
            bar: BarScene {
                name: "Undertow Growl".into(),
                found: true,
                changed: true,
                favorite: true,
                open: true,
                below: true,
                hover: None,
            },
            hover: None,
        });
        strip.frame().save_png(dir.join("strip.png")).unwrap();
        let mut d = DrawerRenderer::new(0.4);
        let names = [
            "Bass",
            "Brass Tutti",
            "Breath Flute",
            "Cruising Whistle",
            "Deep Bass",
            "Elastic Octaves",
            "Hollow Glider",
            "Lead",
            "Pulse Strut",
            "Ringing Saw Line",
            "Shoreline Wash",
        ];
        d.render(&DrawerScene {
            search: Field::new("bass"),
            favourites: false,
            mine: true,
            chips: [
                "bass", "brass", "dark", "feedback", "funk", "glide", "lead", "prog",
            ]
            .iter()
            .map(|t| ((*t).to_owned(), *t == "bass"))
            .collect(),
            rows: names
                .iter()
                .enumerate()
                .map(|(i, n)| Row {
                    name: (*n).to_owned(),
                    tags: "bass · dark · feedback".into(),
                    origin: match i {
                        4 => Some("YOURS"),
                        7 => Some("EDITED"),
                        _ => None,
                    },
                    favorite: i % 4 == 0,
                    current: i == 4,
                })
                .collect(),
            first: 0,
            editing: Some((7, Edit::Delete { factory: true })),
            edit: Field::default(),
            save_name: Field::new("Deep Bass"),
            save_tags: Field::new("bass, midnight"),
            replace: true,
            hint: "Replaces your Deep Bass.".into(),
            focus: Some(FieldId::SaveName),
            hover: Some(DrawerTarget::Row(2)),
            update: UpdateScene {
                text: "0.2.0 IS OUT (THIS IS 0.1.0)".into(),
                tone: Tone::News,
                button: "DOWNLOAD".into(),
            },
        });
        d.frame().save_png(dir.join("drawer.png")).unwrap();
    }
}

/// The drawer's frame laid over the panel's (`dest`), its top `top` pixels down; what falls
/// below the panel is cut (it slides from the panel's foot).
pub fn overlay(dest: &mut Pixmap, drawer: &Pixmap, top: i32) {
    dest.draw_pixmap(
        0,
        top,
        drawer.as_ref(),
        &resvg::tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

/// Where a bar control's middle is (bar units).
pub fn bar_centre(t: BarTarget) -> (f64, f64) {
    let x = match t {
        BarTarget::Star => STAR_X + STEP_W / 2.0,
        BarTarget::Prev => PREV_X + STEP_W / 2.0,
        BarTarget::Name => (NAME_X + NAME_END) / 2.0,
        BarTarget::Next => NEXT_X + STEP_W / 2.0,
        BarTarget::Save => SAVE_X + SAVE_W / 2.0,
    };
    (x, BAR_MID)
}

/// Where a field's middle is (drawer units; a row's field in its row `y`).
pub fn field_centre(id: FieldId, row: Option<f64>) -> (f64, f64) {
    let (x0, x1, y, h) = field_box(id, row);
    ((x0 + x1) / 2.0, y + h / 2.0)
}

/// Whether the update check's `scene` is drawn whole: its text not shortened, its button's
/// label within the button.
pub fn update_fits(fonts: &Fonts, scene: &UpdateScene) -> bool {
    let w = |t: &str, size| fonts.advance(t, size, Weight::Regular, 1.0);
    w(&scene.text, SMALL) <= UPDATE_TEXT_END - (MINE_X + MINE_W + 30.0)
        && w(&scene.button, UPDATE_LABEL) <= UPDATE_W - 40.0
}

/// Where the update check's button's middle is (drawer units).
pub fn update_centre() -> (f64, f64) {
    (UPDATE_X + UPDATE_W / 2.0, TOOLS_Y + TOOLS_H / 2.0)
}

/// Where row `i`'s middle is, if it is shown (drawer units).
pub fn row_centre(s: &DrawerScene, i: usize) -> Option<(f64, f64)> {
    row_y(s, i).map(|y| (ROW_TAGS_X - 100.0, y + ROW_H / 2.0))
}

/// Where row `i`'s button doing `a` is, if the row shows it (drawer units).
pub fn action_centre(s: &DrawerScene, i: usize, a: RowAction) -> Option<(f64, f64)> {
    let y = row_y(s, i)?;
    let editing = s.editing.filter(|(r, _)| *r == i).map(|(_, e)| e);
    let k = actions(s.rows.get(i)?, editing)
        .iter()
        .position(|(b, _)| *b == a)?;
    Some((action_x(k) + ACTION_W / 2.0, y + ROW_H / 2.0))
}
