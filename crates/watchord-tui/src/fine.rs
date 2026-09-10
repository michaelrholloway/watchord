//! The fine figure: the headline drawn large on a half-block grid.
//!
//! Each glyph is a bitmap ten rows tall on a grid whose pixel is half a
//! terminal cell — one cell wide, half a cell high, near square at 8.4×10 —
//! so a figure five terminal rows tall carries twice the detail of the
//! full-cell figure in [`crate::figure`]. Two bitmap rows fold into one
//! terminal row as `▀`, `▄` or `█`, which every terminal draws. A scale
//! multiplies each pixel by a whole number of cells, so a larger window gets
//! a larger figure with the same shapes.
//!
//! The alphabet is the notation alphabet and nothing else — `A`–`G`, `#`,
//! `b`, the digits, `Δ`, `m`, `+`, `-`, the letters of `sus`, `add`, `dim`
//! and `no`, `/`, `≈`, `·`, and space. Michael's TALL / SHORT figures in the
//! Figma file (`10:2`) are the model; the widths follow the full-cell set.

/// One glyph: ten rows of `#` (ink) and `.` (ground), all the same width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub rows: [&'static str; 10],
}

/// Bitmap rows per glyph.
pub const BITMAP_ROWS: usize = 10;
/// Terminal rows the figure takes at scale 1: two bitmap rows per cell row.
pub const ROWS: u16 = (BITMAP_ROWS / 2) as u16;
/// Columns between glyphs, in pixels.
const GAP: usize = 1;

const fn g(rows: [&'static str; 10]) -> Glyph {
    Glyph { rows }
}

/// The glyph for one character of the alphabet, or `None` outside it.
#[rustfmt::skip]
pub fn glyph(c: char) -> Option<Glyph> {
    let glyph = match c {
        'A' => g([".###.", "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#", "#...#", "#...#"]),
        'B' => g(["####.", "#...#", "#...#", "#...#", "####.", "#...#", "#...#", "#...#", "#...#", "####."]),
        'C' => g([".###.", "#...#", "#....", "#....", "#....", "#....", "#....", "#....", "#...#", ".###."]),
        'D' => g(["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
        'E' => g(["#####", "#....", "#....", "#....", "####.", "#....", "#....", "#....", "#....", "#####"]),
        'F' => g(["#####", "#....", "#....", "#....", "####.", "#....", "#....", "#....", "#....", "#...."]),
        'G' => g([".###.", "#...#", "#....", "#....", "#....", "#..##", "#...#", "#...#", "#...#", ".###."]),
        '#' => g([".#.#.", ".#.#.", "#####", ".#.#.", ".#.#.", ".#.#.", "#####", ".#.#.", ".#.#.", "....."]),
        'b' => g(["#...", "#...", "#...", "###.", "#..#", "#..#", "#..#", "#..#", "#..#", "###."]),
        '0' => g([".##.", "#..#", "#..#", "#..#", "#..#", "#..#", "#..#", "#..#", "#..#", ".##."]),
        '1' => g([".#.", "##.", ".#.", ".#.", ".#.", ".#.", ".#.", ".#.", ".#.", "###"]),
        '2' => g([".##.", "#..#", "...#", "...#", "..#.", ".#..", ".#..", "#...", "#...", "####"]),
        '3' => g([".##.", "#..#", "...#", "...#", ".##.", "...#", "...#", "...#", "#..#", ".##."]),
        '4' => g(["...#", "..##", "..##", ".#.#", ".#.#", "#..#", "####", "...#", "...#", "...#"]),
        '5' => g(["####", "#...", "#...", "#...", "###.", "...#", "...#", "...#", "#..#", ".##."]),
        '6' => g([".##.", "#..#", "#...", "#...", "###.", "#..#", "#..#", "#..#", "#..#", ".##."]),
        '7' => g(["####", "...#", "...#", "...#", "..#.", "..#.", "..#.", ".#..", ".#..", ".#.."]),
        '8' => g([".##.", "#..#", "#..#", "#..#", ".##.", "#..#", "#..#", "#..#", "#..#", ".##."]),
        '9' => g([".##.", "#..#", "#..#", "#..#", ".###", "...#", "...#", "...#", "#..#", ".##."]),
        'Δ' => g(["..#..", "..#..", "..#..", ".#.#.", ".#.#.", ".#.#.", "#...#", "#...#", "#...#", "#####"]),
        'm' => g([".....", ".....", ".....", "##.#.", "#.#.#", "#.#.#", "#.#.#", "#.#.#", "#.#.#", "#.#.#"]),
        '+' => g(["...", "...", "...", ".#.", ".#.", "###", ".#.", ".#.", "...", "..."]),
        '-' => g(["...", "...", "...", "...", "...", "###", "...", "...", "...", "..."]),
        's' => g(["....", "....", "....", ".###", "#...", "#...", ".##.", "...#", "...#", "###."]),
        'u' => g(["....", "....", "....", "#..#", "#..#", "#..#", "#..#", "#..#", "#..#", ".###"]),
        'a' => g(["....", "....", "....", ".##.", "...#", "...#", ".###", "#..#", "#..#", ".###"]),
        'd' => g(["...#", "...#", "...#", ".###", "#..#", "#..#", "#..#", "#..#", "#..#", ".###"]),
        'i' => g([".", "#", ".", "#", "#", "#", "#", "#", "#", "#"]),
        'n' => g(["....", "....", "....", "###.", "#..#", "#..#", "#..#", "#..#", "#..#", "#..#"]),
        'o' => g(["....", "....", "....", ".##.", "#..#", "#..#", "#..#", "#..#", "#..#", ".##."]),
        '/' => g(["....#", "....#", "...#.", "...#.", "..#..", "..#..", ".#...", ".#...", "#....", "#...."]),
        '≈' => g([".....", ".....", ".##.#", "#..#.", ".....", ".....", ".##.#", "#..#.", ".....", "....."]),
        '·' => g([".", ".", ".", ".", ".", ".", "#", ".", ".", "."]),
        ' ' => g(["..", "..", "..", "..", "..", "..", "..", "..", "..", ".."]),
        _ => return None,
    };
    Some(glyph)
}

/// The characters of `text` that have no glyph.
pub fn missing(text: &str) -> Vec<char> {
    text.chars().filter(|&c| glyph(c).is_none()).collect()
}

fn glyph_or_space(c: char) -> Glyph {
    glyph(c).unwrap_or_else(|| glyph(' ').expect("space is in the alphabet"))
}

/// Columns the figure of `text` takes at `scale`. A character outside the
/// alphabet counts as a space.
pub fn width(text: &str, scale: u16) -> u16 {
    let glyphs: Vec<Glyph> = text.chars().map(glyph_or_space).collect();
    let ink: usize = glyphs.iter().map(|g| g.rows[0].len()).sum();
    let pixels = ink + GAP * glyphs.len().saturating_sub(1);
    (pixels * scale.max(1) as usize) as u16
}

/// Terminal rows the figure takes at `scale`.
pub fn height(scale: u16) -> u16 {
    ROWS * scale.max(1)
}

/// The largest scale at which the figure of `text` fits `width` columns and
/// `rows` terminal rows, or `None` when it does not fit at scale 1.
pub fn scale_to_fit(text: &str, width_cells: u16, rows: u16) -> Option<u16> {
    (1..=8)
        .rev()
        .find(|&scale| width(text, scale) <= width_cells && height(scale) <= rows)
}

/// The bitmap of the whole text at `scale`: `10 × scale` rows of pixels,
/// glyphs separated by one column of ground, every pixel `scale` wide.
fn bitmap(text: &str, scale: u16) -> Vec<Vec<bool>> {
    let scale = scale.max(1) as usize;
    let mut rows: Vec<Vec<bool>> = vec![Vec::new(); BITMAP_ROWS];
    for (index, glyph) in text.chars().map(glyph_or_space).enumerate() {
        for (row, pixels) in rows.iter_mut().zip(glyph.rows) {
            if index > 0 {
                row.extend(std::iter::repeat_n(false, GAP * scale));
            }
            for p in pixels.chars() {
                row.extend(std::iter::repeat_n(p == '#', scale));
            }
        }
    }
    rows.iter()
        .flat_map(|row| std::iter::repeat_n(row.clone(), scale))
        .collect()
}

/// The figure as text: `height(scale)` strings, each `width(text, scale)`
/// characters of `▀`, `▄`, `█` and space.
pub fn render(text: &str, scale: u16) -> Vec<String> {
    let rows = bitmap(text, scale);
    rows.chunks(2)
        .map(|pair| {
            pair[0]
                .iter()
                .zip(&pair[1])
                .map(|(t, b)| match (t, b) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALPHABET: &str = "ABCDEFG#b0123456789Δm+-suadino/≈· ";

    #[test]
    fn every_glyph_has_ten_rows_of_one_width() {
        for c in ALPHABET.chars() {
            let glyph = glyph(c).unwrap_or_else(|| panic!("{c:?} has a glyph"));
            let width = glyph.rows[0].len();
            assert!(width > 0, "{c:?}");
            for row in glyph.rows {
                assert_eq!(row.len(), width, "{c:?} row {row:?}");
                assert!(row.chars().all(|p| p == '#' || p == '.'), "{c:?} {row:?}");
            }
        }
        assert!(missing("C7#9").is_empty());
        assert_eq!(missing("C7 x"), vec!['x']);
    }

    #[test]
    fn the_figure_is_five_rows_per_scale_and_scales_its_width() {
        let one = render("C7#9", 1);
        assert_eq!(one.len(), 5);
        assert_eq!(width("C7#9", 1), 5 + 1 + 4 + 1 + 5 + 1 + 4);
        assert!(
            one.iter()
                .all(|r| r.chars().count() == width("C7#9", 1) as usize)
        );
        let two = render("C7#9", 2);
        assert_eq!(two.len(), 10);
        assert_eq!(width("C7#9", 2), 2 * width("C7#9", 1));
        assert!(
            two.iter()
                .all(|r| r.chars().count() == width("C7#9", 2) as usize)
        );
        // At scale 2 every pixel is two cells wide and one cell tall, so no
        // half block is needed.
        assert!(two.iter().all(|r| !r.contains('▀') && !r.contains('▄')));
    }

    #[test]
    fn a_pixel_folds_into_the_half_block_that_names_its_half() {
        // `·` is ink on bitmap row 6 only: terminal row 3 (rows 6–7) shows
        // its top half. Each wave of `≈` sits inside one terminal row, so
        // the two waves draw alike.
        let dot = render("·", 1);
        assert_eq!(dot, vec![" ", " ", " ", "▀", " "]);
        let wave = render("≈", 1);
        assert_eq!(wave[1], wave[3]);
        assert_eq!(wave[1], "▄▀▀▄▀");
        // `i` is ink on every row from 3 down, and row 1: `▄` then solid.
        let i = render("i", 1);
        assert_eq!(i, vec!["▄", "▄", "█", "█", "█"]);
    }

    #[test]
    fn scale_to_fit_picks_the_largest_scale_and_none_below_one() {
        assert_eq!(scale_to_fit("C6", 10, 5), Some(1));
        assert_eq!(scale_to_fit("C6", 20, 10), Some(2));
        assert_eq!(scale_to_fit("C6", 20, 9), Some(1));
        assert_eq!(scale_to_fit("C7#9", 17, 20), None);
    }
}
