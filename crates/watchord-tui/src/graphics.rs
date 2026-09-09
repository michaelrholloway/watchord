//! Pixel drawings for the packed skin (ADR-0006, ticket #8).
//!
//! A terminal has one type size, so the headline cannot be larger text —
//! unless it is a picture. Terminals that speak the kitty graphics protocol
//! (Ghostty, kitty, WezTerm), sixel or iTerm2 can place a picture in the cell
//! grid. This module draws two: the headline in JetBrains Mono Bold at a
//! larger size, and the staff at pixel resolution, where a line is a line and
//! a note sits exactly on it or exactly between two. `ratatui-image` detects
//! the protocol and places the picture; a terminal with none of them gets the
//! skin's text drawings instead, and nothing here runs.
//!
//! The font files are JetBrains Mono, SIL Open Font License 1.1
//! (`assets/OFL.txt`), embedded so the picture matches the terminal's own
//! face wherever it is drawn.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{DynamicImage, Rgba, RgbaImage};
use ratatui::layout::{Rect, Size};
use ratatui_image::Resize;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use watchord_theory::staff::{Clef, StaffNote};

const BOLD: &[u8] = include_bytes!("../assets/JetBrainsMono-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../assets/JetBrainsMono-Italic.ttf");

const GROUND: Rgba<u8> = Rgba([0, 0, 0, 255]);
const INK: Rgba<u8> = Rgba([255, 255, 255, 255]);
const LIME: Rgba<u8> = Rgba([0xC6, 0xF0, 0x00, 255]);

/// The headline's type size as `ab_glyph` measures it — the face's full
/// height, ascent to descent — as a multiple of the cell height. The design
/// sets a 22px headline on a 20px cell; JetBrains Mono's height is 1.32 em,
/// so 1.1 cells of em is 1.45 cells of height.
const HEADLINE_SCALE: f32 = 1.45;

/// A drawing kept until its inputs change.
struct Cached {
    key: String,
    protocol: Option<Protocol>,
}

impl Cached {
    fn empty() -> Self {
        Cached {
            key: String::new(),
            protocol: None,
        }
    }
}

/// The terminal's picture support, the cell size in pixels, and the two
/// drawings the packed skin places.
pub struct Graphics {
    picker: Picker,
    headline: Cached,
    staff: Cached,
}

impl Graphics {
    /// Asks the terminal what it supports. `None` when it answers with
    /// nothing better than half-blocks, or does not answer. Call before the
    /// alternate screen is entered: the query reads the terminal's reply
    /// from stdin.
    pub fn detect() -> Option<Self> {
        let picker = Picker::from_query_stdio().ok()?;
        if picker.protocol_type() == ProtocolType::Halfblocks {
            return None;
        }
        Some(Graphics {
            picker,
            headline: Cached::empty(),
            staff: Cached::empty(),
        })
    }

    /// A cell's size in pixels, `(width, height)`.
    pub fn cell(&self) -> (u16, u16) {
        let size = self.picker.font_size();
        (size.width, size.height)
    }

    /// The headline drawn large, to fill `area`. `None` when the area is
    /// empty or the picture could not be placed.
    pub fn headline(&mut self, text: &str, area: Rect) -> Option<&Protocol> {
        let key = format!("{text}|{}x{}", area.width, area.height);
        if self.headline.key != key {
            let (cw, ch) = self.cell();
            let image = draw_headline(
                text,
                area.width as u32 * cw as u32,
                area.height as u32 * ch as u32,
                ch,
            );
            self.headline = Cached {
                key,
                protocol: self.place(image, area),
            };
        }
        self.headline.protocol.as_ref()
    }

    /// The staff drawn at pixel resolution, to fill `area`.
    pub fn staff(&mut self, notes: &[StaffNote], area: Rect) -> Option<&Protocol> {
        let mut key = format!("{}x{}", area.width, area.height);
        for note in notes {
            key.push_str(&format!("|{}:{:?}:{}", note.midi_note, note.clef, note.row));
        }
        if self.staff.key != key {
            let (cw, ch) = self.cell();
            let image = draw_staff(
                notes,
                area.width as u32 * cw as u32,
                area.height as u32 * ch as u32,
                ch,
            );
            self.staff = Cached {
                key,
                protocol: self.place(image, area),
            };
        }
        self.staff.protocol.as_ref()
    }

    fn place(&self, image: RgbaImage, area: Rect) -> Option<Protocol> {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        self.picker
            .new_protocol(
                DynamicImage::ImageRgba8(image),
                Size::new(area.width, area.height),
                Resize::Fit(None),
            )
            .ok()
    }
}

// MARK: - The headline

/// `text` in JetBrains Mono Bold, lime on black, centred in a `width`×`height`
/// picture. The type size follows the cell height; a long name shrinks to fit
/// the width.
pub fn draw_headline(text: &str, width: u32, height: u32, cell_height: u16) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width.max(1), height.max(1), GROUND);
    let font = FontRef::try_from_slice(BOLD).expect("the embedded bold face parses");
    let mut px = cell_height as f32 * HEADLINE_SCALE;
    let mut advance = text_width(&font, text, px);
    if advance > width as f32 - 2.0 {
        px *= (width as f32 - 2.0) / advance;
        advance = text_width(&font, text, px);
    }
    let scaled = font.as_scaled(PxScale::from(px));
    // Centre the cap height, not the whole ascent-to-descent box: the
    // notation has no descenders, so centring the box would sit it high.
    let cap = scaled.ascent() * 0.72;
    let baseline = (height as f32 + cap) / 2.0;
    let x = (width as f32 - advance) / 2.0;
    draw_text(&mut image, &font, text, px, x, baseline, LIME);
    image
}

fn text_width(font: &FontRef, text: &str, px: f32) -> f32 {
    let scaled = font.as_scaled(PxScale::from(px));
    text.chars()
        .map(|c| scaled.h_advance(scaled.glyph_id(c)))
        .sum()
}

/// Draws `text` with its baseline at `y`, starting at `x`, blending coverage
/// into `colour`.
fn draw_text(
    image: &mut RgbaImage,
    font: &FontRef,
    text: &str,
    px: f32,
    x: f32,
    y: f32,
    colour: Rgba<u8>,
) {
    let scaled = font.as_scaled(PxScale::from(px));
    let mut cursor = x;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        let glyph = id.with_scale_and_position(PxScale::from(px), ab_glyph::point(cursor, y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px_x = bounds.min.x as i32 + gx as i32;
                let px_y = bounds.min.y as i32 + gy as i32;
                blend(image, px_x, px_y, colour, coverage);
            });
        }
        cursor += scaled.h_advance(id);
    }
}

fn blend(image: &mut RgbaImage, x: i32, y: i32, colour: Rgba<u8>, coverage: f32) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 || coverage <= 0.0
    {
        return;
    }
    let coverage = coverage.min(1.0);
    let under = *image.get_pixel(x as u32, y as u32);
    let mix = |a: u8, b: u8| (a as f32 * (1.0 - coverage) + b as f32 * coverage).round() as u8;
    image.put_pixel(
        x as u32,
        y as u32,
        Rgba([
            mix(under[0], colour[0]),
            mix(under[1], colour[1]),
            mix(under[2], colour[2]),
            255,
        ]),
    );
}

fn fill(image: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, colour: Rgba<u8>) {
    for py in y.max(0)..(y + h).min(image.height() as i32) {
        for px in x.max(0)..(x + w).min(image.width() as i32) {
            image.put_pixel(px as u32, py as u32, colour);
        }
    }
}

// MARK: - The staff

/// The staff's proportions, in units of the line gap `L` (the distance from
/// one staff line to the next). The design has L = 10px on a 20px cell: heads
/// 6px square, lines 48px wide, 32px between the staves, sharps in 8px italic.
struct Proportions {
    /// Pixels from one line to the next.
    gap: f32,
}

impl Proportions {
    fn pitch(&self) -> f32 {
        self.gap / 2.0
    }
    fn head(&self) -> f32 {
        (self.gap * 0.6).max(2.0)
    }
    fn line_width(&self) -> f32 {
        self.gap * 4.8
    }
    fn stroke(&self) -> i32 {
        (self.gap / 10.0).round().max(1.0) as i32
    }
    fn between_staves(&self) -> f32 {
        self.gap * 3.2
    }
    /// The design's 8px italic sharp on a 10px gap, as `ab_glyph` height.
    fn sharp_px(&self) -> f32 {
        self.gap * 1.05
    }
}

/// One clef's vertical extent in positions: the staff 0..=8, widened to the
/// farthest note.
fn extent(notes: &[&StaffNote]) -> (i32, i32) {
    let top = notes.iter().map(|n| n.row).max().unwrap_or(8).max(8);
    let bottom = notes.iter().map(|n| n.row).min().unwrap_or(0).min(0);
    (bottom, top)
}

/// Both clefs, treble over bass, drawn to fill `width`×`height` at the cell
/// height's proportions — smaller if the notes' span does not fit.
pub fn draw_staff(notes: &[StaffNote], width: u32, height: u32, cell_height: u16) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width.max(1), height.max(1), GROUND);
    let treble: Vec<&StaffNote> = notes.iter().filter(|n| n.clef == Clef::Treble).collect();
    let bass: Vec<&StaffNote> = notes.iter().filter(|n| n.clef == Clef::Bass).collect();
    let (t_bottom, t_top) = extent(&treble);
    let (b_bottom, b_top) = extent(&bass);
    // Height in half-gaps (positions): each clef's span, the gap between, and
    // a head's worth of margin top and bottom.
    let positions = (t_top - t_bottom) as f32 + (b_top - b_bottom) as f32;
    let mut proportions = Proportions {
        gap: cell_height as f32 / 2.0,
    };
    let needed = |p: &Proportions| positions * p.pitch() + p.between_staves() + 2.0 * p.head();
    if needed(&proportions) > height as f32 {
        proportions.gap *= height as f32 / needed(&proportions);
    }
    let p = proportions;
    let block = needed(&p);
    let top_margin = (height as f32 - block) / 2.0 + p.head();
    let centre_x = width as f32 / 2.0;

    // Treble: position t_top at the top margin, one pitch per position down.
    let treble_y = |pos: i32| top_margin + (t_top - pos) as f32 * p.pitch();
    let bass_top = treble_y(t_bottom) + p.between_staves();
    let bass_y = |pos: i32| bass_top + (b_top - pos) as f32 * p.pitch();

    let italic = FontRef::try_from_slice(ITALIC).expect("the embedded italic face parses");
    draw_clef(
        &mut image,
        &treble,
        (t_bottom, t_top),
        &treble_y,
        centre_x,
        &p,
        &italic,
    );
    draw_clef(
        &mut image,
        &bass,
        (b_bottom, b_top),
        &bass_y,
        centre_x,
        &p,
        &italic,
    );
    image
}

#[allow(clippy::too_many_arguments)]
fn draw_clef(
    image: &mut RgbaImage,
    notes: &[&StaffNote],
    (bottom, top): (i32, i32),
    y_of: &dyn Fn(i32) -> f32,
    centre_x: f32,
    p: &Proportions,
    italic: &FontRef,
) {
    let stroke = p.stroke();
    let half_stroke = stroke as f32 / 2.0;
    // Lines on even positions: the staff's five in full, and a short ledger
    // line on every even position out to the farthest note.
    let mut pos = bottom;
    while pos <= top {
        if pos % 2 == 0 {
            let y = (y_of(pos) - half_stroke).round() as i32;
            if (0..=8).contains(&pos) {
                let w = p.line_width();
                fill(
                    image,
                    (centre_x - w / 2.0).round() as i32,
                    y,
                    w.round() as i32,
                    stroke,
                    INK,
                );
            } else {
                let reaches = if pos < 0 {
                    notes.iter().any(|n| n.row <= pos)
                } else {
                    notes.iter().any(|n| n.row >= pos)
                };
                if reaches {
                    let w = p.head() * 3.0;
                    fill(
                        image,
                        (centre_x - w / 2.0).round() as i32,
                        y,
                        w.round() as i32,
                        stroke,
                        INK,
                    );
                }
            }
        }
        pos += 1;
    }
    // Heads, lowest first; a second head on the same position sits to the
    // right by a head and a gap. Sharps sit to the left, centred on the head.
    let mut positions: Vec<i32> = notes.iter().map(|n| n.row).collect();
    positions.sort_unstable();
    positions.dedup();
    let head = p.head();
    for row in positions {
        let heads: Vec<&&StaffNote> = notes.iter().filter(|n| n.row == row).collect();
        let y = y_of(row);
        for (index, note) in heads.iter().enumerate() {
            let x = centre_x - head / 2.0 + index as f32 * (head + head * 0.5);
            fill(
                image,
                x.round() as i32,
                (y - head / 2.0).round() as i32,
                head.round() as i32,
                head.round() as i32,
                INK,
            );
            let accidental = note.spelled.accidental.symbol();
            if !accidental.is_empty() {
                let px = p.sharp_px();
                let scaled = italic.as_scaled(PxScale::from(px));
                let advance = scaled.h_advance(scaled.glyph_id(accidental.chars().next().unwrap()));
                // The glyph's cap height, centred on the head.
                let cap = scaled.ascent() * 0.72;
                let baseline = y + cap / 2.0;
                draw_text(
                    image,
                    italic,
                    accidental,
                    px,
                    x - advance - head * 0.3,
                    baseline,
                    INK,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watchord_theory::staff::position_of;

    /// Writes `image` as a binary PPM so a person can look at it: set
    /// `WATCHORD_DUMP_PICTURES=<dir>` and run the tests.
    fn dump(name: &str, image: &RgbaImage) {
        let Some(dir) = std::env::var_os("WATCHORD_DUMP_PICTURES") else {
            return;
        };
        let mut out = format!("P6\n{} {}\n255\n", image.width(), image.height()).into_bytes();
        for px in image.pixels() {
            out.extend_from_slice(&px.0[..3]);
        }
        std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), out)
            .expect("write the picture");
    }

    #[test]
    fn dump_the_pictures_for_a_person_to_look_at() {
        // The Hendrix voicing: C1 E2 A#2 D#3 G3, both clefs, ledger notes
        // both ends, two sharps.
        let notes: Vec<StaffNote> = [36, 52, 58, 63, 67]
            .iter()
            .map(|&m| position_of(m, false))
            .collect();
        // A retina Ghostty cell is about 18×40 px; a 24×20 staff box.
        dump(
            "staff-24x20-cell18x40",
            &draw_staff(&notes, 24 * 18, 20 * 40, 40),
        );
        dump(
            "staff-24x11-cell18x40",
            &draw_staff(&notes, 24 * 18, 11 * 40, 40),
        );
        dump("headline-c7s9", &draw_headline("C7#9", 24 * 18, 2 * 40, 40));
        dump(
            "headline-long",
            &draw_headline("C#Δ7#11/G", 24 * 18, 2 * 40, 40),
        );
        dump(
            "headline-approx",
            &draw_headline("≈ Cm7/E", 24 * 18, 2 * 40, 40),
        );
    }

    fn is_ink(px: &Rgba<u8>) -> bool {
        px[0] > 128 && px[1] > 128 && px[2] > 128
    }

    /// Rows of the picture that hold a long white run: the staff lines.
    fn line_rows(image: &RgbaImage, min_run: u32) -> Vec<u32> {
        (0..image.height())
            .filter(|&y| {
                let run = (0..image.width())
                    .filter(|&x| is_ink(image.get_pixel(x, y)))
                    .count() as u32;
                run >= min_run
            })
            .collect()
    }

    #[test]
    fn the_staff_lines_are_evenly_spaced_half_a_cell_apart() {
        // A C major triad in the treble: E3 G3 C4 — no ledger notes, so each
        // clef is exactly five lines.
        let notes: Vec<StaffNote> = [64, 67, 72]
            .iter()
            .map(|&m| position_of(m, false))
            .collect();
        let image = draw_staff(&notes, 24 * 10, 20 * 20, 20);
        let rows = line_rows(&image, 40);
        // Ten line rows (stroke 1px at a 10px gap): five treble, five bass.
        assert_eq!(rows.len(), 10, "{rows:?}");
        let gaps: Vec<u32> = rows.windows(2).map(|w| w[1] - w[0]).collect();
        // Nine gaps: four of 10px, one of 32px between the staves, four of 10px.
        assert_eq!(gaps[..4], [10, 10, 10, 10], "{gaps:?}");
        assert_eq!(gaps[4], 32, "{gaps:?}");
        assert_eq!(gaps[5..], [10, 10, 10, 10], "{gaps:?}");
    }

    #[test]
    fn a_note_on_a_line_is_centred_on_it_and_a_space_note_between_two() {
        // G3 (67) is the treble's second line, position 2. A3 (69) is the
        // space above it, position 3.
        let g = vec![position_of(67, false)];
        let image = draw_staff(&g, 240, 400, 20);
        let lines = line_rows(&image, 40);
        let line_y = lines[3]; // bottom line is index 4; G is one line up
        // The head is a 6px square: rows line_y-3 .. line_y+3 hold ink at centre.
        let cx = image.width() / 2;
        let head_rows: Vec<u32> = (0..image.height())
            .filter(|&y| is_ink(image.get_pixel(cx, y)) && is_ink(image.get_pixel(cx - 2, y)))
            .filter(|&y| {
                // a head row has ink at the centre but the row is not a full line
                (0..image.width())
                    .filter(|&x| is_ink(image.get_pixel(x, y)))
                    .count()
                    < 40
            })
            .collect();
        assert!(!head_rows.is_empty());
        let head_centre = (head_rows[0] + head_rows[head_rows.len() - 1]) as f32 / 2.0;
        assert!(
            (head_centre - line_y as f32).abs() <= 1.0,
            "head at {head_centre}, line at {line_y}"
        );

        let a = vec![position_of(69, false)];
        let image = draw_staff(&a, 240, 400, 20);
        let lines = line_rows(&image, 40);
        let between = (lines[2] + lines[3]) as f32 / 2.0;
        let head_rows: Vec<u32> = (0..image.height())
            .filter(|&y| is_ink(image.get_pixel(cx, y)))
            .filter(|&y| {
                (0..image.width())
                    .filter(|&x| is_ink(image.get_pixel(x, y)))
                    .count()
                    < 40
            })
            .collect();
        let head_centre = (head_rows[0] + head_rows[head_rows.len() - 1]) as f32 / 2.0;
        assert!(
            (head_centre - between).abs() <= 1.0,
            "head at {head_centre}, space at {between}"
        );
    }

    #[test]
    fn a_sharp_sits_left_of_its_head_at_the_same_height() {
        let notes = vec![position_of(68, false)]; // G#3: line 2, sharp
        let image = draw_staff(&notes, 240, 400, 20);
        let cx = image.width() / 2;
        // Ink to the left of the head, off the staff lines, is the sharp.
        let lines = line_rows(&image, 40);
        let sharp: Vec<(u32, u32)> = (0..image.height())
            .filter(|y| !lines.contains(y))
            .flat_map(|y| {
                (0..cx - 4)
                    .filter(move |&x| x < cx - 4)
                    .map(move |x| (x, y))
            })
            .filter(|&(x, y)| is_ink(image.get_pixel(x, y)))
            .collect();
        assert!(!sharp.is_empty(), "a sharp is drawn");
        let ys: Vec<u32> = sharp.iter().map(|&(_, y)| y).collect();
        let sharp_centre = (*ys.iter().min().unwrap() + *ys.iter().max().unwrap()) as f32 / 2.0;
        let line_y = lines[3] as f32;
        assert!(
            (sharp_centre - line_y).abs() <= 2.0,
            "sharp at {sharp_centre}, head at {line_y}"
        );
    }

    #[test]
    fn the_headline_is_lime_and_fills_about_two_rows() {
        let image = draw_headline("C7#9", 24 * 10, 3 * 20, 20);
        let lime_rows: Vec<u32> = (0..image.height())
            .filter(|&y| {
                (0..image.width())
                    .any(|x| image.get_pixel(x, y)[1] > 200 && image.get_pixel(x, y)[2] < 100)
            })
            .collect();
        let tall = lime_rows.len();
        assert!(
            (14..=26).contains(&tall),
            "cap height {tall}px on a 20px cell"
        );
        // A long name shrinks to fit rather than clipping.
        let image = draw_headline("C#Δ7#11/G", 24 * 10, 60, 20);
        assert!(image.get_pixel(0, 30)[1] < 200 && image.get_pixel(image.width() - 1, 30)[1] < 200);
    }
}
