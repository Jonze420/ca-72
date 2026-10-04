//! The lettering's typeface, URW Gothic (AGPL-3.0 with a font exception; decisions.md R22),
//! built in, and its
//! measures: what a browser's `getComputedTextLength` and `getBBox` give for a text.

use std::fmt;
use std::sync::Arc;

use resvg::usvg::fontdb;

/// The typeface's family name.
pub const FAMILY: &str = "URW Gothic";

static REGULAR: &[u8] = include_bytes!("../../../third_party/urw-gothic/URWGothic-Book.otf");
/// Its bold: URW Gothic's Demi.
static BOLD: &[u8] = include_bytes!("../../../third_party/urw-gothic/URWGothic-Demi.otf");

/// A weight of the typeface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

/// The typeface, for rendering (its font database) and for measuring.
pub struct Fonts {
    db: Arc<fontdb::Database>,
    regular: rustybuzz::Face<'static>,
    bold: rustybuzz::Face<'static>,
}

impl fmt::Debug for Fonts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fonts")
            .field("faces", &self.db.len())
            .finish()
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(REGULAR.to_vec());
        db.load_font_data(BOLD.to_vec());
        db.set_sans_serif_family(FAMILY);
        let face = |data: &'static [u8]| {
            rustybuzz::Face::from_slice(data, 0).expect("the built-in typeface parses")
        };
        Self {
            db: Arc::new(db),
            regular: face(REGULAR),
            bold: face(BOLD),
        }
    }

    /// The font database rendering uses.
    pub fn database(&self) -> Arc<fontdb::Database> {
        Arc::clone(&self.db)
    }

    fn face(&self, weight: Weight) -> &rustybuzz::Face<'static> {
        match weight {
            Weight::Regular => &self.regular,
            Weight::Bold => &self.bold,
        }
    }

    /// The advance of `s` set at `size` (shaped and kerned), with `spacing` after each
    /// character: a browser's `getComputedTextLength`.
    pub fn advance(&self, s: &str, size: f64, weight: Weight, spacing: f64) -> f64 {
        let face = self.face(weight);
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(s);
        let shaped = rustybuzz::shape(face, &[], buffer);
        let units: i32 = shaped.glyph_positions().iter().map(|p| p.x_advance).sum();
        f64::from(units) * size / f64::from(face.units_per_em())
            + spacing * s.chars().count() as f64
    }

    /// How far the typeface reaches above its baseline and below it, in ems: the height a
    /// browser gives a text's box.
    pub fn extent(&self, weight: Weight) -> (f64, f64) {
        let face = self.face(weight);
        let em = f64::from(face.units_per_em());
        (
            f64::from(face.ascender()) / em,
            -f64::from(face.descender()) / em,
        )
    }
}
