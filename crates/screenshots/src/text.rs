//! Text with real fonts: line widths from the chosen font's own advances,
//! so wrapping in the editor and in the export match what gets drawn, for
//! any typeface. The fonts are the system's, the same ones the canvas and
//! resvg draw with.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use resvg::usvg::fontdb::{self, Database, Family, Query};

/// The typeface text layers use unless they name another.
pub const DEFAULT_FONT: &str = "Helvetica Neue";
/// Tried in order when a layer's font isn't installed: macOS, then Windows.
const FALLBACKS: &[&str] = &["Helvetica Neue", "Segoe UI", "Arial"];

/// Typefaces that suit screenshot captions and ship with macOS or Windows.
/// The picker shows the installed ones first.
pub const SUGGESTED_FONTS: &[&str] = &[
    "Helvetica Neue",
    "Avenir Next",
    "Futura",
    "Gill Sans",
    "Optima",
    "Arial Rounded MT Bold",
    "Trebuchet MS",
    "Verdana",
    "Georgia",
    "Didot",
    "Baskerville",
    "Rockwell",
    "American Typewriter",
    "Menlo",
    "Marker Felt",
    "Chalkboard SE",
    "Noteworthy",
    "Impact",
    "Segoe UI",
    "Bahnschrift",
];

/// System fonts, loaded once: scanning them takes a moment, so call
/// [`warm_fonts`] early on a background thread.
pub(crate) fn fonts() -> Arc<Database> {
    static FONTS: OnceLock<Arc<Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// Loads the system fonts now, so the first text edit doesn't wait.
pub fn warm_fonts() {
    fonts();
}

/// Installed font families, sorted, without the hidden system ones.
pub fn font_families() -> Vec<String> {
    let db = fonts();
    let mut families: Vec<String> = db
        .faces()
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .filter(|name| !name.starts_with('.') && !name.is_empty())
        .collect();
    families.sort_by_key(|name| name.to_lowercase());
    families.dedup();
    families
}

pub fn has_font(family: &str) -> bool {
    fonts()
        .faces()
        .any(|face| face.families.iter().any(|(name, _)| name.eq_ignore_ascii_case(family)))
}

/// How a text layer is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style<'a> {
    pub family: Option<&'a str>,
    pub weight: u16,
    pub italic: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    family: Option<String>,
    weight: u16,
    italic: bool,
}

struct Face {
    id: Option<fontdb::ID>,
    /// Ascent and descent in em.
    metrics: (f32, f32),
    /// Advance widths in em, filled in as characters are seen.
    advances: HashMap<char, f32>,
}

fn faces() -> &'static Mutex<HashMap<Key, Face>> {
    static FACES: OnceLock<Mutex<HashMap<Key, Face>>> = OnceLock::new();
    FACES.get_or_init(Default::default)
}

fn query(db: &Database, style: Style) -> Option<fontdb::ID> {
    let mut families: Vec<Family> = style.family.map(Family::Name).into_iter().collect();
    families.extend(FALLBACKS.iter().map(|name| Family::Name(name)));
    families.push(Family::SansSerif);
    db.query(&Query {
        families: &families,
        weight: fontdb::Weight(style.weight),
        stretch: fontdb::Stretch::Normal,
        style: if style.italic {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        },
    })
}

/// Runs `f` on the cached face for `style`, after measuring any of
/// `chars` it hasn't seen.
fn with_face<T>(style: Style, chars: impl Iterator<Item = char>, f: impl FnOnce(&Face) -> T) -> T {
    let db = fonts();
    let mut cache = faces().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let key = Key {
        family: style.family.map(str::to_owned),
        weight: style.weight,
        italic: style.italic,
    };
    let face = cache.entry(key).or_insert_with(|| {
        let id = query(&db, style);
        let metrics = id
            .and_then(|id| {
                db.with_face_data(id, |data, index| {
                    let face = ttf_parser::Face::parse(data, index).ok()?;
                    let em = face.units_per_em() as f32;
                    Some((face.ascender() as f32 / em, -(face.descender() as f32) / em))
                })
                .flatten()
            })
            .unwrap_or((0.8, 0.2));
        Face {
            id,
            metrics,
            advances: HashMap::new(),
        }
    });
    let missing: Vec<char> = chars.filter(|c| !face.advances.contains_key(c)).collect();
    if !missing.is_empty() {
        let measured = face.id.and_then(|id| {
            db.with_face_data(id, |data, index| {
                let parsed = ttf_parser::Face::parse(data, index).ok()?;
                let em = parsed.units_per_em() as f32;
                Some(
                    missing
                        .iter()
                        .map(|c| {
                            let advance = parsed
                                .glyph_index(*c)
                                .and_then(|glyph| parsed.glyph_hor_advance(glyph))
                                .map(|advance| advance as f32 / em);
                            (*c, advance)
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
        });
        for c in &missing {
            // Characters the font lacks are drawn by a fallback font; the
            // estimate stands in for its width.
            let advance = measured
                .as_ref()
                .and_then(|all| all.iter().find(|(m, _)| m == c).and_then(|(_, a)| *a))
                .unwrap_or_else(|| estimate(*c));
            face.advances.insert(*c, advance);
        }
    }
    f(face)
}

/// A rough width in em, for when no font can be read.
fn estimate(c: char) -> f32 {
    if c.is_ascii() {
        0.54
    } else if c.len_utf8() >= 3 {
        1.0
    } else {
        0.6
    }
}

/// The width of `text` in em.
pub fn measure(text: &str, style: Style) -> f32 {
    with_face(style, text.chars(), |face| {
        text.chars().map(|c| face.advances.get(&c).copied().unwrap_or(0.)).sum()
    })
}

/// The font's ascent and descent in em, for placing baselines.
pub fn vertical_metrics(style: Style) -> (f32, f32) {
    with_face(style, std::iter::empty(), |face| face.metrics)
}

/// A word of a text layer, for drawing it in its own colour or revealing
/// words one by one.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    /// Marked `*like this*` in the text: drawn in the layer's accent colour.
    pub accent: bool,
    /// Its place in the whole text, counting from 0.
    pub index: usize,
}

/// The text without its `*accent*` marks, as it is measured and drawn.
pub fn strip_marks(text: &str) -> String {
    text.replace('*', "")
}

/// `lines` (as wrapped from the unmarked text) split back into words, each
/// knowing whether `marked` set it in the accent colour. Wrapping keeps the
/// words in order, so they line up one for one.
pub fn words_by_line(marked: &str, lines: &[String]) -> Vec<Vec<Word>> {
    let mut accent = false;
    let mut tokens = marked.split_whitespace().map(|token| {
        let starts = token.starts_with('*');
        if starts {
            accent = true;
        }
        let word = Word {
            text: token.replace('*', ""),
            accent,
            index: 0,
        };
        // A closing mark ends the run: "*better*", or "*two words*".
        if token.ends_with('*') && (!starts || token.matches('*').count() >= 2) {
            accent = false;
        }
        word
    });
    let mut index = 0;
    lines
        .iter()
        .map(|line| {
            (0..line.split_whitespace().count())
                .filter_map(|_| tokens.next())
                .filter(|word| !word.text.is_empty())
                .map(|mut word| {
                    word.index = index;
                    index += 1;
                    word
                })
                .collect()
        })
        .collect()
}

/// Greedy word wrap with the font's real widths. Explicit newlines always
/// break; a word too long for a line gets a line of its own.
pub fn wrap_styled(text: &str, style: Style, font_size: f32, max_width: f32) -> Vec<String> {
    lay_out(text, max_width, |line| measure(line, style) * font_size)
}

/// Greedy word wrap by estimated glyph width (about half the font size for
/// Latin letters, a full em for CJK), for when no font is involved.
pub fn wrap(text: &str, font_size: f32, max_width: f32) -> Vec<String> {
    lay_out(text, max_width, |line| line.chars().map(estimate).sum::<f32>() * font_size)
}

fn lay_out(text: &str, max_width: f32, width: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if width(&candidate) > max_width && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}
