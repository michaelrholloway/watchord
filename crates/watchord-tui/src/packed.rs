//! The packed skin: Michael's Figma frame `48:2938`, cell for cell at 80×24,
//! growing into a larger terminal (spec #20, ADR-0006).
//!
//! One outer box and a foot. The body is two columns: the left three
//! quarters hold the title over a rule, then on one column grid the KEY,
//! FUNCTION, NUMERAL and KEYS rows, the READINGS table, the HISTORY table
//! and the NOTES section with the field and the saved notes. The right
//! quarter holds the headline — the name as a fine figure where it fits,
//! else one line, over the spoken form — at the top and the staff under
//! it, full height, no rule between. Section headers
//! are rows filled `#1E1E1E`; one blank row divides two sections (the
//! layout Michael asked for on 2026-09-09, over the frame's narrow left
//! panel).
//!
//! What the frame carries and the skin does not draw stays in the frame:
//! drill, settle, the sostenuto and soft pedals, score, root, claimed,
//! Nashville, the annotations, voice leading. The skin binds no key to any
//! of them; PUSH and plain still do.
//!
//! The mouse and every bound key work as in the other skins: this module
//! fills the same [`Hits`], and [`crate::tui`] reads them the same way.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;
use watchord_model::notes::tags_of;
use watchord_model::{Frame, FrameState, Screen};
use watchord_theory::staff::{Clef, StaffNote};

use crate::fine;
use crate::push::Token;
use crate::screens::{Drawn, Hits, ScrollTarget, UiState};

// MARK: - Palette

/// The eight colours of the design, hex from Figma, with a 256-colour
/// fallback each (ADR-0003 (c)).
const GROUND: Token = Token {
    hex: 0x000000,
    fallback: 16,
};
const INK: Token = Token {
    hex: 0xFFFFFF,
    fallback: 231,
};
const DIM: Token = Token {
    hex: 0x808080,
    fallback: 244,
};
const FILL: Token = Token {
    hex: 0x1E1E1E,
    fallback: 234,
};
const LIME: Token = Token {
    hex: 0xC6F000,
    fallback: 190,
};
const MAGENTA: Token = Token {
    hex: 0xF031E6,
    fallback: 201,
};
const PINK: Token = Token {
    hex: 0xFABED4,
    fallback: 218,
};
const RED: Token = Token {
    hex: 0xFF0000,
    fallback: 196,
};

fn ink() -> Style {
    Style::default().fg(INK.color()).bg(GROUND.color())
}
fn bold() -> Style {
    ink().add_modifier(Modifier::BOLD)
}
fn dim() -> Style {
    Style::default().fg(DIM.color()).bg(GROUND.color())
}
fn dim_bold() -> Style {
    dim().add_modifier(Modifier::BOLD)
}
fn lime_bold() -> Style {
    Style::default()
        .fg(LIME.color())
        .bg(GROUND.color())
        .add_modifier(Modifier::BOLD)
}
/// Text on a filled (`#1E1E1E`) row or chip.
fn on_fill() -> Style {
    Style::default().fg(INK.color()).bg(FILL.color())
}
fn on_fill_bold() -> Style {
    on_fill().add_modifier(Modifier::BOLD)
}
fn dim_on_fill() -> Style {
    Style::default().fg(DIM.color()).bg(FILL.color())
}
fn lime_on_fill_bold() -> Style {
    Style::default()
        .fg(LIME.color())
        .bg(FILL.color())
        .add_modifier(Modifier::BOLD)
}
/// Text on the selected note row: black on lime.
fn on_lime() -> Style {
    Style::default().fg(GROUND.color()).bg(LIME.color())
}

// MARK: - Geometry

/// The smallest terminal the skin draws in. Below it, one line says so.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
const TOO_SMALL: &str = "watchord needs 80×24";

/// The staff column's share of the inner width: the right quarter. The
/// headline and every table take the rest, at the left.
const STAFF_SHARE: u16 = 4;
/// How the headline draws at the top of the staff column: the name as a
/// fine figure at `scale`, or as one line when no scale fits, then the
/// spoken form with the fit detail after it. `rows` is the block's height;
/// a blank row follows, then the staff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeadlinePlan {
    scale: Option<u16>,
    rows: u16,
}

/// The headline's name as drawn: `≈` before it when the fit is nearest,
/// `—` when there is no chord.
fn headline_name(frame: &Frame) -> String {
    match frame.headline.as_ref() {
        Some(headline) => match &headline.approximation {
            Some(mark) => format!("{mark} {}", headline.name),
            None => headline.name.clone(),
        },
        None => ABSENT.to_string(),
    }
}

/// The largest figure that fits the staff column, leaving the staff its
/// nine rows and a blank row; one text line when none does, or when there
/// is no chord to draw.
fn plan_headline(frame: &Frame, column: Rect) -> HeadlinePlan {
    let name = headline_name(frame);
    let figure_rows = column.height.saturating_sub(1 + 1 + STAFF_ROWS);
    let scale = match frame.headline {
        Some(_) => fine::scale_to_fit(&name, column.width.saturating_sub(2), figure_rows),
        None => None,
    };
    HeadlinePlan {
        scale,
        rows: scale.map_or(1, fine::height) + 1,
    }
}
/// A five-line staff: nine rows, one per position — a line on each even
/// row, a space on each odd row — so every head sits exactly on its line or
/// in its space. Text alone, so every terminal draws the same staff.
const STAFF_ROWS: u16 = 9;
/// A note head.
const HEAD: &str = "■";
/// Rows between the treble and bass staves in text.
const STAFF_GAP: u16 = 2;
/// Both clefs draw when the staff box has room for two staves and the gap.
const BOTH_CLEFS_FROM: u16 = STAFF_ROWS * 2 + STAFF_GAP;
/// The staff lines' width; they centre in the staff column.
const STAFF_LINE_WIDTH: u16 = 10;

/// The right panel's grid, as offsets from its left edge: the label or NAME
/// column, the second column every section shares, then FIT (the history
/// chips share its room), NUMERAL and FUNCTION. FIT takes a wider terminal's
/// slack up to a cap, so the two right columns stay near the rest of the
/// table rather than hanging off the far edge.
const COL_LABEL: u16 = 0;
const COL_VALUE: u16 = 12;
const COL_FIT: u16 = 23;
const FIT_MIN: u16 = 9;
const FIT_MAX: u16 = 26;
const NUMERAL_WIDTH: u16 = 8;
const FUNCTION_WIDTH: u16 = 11;

/// The five column starts for a panel `w` cells wide.
fn columns(w: u16) -> [u16; 5] {
    let fit = w
        .saturating_sub(COL_FIT + NUMERAL_WIDTH + FUNCTION_WIDTH)
        .clamp(FIT_MIN, FIT_MAX);
    let numeral = COL_FIT + fit;
    let function = numeral + NUMERAL_WIDTH;
    [COL_LABEL, COL_VALUE, COL_FIT, numeral, function]
}

/// The value drawn for an absent optional field.
const ABSENT: &str = "—";
const ELLIPSIS: &str = "…";
const DOT: &str = "●";
const SQUARE: &str = "■";

/// The rows each list is guaranteed at 24 rows.
const READINGS_MIN: usize = 1;
const HISTORY_MIN: usize = 1;
const NOTES_MIN: usize = 0;
// One blank row divides two sections (Michael's ruling, 2026-09-09, over
// the earlier underline border, which drew differently in every terminal).

/// How the right panel's body rows are spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    readings: usize,
    history: usize,
    notes: usize,
}

/// Spare rows beyond the minimums go, in order: to
/// readings until every reading shows, to notes until every note shows, to
/// history until every entry shows, then the rest to notes. Every row is one
/// terminal row; nothing is double-spaced. `rows` is the section area under
/// the headline's rule; the four KEY rows, three headers, two heads rows,
/// the note field and three blank dividers are fixed.
fn allocate(rows: u16, readings: usize, history: usize, notes: usize) -> Layout {
    const FIXED: usize = 4 + 1 + 2 + 1 + 2 + 1 + 3;
    let variable = (rows as usize).saturating_sub(FIXED);
    let mut layout = Layout {
        readings: READINGS_MIN,
        history: HISTORY_MIN,
        notes: NOTES_MIN,
    };
    let mut spare = variable.saturating_sub(READINGS_MIN + HISTORY_MIN + NOTES_MIN);
    let give = spare.min(readings.max(1).saturating_sub(layout.readings));
    layout.readings += give;
    spare -= give;
    let give = spare.min(notes.saturating_sub(layout.notes));
    layout.notes += give;
    spare -= give;
    let give = spare.min(history.saturating_sub(layout.history));
    layout.history += give;
    spare -= give;
    layout.notes += spare;
    // A list never holds a blank row above the next section's header: rows a
    // short list cannot fill move down to the notes, at the foot of the panel.
    let readings_used = layout.readings.min(readings.max(1));
    let history_used = layout.history.min(history.max(1));
    layout.notes += (layout.readings - readings_used) + (layout.history - history_used);
    layout.readings = readings_used;
    layout.history = history_used;
    layout
}

// MARK: - Entry

/// Draws the whole frame. Returns what the mouse can hit in it.
pub fn draw(target: &mut ratatui::Frame, frame: &Frame, ui: &UiState) -> Hits {
    let area = target.area();
    let buf = target.buffer_mut();
    let drawn = draw_into(area, buf, frame, ui);
    if let Some(position) = drawn.cursor {
        target.set_cursor_position(position);
    }
    drawn.hits
}

/// Draws the whole frame into a buffer.
pub fn draw_into(area: Rect, buf: &mut Buffer, frame: &Frame, ui: &UiState) -> Drawn {
    buf.set_style(area, ink());
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        Canvas { buf, area }.put(area.x, area.y, TOO_SMALL, dim());
        return Drawn::default();
    }

    let mut canvas = Canvas { buf, area };
    let mut hits = Hits::default();
    let x0 = area.x;
    let x1 = area.x + area.width - 1;
    let y0 = area.y;
    let y1 = area.y + area.height - 1;
    let staff_width = (area.width - 2) / STAFF_SHARE;
    let divider_x = x1 - 1 - staff_width;

    if frame.screen == Screen::AllNotes {
        let body = Rect::new(x0 + 1, y0 + 2, area.width - 2, area.height - 5);
        canvas.frame_box(None);
        canvas.put(
            x0 + 2,
            y0 + 1,
            "WATCHORD",
            bold().add_modifier(Modifier::ITALIC),
        );
        let cursor = canvas.all_notes(body, frame, ui, &mut hits);
        canvas.foot(x0 + 2, y1 - 1, x1 - 1, frame);
        return Drawn { cursor, hits };
    }

    canvas.frame_box(Some(divider_x));

    // Title, over a rule across the left column.
    canvas.put(
        x0 + 2,
        y0 + 1,
        "WATCHORD",
        bold().add_modifier(Modifier::ITALIC),
    );
    if let Some(step) = frame.history_step {
        let plate = format!("HISTORY {}/{}", step.index, step.total);
        canvas.put(
            divider_x - 1 - plate.width() as u16,
            y0 + 1,
            &plate,
            dim_bold(),
        );
    }
    canvas.rule(x0 + 1, y0 + 2, divider_x - x0 - 1);

    // Body: the left column holds the tables under the rule; the staff
    // column at the right holds the headline at the top, then the staff.
    let body_bottom = y1 - 3; // inclusive
    let sections = Rect {
        x: x0 + 1,
        y: y0 + 3,
        width: divider_x - x0 - 1,
        height: body_bottom - y0 - 2,
    };
    let column = Rect {
        x: divider_x + 1,
        y: y0 + 1,
        width: x1 - divider_x - 1,
        height: body_bottom - y0,
    };
    let plan = plan_headline(frame, column);
    let headline = Rect {
        height: plan.rows,
        ..column
    };
    let staff = Rect {
        y: column.y + plan.rows + 1,
        height: column.height - plan.rows - 1,
        ..column
    };
    let cursor = canvas.sections(sections, frame, ui, &mut hits);
    canvas.headline_box(headline, frame, plan);
    canvas.staff_box(staff, frame);

    // Foot.
    canvas.foot(x0 + 2, y1 - 1, x1 - 1, frame);

    Drawn { cursor, hits }
}

// MARK: - Text helpers

/// `text` cut to `width` cells with an ellipsis when it did not fit.
fn cut(text: &str, width: usize) -> String {
    let text = single_line(text);
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + 1 >= width {
            break;
        }
        out.push(c);
    }
    out + ELLIPSIS
}

/// A note's text on one row: an embedded line break shows as a mark.
fn single_line(text: &str) -> String {
    text.replace('\n', " ⏎ ")
}

fn or_absent(value: Option<&str>) -> &str {
    match value {
        Some(text) if !text.is_empty() => text,
        _ => ABSENT,
    }
}

/// What INPUT reads: the picker's filter when set, else the plate's own word
/// — `fake` in a demo, a device name, a device count, `none`.
fn input_word(frame: &Frame) -> String {
    frame
        .input_filter
        .clone()
        .unwrap_or_else(|| frame.input.clone())
        .to_uppercase()
}

/// The first row to draw so that `selected` is inside `visible` rows.
fn scroll_to(selected: Option<usize>, visible: usize) -> usize {
    match selected {
        Some(index) if visible > 0 && index >= visible => index + 1 - visible,
        _ => 0,
    }
}

/// The rows one clef's text window needs: the staff, widened to any ledger
/// note.
fn window_rows(notes: &[StaffNote], clef: Clef) -> u16 {
    let rows: Vec<i32> = notes
        .iter()
        .filter(|n| n.clef == clef)
        .map(|n| n.row)
        .collect();
    let top = rows.iter().copied().max().unwrap_or(8).max(8);
    let bottom = rows.iter().copied().min().unwrap_or(0).min(0);
    (top - bottom + 1) as u16
}

// MARK: - Canvas

/// The buffer and the area every write is clipped to.
struct Canvas<'a> {
    buf: &'a mut Buffer,
    area: Rect,
}

impl Canvas<'_> {
    /// Writes `text` at `(x, y)`, clipped to the area. Returns the rect it
    /// covers.
    fn put(&mut self, x: u16, y: u16, text: &str, style: Style) -> Rect {
        let right = self.area.x + self.area.width;
        let mut column = x;
        if y < self.area.y || y >= self.area.y + self.area.height {
            return Rect::new(x, y, 0, 1);
        }
        for c in text.chars() {
            if column >= right {
                break;
            }
            let cell = &mut self.buf[(column, y)];
            cell.set_symbol(&c.to_string());
            cell.set_style(style);
            column += 1;
        }
        Rect::new(x, y, column.saturating_sub(x), 1)
    }

    /// Writes `text` cut to `width` cells, an ellipsis on the last one when
    /// it did not fit.
    fn put_cut(&mut self, x: u16, y: u16, text: &str, width: u16, style: Style) -> Rect {
        self.put(x, y, &cut(text, width as usize), style)
    }

    /// Fills `width` cells at `(x, y)` with spaces in `style`.
    fn fill_row(&mut self, x: u16, y: u16, width: u16, style: Style) {
        let blank: String = " ".repeat(width as usize);
        self.put(x, y, &blank, style);
    }

    /// A rule across `width` cells at `(x, y)`, joined to the vertical lines
    /// on either side: `├────┤`.
    fn rule(&mut self, x: u16, y: u16, width: u16) {
        let line: String = "─".repeat(width as usize);
        self.put(x - 1, y, &format!("├{line}┤"), ink());
    }

    /// `LABEL[K]:` — the word in `word_style`, the bracketed keys dim, the
    /// colon in ink. Returns the width written.
    fn hint_label(&mut self, x: u16, y: u16, label: HintLabel) -> u16 {
        let mut cursor = x;
        cursor += self.put(cursor, y, label.word, label.style).width;
        if !label.keys.is_empty() {
            cursor += self.put(cursor, y, label.keys, label.keys_style).width;
        }
        if label.colon {
            cursor += self.put(cursor, y, ":", label.colon_style).width;
        }
        cursor - x
    }

    /// `ON ●` lit lime, or `OFF ●` dim. Returns the width written.
    fn on_off(&mut self, x: u16, y: u16, on: bool) -> u16 {
        let word = if on { "ON" } else { "OFF" };
        let mut cursor = x;
        cursor += self.put(cursor, y, word, ink()).width + 1;
        cursor += self
            .put(cursor, y, DOT, if on { lime_bold() } else { dim() })
            .width;
        cursor - x
    }

    /// The sounding keys as chips: ` C3 ` in `style`, a space between,
    /// as many as fit in `width`.
    fn chips(&mut self, x: u16, y: u16, width: u16, keys: &str, style: Style) {
        let right = x + width;
        let mut cursor = x;
        for key in keys.split_whitespace() {
            let chip = format!(" {key} ");
            if cursor + chip.width() as u16 > right {
                break;
            }
            cursor += self.put(cursor, y, &chip, style).width + 1;
        }
    }

    // MARK: Chrome

    /// The outer box, the rule over the foot, and, on Now Playing, the
    /// staff column's divider from the top border down to the foot rule.
    fn frame_box(&mut self, divider_x: Option<u16>) {
        let area = self.area;
        let x0 = area.x;
        let x1 = area.x + area.width - 1;
        let y0 = area.y;
        let y1 = area.y + area.height - 1;
        let style = ink();
        let horizontal: String = "─".repeat((area.width - 2) as usize);
        self.put(x0, y0, &format!("┌{horizontal}┐"), style);
        self.put(x0, y1 - 2, &format!("├{horizontal}┤"), style);
        self.put(x0, y1, &format!("└{horizontal}┘"), style);
        for y in y0 + 1..y1 - 2 {
            self.put(x0, y, "│", style);
            self.put(x1, y, "│", style);
        }
        self.put(x1, y1 - 1, "│", style);
        self.put(x0, y1 - 1, "│", style);
        if let Some(divider_x) = divider_x {
            self.put(divider_x, y0, "┬", style);
            for y in y0 + 1..y1 - 2 {
                self.put(divider_x, y, "│", style);
            }
            self.put(divider_x, y1 - 2, "┴", style);
        }
    }

    /// The foot: SUSTAIN, ARPEGGIO, STATE and INPUT, each with its dot,
    /// spread across the row. A status message, when the model has one,
    /// takes the row.
    fn foot(&mut self, x: u16, y: u16, right: u16, frame: &Frame) {
        if let Some(status) = frame.status.as_deref() {
            self.put_cut(x, y, status, right - x, ink());
            return;
        }
        let width = right - x;
        // Four groups; the first sits at the left edge, the last ends at the
        // right edge, the middle two spread evenly.
        let sustain_w = "SUSTAIN: OFF ●".width() as u16;
        let arpeggio_w = "ARPEGGIO[A]: OFF ●".width() as u16;
        let state_w = "STATE: RELEASED ●".width() as u16;
        let input_text = input_word(frame);
        let input_w = ("INPUT[I]: ".width() + input_text.width()) as u16;
        let used = sustain_w + arpeggio_w + state_w + input_w;
        let gap = width.saturating_sub(used) / 3;

        let mut cursor = x;
        cursor += self.put(cursor, y, "SUSTAIN:", ink()).width + 1;
        cursor += self.on_off(cursor, y, frame.sustain);
        cursor += gap;
        cursor += self.hint_label(cursor, y, HintLabel::lime("ARPEGGIO", "[A]").colon()) + 1;
        cursor += self.on_off(cursor, y, frame.arpeggio);
        cursor += gap;
        cursor += self.put(cursor, y, "STATE:", bold()).width + 1;
        let (state_word, playing) = match frame.state {
            FrameState::Held => ("PLAYING", true),
            FrameState::Released => ("RELEASED", false),
            FrameState::Idle => ("IDLE", false),
        };
        cursor += self.put(cursor, y, state_word, ink()).width + 1;
        self.put(cursor, y, DOT, if playing { lime_bold() } else { dim() });

        let mut cursor = right.saturating_sub(input_w);
        cursor += self.hint_label(cursor, y, HintLabel::lime("INPUT", "[I]").colon()) + 1;
        self.put_cut(cursor, y, &input_text, right.saturating_sub(cursor), ink());
    }

    // MARK: Staff column

    /// At the top of the staff column: the name in lime — the fine figure
    /// at the plan's scale, else one bold line — with `≈` before it when
    /// the fit is nearest; under it the spoken form, then the fit detail
    /// dim after two spaces. Declined: `—` and the reason. Long text is cut
    /// to the column.
    fn headline_box(&mut self, box_: Rect, frame: &Frame, plan: HeadlinePlan) {
        let x = box_.x + 1;
        let w = box_.width - 2;
        let name = headline_name(frame);
        match plan.scale {
            Some(scale) => {
                for (row, line) in (box_.y..).zip(fine::render(&name, scale)) {
                    self.put(x, row, &line, lime_bold());
                }
            }
            None => {
                self.put_cut(x, box_.y, &name, w, lime_bold());
            }
        }
        let y = box_.y + plan.rows - 1;
        match frame.headline.as_ref() {
            Some(headline) => {
                let spoken = headline.spoken.to_uppercase();
                let written = self.put_cut(x, y, &spoken, w, ink()).width;
                if let Some(detail) = headline.fit_detail.as_deref() {
                    let at = x + written + 2;
                    self.put_cut(at, y, detail, (x + w).saturating_sub(at), dim());
                }
            }
            None => {
                if let Some(reason) = frame.declined.as_deref() {
                    self.put_cut(x, y, reason, w, dim());
                }
            }
        }
    }

    /// The staff: five lines a clef,
    /// one row per position, `■` heads with their accidental in the cell to
    /// the left, short ledger lines out to the farthest note. Treble alone
    /// unless the box has room for both clefs and the gap between; the spare
    /// rows go above and below the pair.
    fn staff_box(&mut self, box_: Rect, frame: &Frame) {
        let notes = &frame.annotations.staff;
        if box_.height < BOTH_CLEFS_FROM {
            self.staff(box_, Clef::Treble, notes);
            return;
        }
        // Each clef asks for the rows its window needs; a low bass note asks
        // for more than nine. A shortfall comes off the taller of the two
        // first, never below nine.
        let (mut t, mut b) = (
            window_rows(notes, Clef::Treble),
            window_rows(notes, Clef::Bass),
        );
        while t + b + STAFF_GAP > box_.height {
            if b >= t && b > STAFF_ROWS {
                b -= 1;
            } else if t > STAFF_ROWS {
                t -= 1;
            } else {
                break;
            }
        }
        let block = t + STAFF_GAP + b;
        let top = box_.y + box_.height.saturating_sub(block) / 2;
        let treble = Rect {
            y: top,
            height: t,
            ..box_
        };
        let bass = Rect {
            y: top + t + STAFF_GAP,
            height: b,
            ..box_
        };
        self.staff(treble, Clef::Treble, notes);
        self.staff(bass, Clef::Bass, notes);
    }

    /// One clef in `box_`, one row per position. The window covers the staff
    /// and every ledger position out to the farthest note; when the box is
    /// too small, the empty edges give way first, then the ledger note
    /// farthest from the staff. Heads stack on one column; an accidental
    /// sits in the cell to the left and never moves the head.
    fn staff(&mut self, box_: Rect, clef: Clef, notes: &[StaffNote]) {
        let notes: Vec<&StaffNote> = notes.iter().filter(|n| n.clef == clef).collect();
        let rows = box_.height as i32;
        if rows <= 0 {
            return;
        }
        let (note_min, note_max) = match (
            notes.iter().map(|n| n.row).min(),
            notes.iter().map(|n| n.row).max(),
        ) {
            (Some(min), Some(max)) => (min, max),
            _ => (0, 8),
        };
        let mut top = note_max.max(8);
        let mut bottom = note_min.min(0);
        while top - bottom + 1 > rows {
            if top > note_max {
                top -= 1;
            } else if bottom < note_min || (bottom < 0 && -bottom >= top - 8) {
                // An empty bottom edge, or the ledger farthest from the staff.
                bottom += 1;
            } else {
                top -= 1;
            }
        }
        let slack = rows - (top - bottom + 1);
        let y_top = box_.y as i32 + slack / 2;
        let line_x = box_.x + box_.width.saturating_sub(STAFF_LINE_WIDTH) / 2;
        let head_x = line_x + STAFF_LINE_WIDTH / 2;
        let line: String = "─".repeat(STAFF_LINE_WIDTH as usize);
        for row in (bottom..=top).rev() {
            let y = (y_top + (top - row)) as u16;
            let on_staff = (0..=8).contains(&row);
            if on_staff && row % 2 == 0 {
                self.put(line_x, y, &line, ink());
            }
            let heads: Vec<&&StaffNote> = notes.iter().filter(|n| n.row == row).collect();
            // Every even row off the staff between it and the farthest note
            // is a ledger line, as on paper.
            if !on_staff && row % 2 == 0 {
                let reaches = if row < 0 {
                    notes.iter().any(|n| n.row <= row)
                } else {
                    notes.iter().any(|n| n.row >= row)
                };
                if reaches {
                    let ledger: String = "─".repeat(2 * heads.len().max(1) + 1);
                    self.put(head_x - 1, y, &ledger, ink());
                }
            }
            for (index, head) in heads.iter().enumerate() {
                let x = head_x + 2 * index as u16;
                let accidental = head.spelled.accidental.symbol();
                if !accidental.is_empty() {
                    self.put(x - 1, y, accidental, bold());
                }
                self.put(x, y, HEAD, bold());
            }
        }
    }

    // MARK: Sections

    /// The four field rows, the two tables and the notes section, top down,
    /// a blank row between sections. Returns the cursor position in the
    /// note field.
    fn sections(
        &mut self,
        area: Rect,
        frame: &Frame,
        ui: &UiState,
        hits: &mut Hits,
    ) -> Option<(u16, u16)> {
        let x = area.x;
        let w = area.width;
        let mut y = area.y;
        let layout = allocate(
            area.height,
            frame.readings().len(),
            frame.history.len(),
            frame.notes.len(),
        );
        // KEY, on the fill like a section header; then FUNCTION, NUMERAL, KEYS.
        self.fill_row(x, y, w, on_fill());
        self.hint_label(
            x + COL_LABEL,
            y,
            HintLabel::lime("KEY", "[K][M]").colon().on_fill(),
        );
        let key = frame.key_context.as_ref().map(|k| k.label().to_uppercase());
        self.put_cut(
            x + COL_VALUE,
            y,
            or_absent(key.as_deref()),
            w - COL_VALUE,
            on_fill(),
        );
        y += 1;
        self.put(x + COL_LABEL, y, "FUNCTION:", bold());
        let function = frame
            .headline
            .as_ref()
            .and_then(|h| h.function)
            .map(|f| f.label().to_uppercase());
        self.put_cut(
            x + COL_VALUE,
            y,
            or_absent(function.as_deref()),
            w - COL_VALUE,
            ink(),
        );
        y += 1;
        self.put(x + COL_LABEL, y, "NUMERAL:", bold());
        let numeral = frame.headline.as_ref().and_then(|h| h.numeral.as_deref());
        self.put_cut(x + COL_VALUE, y, or_absent(numeral), w - COL_VALUE, ink());
        y += 1;
        self.put(x + COL_LABEL, y, "KEYS:", bold());
        // Chips start a cell early so the chip text sits on the value column.
        self.chips(
            x + COL_VALUE - 1,
            y,
            w - COL_VALUE + 1,
            &frame.keys,
            on_fill(),
        );
        y += 2;

        y = self.readings(x, y, w, layout.readings, frame, ui, hits) + 1;
        y = self.history(x, y, w, layout.history, frame) + 1;

        // NOTES.
        self.section_header(x, y, w, "NOTES", PINK);
        y += 1;
        let cursor = self.note_field(x, y, w, frame, hits);
        y += 1;
        self.put(x + COL_LABEL, y, "CHORD", dim_bold());
        self.put(x + COL_VALUE, y, "NOTE", dim_bold());
        y += 1;
        self.notes(Rect::new(x, y, w, layout.notes as u16), frame, ui, hits);

        cursor
    }

    /// The READINGS header, heads and rows. Returns the row after them.
    #[allow(clippy::too_many_arguments)]
    fn readings(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        rows: usize,
        frame: &Frame,
        ui: &UiState,
        hits: &mut Hits,
    ) -> u16 {
        let [_, _, _, col_numeral, col_function] = columns(w);
        self.section_header(x, y, w, "READINGS", MAGENTA);
        self.table_heads(
            x,
            y + 1,
            w,
            ["NAME", "ORIGIN", "FIT", "NUMERAL", "FUNCTION"],
        );
        let from = y + 2;
        let readings = frame.readings();
        let first = ui
            .scroll
            .alternates
            .min(readings.len().saturating_sub(rows));
        for (row_y, reading) in (from..).zip(readings.iter().skip(first).take(rows)) {
            let name = match &reading.approximation {
                Some(mark) => format!("{mark} {}", reading.name),
                None => reading.name.clone(),
            };
            let fit = match &reading.fit_detail {
                Some(detail) => format!("{} {detail}", reading.fit_name().to_uppercase()),
                None => reading.fit_name().to_uppercase(),
            };
            self.put_cut(x + COL_LABEL, row_y, &name, COL_VALUE - 1, ink());
            self.put_cut(
                x + COL_VALUE,
                row_y,
                &reading.origin.raw_value().to_uppercase(),
                COL_FIT - COL_VALUE - 1,
                ink(),
            );
            self.put_cut(x + COL_FIT, row_y, &fit, col_numeral - COL_FIT - 1, ink());
            self.put_cut(
                x + col_numeral,
                row_y,
                or_absent(reading.numeral.as_deref()),
                col_function - col_numeral - 1,
                ink(),
            );
            self.put_cut(
                x + col_function,
                row_y,
                or_absent(reading.function.map(|f| f.label())),
                w - col_function,
                ink(),
            );
        }
        if readings.is_empty() {
            self.put(x + COL_LABEL, from, ABSENT, dim());
        }
        hits.scroll_areas
            .push((Rect::new(x, from, w, rows as u16), ScrollTarget::Alternates));
        from + rows as u16
    }

    /// The HISTORY header, heads and rows, newest last, one row each, the
    /// keys as filled chips as the design draws them. The window
    /// ends at the newest entry, unless the display is stepped to an older
    /// one that must stay in view; the stepped row is filled. Returns the
    /// row after the block.
    fn history(&mut self, x: u16, y: u16, w: u16, rows: usize, frame: &Frame) -> u16 {
        let [_, _, _, col_numeral, col_function] = columns(w);
        self.section_header(x, y, w, "HISTORY", MAGENTA);
        self.table_heads(x, y + 1, w, ["NAME", "KEYS", "", "NUMERAL", "FUNCTION"]);
        let from = y + 2;
        let total = frame.history.len();
        let visible = rows.min(total);
        let end = match frame.history_step {
            Some(step) => step.index.max(visible).min(total),
            None => total,
        };
        let start = end.saturating_sub(visible);
        let shown = frame
            .history
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start);
        for (row_y, (index, entry)) in (from..).zip(shown) {
            let stepped = frame.history_step.is_some_and(|s| s.index == index + 1);
            let (text, chip) = if stepped {
                self.fill_row(x, row_y, w, on_fill());
                (on_fill(), ink())
            } else {
                (ink(), on_fill())
            };
            self.put_cut(x + COL_LABEL, row_y, &entry.name, COL_VALUE - 1, text);
            self.chips(
                x + COL_VALUE - 1,
                row_y,
                col_numeral - COL_VALUE,
                &entry.keys,
                chip,
            );
            self.put_cut(
                x + col_numeral,
                row_y,
                or_absent(entry.numeral.as_deref()),
                col_function - col_numeral - 1,
                text,
            );
            self.put_cut(
                x + col_function,
                row_y,
                or_absent(entry.function.map(|f| f.label())),
                w - col_function,
                text,
            );
        }
        if total == 0 {
            self.put(x + COL_LABEL, from, ABSENT, dim());
        }
        from + rows as u16
    }

    /// A filled row with the section's name and its coloured square at the
    /// end.
    fn section_header(&mut self, x: u16, y: u16, w: u16, name: &str, mark: Token) {
        self.fill_row(x, y, w, on_fill());
        self.put(x + COL_LABEL, y, name, on_fill_bold());
        self.put(
            x + w - 2,
            y,
            SQUARE,
            Style::default().fg(mark.color()).bg(FILL.color()),
        );
    }

    /// The five column heads on the grid, dim. An empty head draws nothing.
    fn table_heads(&mut self, x: u16, y: u16, w: u16, heads: [&str; 5]) {
        for (head, col) in heads.iter().zip(columns(w)) {
            if !head.is_empty() {
                self.put(x + col, y, head, dim_bold());
            }
        }
    }

    /// The note field with `SAVE[⏎]` and `VIEW ALL[N]` beside it. Returns
    /// the cursor position when the field can take text.
    fn note_field(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        frame: &Frame,
        hits: &mut Hits,
    ) -> Option<(u16, u16)> {
        let save = if frame.editing { "EDIT" } else { "SAVE" };
        let hints_w = (save.width() + "[⏎]".width() + 1 + "VIEW ALL[N]".width()) as u16;
        let field_w = w.saturating_sub(hints_w + 1);
        self.fill_row(x, y, field_w, on_fill());
        let enabled = frame.note_target_key().is_some() || frame.editing;
        let shown = single_line(&frame.draft);
        let cursor = if !frame.draft.is_empty() {
            // Keep the tail in view when the draft outruns the field.
            let max = field_w.saturating_sub(2) as usize;
            let chars: Vec<char> = shown.chars().collect();
            let skip = chars.len().saturating_sub(max);
            let visible: String = chars[skip..].iter().collect();
            let rect = self.put(x + 1, y, &visible, on_fill());
            Some((rect.x + rect.width, y))
        } else if enabled {
            self.put_cut(x + 1, y, "add a note...", field_w - 1, dim_on_fill());
            Some((x + 1, y))
        } else {
            self.put_cut(
                x + 1,
                y,
                "play a chord to write a note",
                field_w - 1,
                dim_on_fill(),
            );
            None
        };
        hits.field = Some(Rect::new(x, y, field_w, 1));
        let mut cursor_x = x + field_w + 1;
        cursor_x += self.hint_label(cursor_x, y, HintLabel::ink(save, "[⏎]")) + 1;
        let start = cursor_x;
        cursor_x += self.hint_label(cursor_x, y, HintLabel::lime("VIEW ALL", "[N]"));
        hits.tabs
            .push((Rect::new(start, y, cursor_x - start, 1), Screen::AllNotes));
        cursor
    }

    /// The saved notes on this chord, newest first, one row each: the name
    /// it was written as, then the text. The selected row is lime with black
    /// text and shows `EDIT[E]` and `DEL[D]`.
    fn notes(&mut self, list: Rect, frame: &Frame, ui: &UiState, hits: &mut Hits) {
        let rows = list.height as usize;
        if frame.notes.is_empty() || rows == 0 {
            return;
        }
        let (x, w) = (list.x, list.width);
        let selected = ui.selected_now_playing;
        let first = scroll_to(selected, rows)
            .max(ui.scroll.notes)
            .min(frame.notes.len().saturating_sub(1));
        let actions_w = "EDIT[E] DEL[D]".width() as u16;
        let shown = frame.notes.iter().enumerate().skip(first).take(rows);
        for (row_y, (index, note)) in (list.y..).zip(shown) {
            let is_selected = selected == Some(index);
            let style = if is_selected {
                self.fill_row(x, row_y, w, on_lime());
                on_lime()
            } else {
                ink()
            };
            self.put_cut(
                x + COL_LABEL,
                row_y,
                &note.spelling_when_written,
                COL_VALUE - 1,
                style,
            );
            let text_w = if is_selected {
                w.saturating_sub(COL_VALUE + actions_w + 1)
            } else {
                w - COL_VALUE
            };
            self.put_cut(x + COL_VALUE, row_y, &note.text, text_w, style);
            hits.note_rows.push((Rect::new(x, row_y, w, 1), index));
            if is_selected {
                let mut cursor = x + w - actions_w;
                cursor += self.hint_label(cursor, row_y, HintLabel::on_lime("EDIT", "[E]")) + 1;
                let del_x = cursor;
                cursor += self.hint_label(cursor, row_y, HintLabel::red_on_lime("DEL", "[D]"));
                hits.delete_cells
                    .push((Rect::new(del_x, row_y, cursor - del_x, 1), index));
            }
        }
        hits.scroll_areas.push((list, ScrollTarget::Notes));
    }

    // MARK: All Notes

    /// Every note, grouped under its chord, in the packed chrome: the NOTES
    /// header, the search field, the sort and the way back, the column
    /// heads, then one filled row per group and one row per note. Same grid
    /// as Now Playing. Returns the cursor position in the search field.
    fn all_notes(
        &mut self,
        body: Rect,
        frame: &Frame,
        ui: &UiState,
        hits: &mut Hits,
    ) -> Option<(u16, u16)> {
        let (x, w) = (body.x, body.width);
        let mut y = body.y;
        let [_, _, _, col_written, col_when] = all_notes_columns(w);

        self.section_header(x, y, w, "NOTES", PINK);
        self.put(
            x + COL_VALUE,
            y,
            &format!("{} · {} GROUPS", frame.notes_total, frame.groups.len()),
            dim_on_fill(),
        );
        y += 1;

        // The search field, on the value column.
        self.put(x + COL_LABEL, y, "SEARCH:", bold());
        let field_w = w - COL_VALUE;
        self.fill_row(x + COL_VALUE, y, field_w, on_fill());
        let cursor = if frame.search.is_empty() {
            self.put_cut(
                x + COL_VALUE + 1,
                y,
                "type to search",
                field_w - 1,
                dim_on_fill(),
            );
            Some((x + COL_VALUE + 1, y))
        } else {
            let rect = self.put_cut(x + COL_VALUE + 1, y, &frame.search, field_w - 1, on_fill());
            Some((rect.x + rect.width, y))
        };
        hits.field = Some(Rect::new(x + COL_VALUE, y, field_w, 1));
        y += 1;

        // Sort, and the way back.
        let mut cursor_x = x + COL_LABEL;
        cursor_x += self.hint_label(cursor_x, y, HintLabel::ink("SORT", "[S]").colon()) + 1;
        self.put(cursor_x, y, &frame.notes_sort.label().to_uppercase(), ink());
        let back = "NOW PLAYING[N]";
        let start = x + w - back.width() as u16;
        let end = start + self.hint_label(start, y, HintLabel::lime("NOW PLAYING", "[N]"));
        hits.tabs
            .push((Rect::new(start, y, end - start, 1), Screen::NowPlaying));
        y += 1;

        // Column heads.
        self.put(x + COL_LABEL, y, "CHORD", dim_bold());
        self.put(x + COL_VALUE, y, "NOTE", dim_bold());
        self.put(x + col_written, y, "WRITTEN AS", dim_bold());
        self.put(x + col_when, y, "WHEN", dim_bold());
        y += 1;

        let list = Rect::new(x, y, w, body.y + body.height - y);
        self.groups(list, frame, ui, hits);
        cursor
    }

    /// One flat list: a filled row per group, then its notes. The selection
    /// is an ordinal over notes only, as on PUSH and plain.
    fn groups(&mut self, list: Rect, frame: &Frame, ui: &UiState, hits: &mut Hits) {
        let (x, w) = (list.x, list.width);
        let rows = list.height as usize;
        if rows == 0 {
            return;
        }
        if frame.groups.is_empty() {
            let word = if frame.search.is_empty() {
                "no notes yet"
            } else {
                "nothing matches"
            };
            self.put(x + COL_LABEL, list.y, word, dim());
            return;
        }
        let [_, _, _, col_written, col_when] = all_notes_columns(w);
        enum Row<'a> {
            Group(&'a watchord_model::NoteGroup),
            Note(usize, &'a watchord_core::ChordNote),
        }
        let mut flat = Vec::new();
        let mut ordinal = 0;
        let mut selected_row = None;
        for group in &frame.groups {
            flat.push(Row::Group(group));
            for note in &group.notes {
                if ui.selected_all_notes == Some(ordinal) {
                    selected_row = Some(flat.len());
                }
                flat.push(Row::Note(ordinal, note));
                ordinal += 1;
            }
        }
        let first = scroll_to(selected_row, rows)
            .max(ui.scroll.all_notes)
            .min(flat.len().saturating_sub(1));
        let actions_w = "EDIT[E] DEL[D]".width() as u16;
        for (row_y, row) in (list.y..).zip(flat.iter().skip(first).take(rows)) {
            match row {
                Row::Group(group) => {
                    self.fill_row(x, row_y, w, on_fill());
                    self.put_cut(
                        x + COL_LABEL,
                        row_y,
                        &group.heading,
                        COL_VALUE - 1,
                        on_fill_bold(),
                    );
                    let tags: std::collections::BTreeSet<String> = tags_of(group);
                    let tags_text = tags
                        .iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let count = format!("{} NOTES", group.notes.len());
                    let summary = if tags_text.is_empty() {
                        count
                    } else {
                        format!("{count}   {tags_text}")
                    };
                    self.put_cut(
                        x + COL_VALUE,
                        row_y,
                        &summary,
                        col_written - COL_VALUE - 1,
                        dim_on_fill(),
                    );
                    self.put_cut(
                        x + col_written,
                        row_y,
                        group.key.raw(),
                        w - col_written,
                        dim_on_fill(),
                    );
                }
                Row::Note(ordinal, note) => {
                    let is_selected = ui.selected_all_notes == Some(*ordinal);
                    let style = if is_selected {
                        self.fill_row(x, row_y, w, on_lime());
                        on_lime()
                    } else {
                        ink()
                    };
                    self.put_cut(
                        x + COL_VALUE,
                        row_y,
                        &note.text,
                        col_written - COL_VALUE - 1,
                        style,
                    );
                    self.put_cut(
                        x + col_written,
                        row_y,
                        &note.spelling_when_written,
                        col_when - col_written - 1,
                        style,
                    );
                    hits.note_rows.push((Rect::new(x, row_y, w, 1), *ordinal));
                    if is_selected {
                        let mut cursor = x + w - actions_w;
                        cursor +=
                            self.hint_label(cursor, row_y, HintLabel::on_lime("EDIT", "[E]")) + 1;
                        let del_x = cursor;
                        cursor +=
                            self.hint_label(cursor, row_y, HintLabel::red_on_lime("DEL", "[D]"));
                        hits.delete_cells
                            .push((Rect::new(del_x, row_y, cursor - del_x, 1), *ordinal));
                    } else {
                        self.put_cut(
                            x + col_when,
                            row_y,
                            &crate::when::format(note.created_at),
                            w - col_when,
                            dim(),
                        );
                    }
                }
            }
        }
        hits.scroll_areas.push((list, ScrollTarget::AllNotes));
    }
}

/// The All Notes grid for a body `w` cells wide: CHORD, NOTE, then WRITTEN
/// AS and WHEN hanging off the right edge.
fn all_notes_columns(w: u16) -> [u16; 5] {
    let when = w.saturating_sub(WHEN_WIDTH);
    let written = when.saturating_sub(WRITTEN_AS_WIDTH);
    [COL_LABEL, COL_VALUE, COL_FIT, written, when]
}

/// `YYYY-MM-DD HH:MM`.
const WHEN_WIDTH: u16 = 16;
/// `WRITTEN AS` and a gap.
const WRITTEN_AS_WIDTH: u16 = 12;

/// A word with its bracketed key hint: `KEY[K][M]:`, `SAVE[⏎]`, `DEL[D]`.
struct HintLabel<'a> {
    word: &'a str,
    keys: &'a str,
    colon: bool,
    style: Style,
    keys_style: Style,
    colon_style: Style,
}

impl<'a> HintLabel<'a> {
    fn lime(word: &'a str, keys: &'a str) -> Self {
        HintLabel {
            word,
            keys,
            colon: false,
            style: lime_bold(),
            keys_style: dim_bold(),
            colon_style: bold(),
        }
    }

    fn ink(word: &'a str, keys: &'a str) -> Self {
        HintLabel {
            style: bold(),
            ..Self::lime(word, keys)
        }
    }

    fn on_lime(word: &'a str, keys: &'a str) -> Self {
        HintLabel {
            style: on_lime().add_modifier(Modifier::BOLD),
            keys_style: Style::default()
                .fg(DIM.color())
                .bg(LIME.color())
                .add_modifier(Modifier::BOLD),
            colon_style: on_lime().add_modifier(Modifier::BOLD),
            ..Self::lime(word, keys)
        }
    }

    fn red_on_lime(word: &'a str, keys: &'a str) -> Self {
        HintLabel {
            style: Style::default()
                .fg(RED.color())
                .bg(LIME.color())
                .add_modifier(Modifier::BOLD),
            ..Self::on_lime(word, keys)
        }
    }

    fn colon(self) -> Self {
        HintLabel {
            colon: true,
            ..self
        }
    }

    /// The same label on the `#1E1E1E` fill.
    fn on_fill(self) -> Self {
        HintLabel {
            style: lime_on_fill_bold(),
            keys_style: dim_on_fill().add_modifier(Modifier::BOLD),
            colon_style: on_fill_bold(),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_24_rows_the_lists_get_their_minimums() {
        // 18 section rows at 80×24: 14 fixed, 4 variable — the readings
        // take three, history one, and a note row comes with the 25th
        // terminal row.
        assert_eq!(
            allocate(18, 3, 9, 3),
            Layout {
                readings: 3,
                history: 1,
                notes: 0
            }
        );
    }

    #[test]
    fn spare_rows_go_to_readings_notes_history_then_notes_again() {
        // 19 section rows: three spare. Readings take two, notes one.
        assert_eq!(
            allocate(19, 3, 9, 3),
            Layout {
                readings: 3,
                history: 1,
                notes: 1
            }
        );
        // 40 section rows: 24 spare. Readings 2, notes 6 (six notes),
        // history 8 (nine entries), and the last 8 to notes.
        assert_eq!(
            allocate(40, 3, 9, 6),
            Layout {
                readings: 3,
                history: 9,
                notes: 14
            }
        );
        // A short list never keeps a blank row: every row a list cannot
        // fill goes to the notes.
        assert_eq!(
            allocate(20, 1, 0, 3),
            Layout {
                readings: 1,
                history: 1,
                notes: 4
            }
        );
        // Nothing to show: one `—` row each, and every spare row goes to notes.
        assert_eq!(
            allocate(30, 0, 0, 0),
            Layout {
                readings: 1,
                history: 1,
                notes: 14
            }
        );
    }

    #[test]
    fn cut_keeps_short_text_and_ends_long_text_with_an_ellipsis() {
        assert_eq!(cut("C7#9", 12), "C7#9");
        assert_eq!(cut("tristan & isolde chord", 12), "tristan & i…");
        assert_eq!(cut("a\nb", 12), "a ⏎ b");
    }

    #[test]
    fn the_grid_widens_fit_with_the_terminal_up_to_a_cap() {
        assert_eq!(columns(51), [0, 12, 23, 32, 40]);
        // A wider panel widens FIT; the two right columns follow it.
        assert_eq!(columns(61), [0, 12, 23, 42, 50]);
        // Past the cap the rest goes to FUNCTION on the right.
        assert_eq!(columns(120), [0, 12, 23, 49, 57]);
    }
}
