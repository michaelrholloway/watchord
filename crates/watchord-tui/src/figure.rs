//! The headline figure: the headline drawn large (ADR-0003 (e)).
//!
//! A hand-made glyph set, five rows tall, covering the notation alphabet and
//! nothing else — `A`–`G`, `#`, `b`, the digits, `Δ`, `m`, `+`, `-`, the
//! letters of `sus`, `add`, `dim` and `no`, `/`, `≈`, `·`, and space. Each
//! glyph is a bitmap; the tall figure paints a set pixel as `█`, and the short
//! figure folds the same bitmap into two rows of half-blocks, so both are one
//! drawing of the same text at two sizes.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// One glyph: five rows of `#` (ink) and `.` (ground), all the same width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub rows: [&'static str; 5],
}

/// Rows the tall figure takes.
pub const TALL_HEIGHT: u16 = 5;
/// Rows the short figure takes.
pub const SHORT_HEIGHT: u16 = 2;
/// Columns between glyphs.
const GAP: usize = 1;

const fn g(rows: [&'static str; 5]) -> Glyph {
    Glyph { rows }
}

/// The glyph for one character of the alphabet, or `None` outside it.
pub fn glyph(c: char) -> Option<Glyph> {
    let glyph = match c {
        'A' => g([".###.", "#...#", "#####", "#...#", "#...#"]),
        'B' => g(["####.", "#...#", "####.", "#...#", "####."]),
        'C' => g([".####", "#....", "#....", "#....", ".####"]),
        'D' => g(["####.", "#...#", "#...#", "#...#", "####."]),
        'E' => g(["#####", "#....", "####.", "#....", "#####"]),
        'F' => g(["#####", "#....", "####.", "#....", "#...."]),
        'G' => g([".####", "#....", "#..##", "#...#", ".####"]),
        '#' => g([".#.#.", "#####", ".#.#.", "#####", ".#.#."]),
        'b' => g(["#...", "#...", "###.", "#..#", "###."]),
        '0' => g([".##.", "#..#", "#..#", "#..#", ".##."]),
        '1' => g([".#.", "##.", ".#.", ".#.", "###"]),
        '2' => g(["###.", "...#", ".##.", "#...", "####"]),
        '3' => g(["###.", "...#", ".##.", "...#", "###."]),
        '4' => g(["#..#", "#..#", "####", "...#", "...#"]),
        '5' => g(["####", "#...", "###.", "...#", "###."]),
        '6' => g([".##.", "#...", "###.", "#..#", ".##."]),
        '7' => g(["####", "...#", "..#.", ".#..", ".#.."]),
        '8' => g([".##.", "#..#", ".##.", "#..#", ".##."]),
        '9' => g([".##.", "#..#", ".###", "...#", ".##."]),
        'Δ' => g(["..#..", "..#..", ".#.#.", ".#.#.", "#####"]),
        'm' => g([".....", "##.#.", "#.#.#", "#.#.#", "#.#.#"]),
        '+' => g(["...", ".#.", "###", ".#.", "..."]),
        '-' => g(["...", "...", "###", "...", "..."]),
        's' => g(["....", ".###", "##..", "..##", "###."]),
        'u' => g(["....", "#..#", "#..#", "#..#", ".###"]),
        'a' => g(["....", "###.", "...#", ".###", "#..#"]),
        'd' => g(["...#", "...#", ".###", "#..#", ".###"]),
        'i' => g(["#", ".", "#", "#", "#"]),
        'n' => g(["....", "###.", "#..#", "#..#", "#..#"]),
        'o' => g(["....", ".##.", "#..#", "#..#", ".##."]),
        '/' => g(["....#", "...#.", "..#..", ".#...", "#...."]),
        '≈' => g([".....", ".##.#", "#..#.", ".##.#", "#..#."]),
        '·' => g([".", ".", "#", ".", "."]),
        ' ' => g(["..", "..", "..", "..", ".."]),
        _ => return None,
    };
    Some(glyph)
}

/// The characters of `text` that have no glyph.
pub fn missing(text: &str) -> Vec<char> {
    text.chars().filter(|&c| glyph(c).is_none()).collect()
}

/// Columns the figure of `text` takes, at either height. A character outside
/// the alphabet counts as a space.
pub fn width(text: &str) -> usize {
    let glyphs: Vec<Glyph> = text.chars().map(glyph_or_space).collect();
    let ink: usize = glyphs.iter().map(|g| g.rows[0].len()).sum();
    ink + GAP * glyphs.len().saturating_sub(1)
}

fn glyph_or_space(c: char) -> Glyph {
    glyph(c).unwrap_or_else(|| glyph(' ').expect("space is in the alphabet"))
}

/// The bitmap of the whole text: five rows of booleans, glyphs separated by
/// one column of ground.
fn bitmap(text: &str) -> [Vec<bool>; 5] {
    let mut rows: [Vec<bool>; 5] = Default::default();
    for (index, glyph) in text.chars().map(glyph_or_space).enumerate() {
        for (row, pixels) in rows.iter_mut().zip(glyph.rows) {
            if index > 0 {
                row.extend(std::iter::repeat_n(false, GAP));
            }
            row.extend(pixels.chars().map(|p| p == '#'));
        }
    }
    rows
}

/// The tall figure: five lines of `█`.
pub fn tall(text: &str, style: Style) -> Vec<Line<'static>> {
    bitmap(text)
        .iter()
        .map(|row| {
            let painted: String = row.iter().map(|&on| if on { '█' } else { ' ' }).collect();
            Line::from(Span::styled(painted, style))
        })
        .collect()
}

/// The short figure: the same bitmap folded into two rows of half-blocks.
/// Rows 1 and 2 of the bitmap share one pixel row, so five rows become four
/// and four rows are two characters tall.
pub fn short(text: &str, style: Style) -> Vec<Line<'static>> {
    let rows = bitmap(text);
    let folded: [Vec<bool>; 4] = [
        rows[0].clone(),
        rows[1]
            .iter()
            .zip(&rows[2])
            .map(|(a, b)| *a || *b)
            .collect(),
        rows[3].clone(),
        rows[4].clone(),
    ];
    [(0, 1), (2, 3)]
        .iter()
        .map(|&(top, bottom)| {
            let painted: String = folded[top]
                .iter()
                .zip(&folded[bottom])
                .map(|(t, b)| match (t, b) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect();
            Line::from(Span::styled(painted, style))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_has_five_rows_of_one_width() {
        for c in "ABCDEFG#b0123456789Δm+-suadino/≈· ".chars() {
            let glyph = glyph(c).unwrap_or_else(|| panic!("no glyph for {c:?}"));
            let width = glyph.rows[0].len();
            assert!(
                glyph.rows.iter().all(|r| r.len() == width),
                "{c:?} has ragged rows"
            );
        }
    }

    #[test]
    fn width_counts_gaps_between_glyphs() {
        assert_eq!(width("C6"), 5 + 1 + 4);
    }

    #[test]
    fn the_short_figure_is_two_rows_of_the_same_width() {
        let lines = short("C6", Style::default());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].width(), width("C6"));
    }
}
