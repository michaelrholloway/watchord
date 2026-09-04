//! The plain skin: every field of the [`Frame`], as plain text.
//!
//! Michael's words: *"the most basic skin, just white and black, minimal
//! terminal ui"* that outputs *"all data, simple text"*. So: the terminal's
//! default foreground and background, no colour token, no box-drawing
//! character, section labels in UPPERCASE, tables aligned with spaces, the
//! headline as one line of text. Nothing hides behind a density tier — a
//! short terminal draws what fits, top down, with the note field and the key
//! reference kept at the foot.
//!
//! An absent optional field still draws its label, with `—` for the value, so
//! that what the frame *can* say is always on screen. A test greps each
//! snapshot for every label in [`Frame::NOW_PLAYING_LABELS`] and
//! [`Frame::ALL_NOTES_LABELS`].
//!
//! The mouse and every key work as they do in PUSH: this module fills the same
//! [`Hits`], and [`crate::tui`] reads them the same way.

use std::collections::BTreeSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthStr;
use watchord_model::notes::tags_of;
use watchord_model::{Frame, FrameReading, Screen};

use crate::screens::{Drawn, Hits, ScrollTarget, UiState};
use crate::when;

/// The value drawn for an absent optional field.
const ABSENT: &str = "—";

/// The one-line key reference at the foot of every screen.
const KEYS_HELP: &str = "tab screen · ↑↓ select · d delete · e edit · s sort · p drill · shift-enter line break · enter save · q quit";

/// A note's text on one row: an embedded line break would otherwise split the
/// row, so it is shown as a visible mark instead.
fn single_line(text: &str) -> String {
    text.replace('\n', " ⏎ ")
}

/// The mark before the selected row.
const SELECTED: &str = "> ";
/// The mark before every other row.
const UNSELECTED: &str = "  ";

/// Draws the whole frame. Returns what the mouse can hit in it.
pub fn draw(target: &mut ratatui::Frame, frame: &Frame, ui: &UiState) -> Hits {
    let area = target.area();
    let buf = target.buffer_mut();
    // No page style: the terminal's own foreground and background.
    let drawn = draw_into(area, buf, frame, ui);
    if let Some(position) = drawn.cursor {
        target.set_cursor_position(position);
    }
    drawn.hits
}

/// Draws the whole frame into a buffer.
pub fn draw_into(area: Rect, buf: &mut Buffer, frame: &Frame, ui: &UiState) -> Drawn {
    let inner = Rect {
        x: area.x + 1,
        width: area.width.saturating_sub(2),
        ..area
    };
    let mut hits = Hits::default();
    let mut page = Page::new(inner, buf);

    // The foot first, so a short terminal never clips it.
    let help_row = page.take_bottom();
    let field_row = page.take_bottom();
    if frame.screen == Screen::NowPlaying && frame.note_target_key().is_none() && !frame.editing {
        // Room for the hint under a disabled field.
        page.take_bottom();
    }

    head(frame, &mut page, &mut hits);
    pedals(frame, &mut page);
    input_picker(frame, ui, &mut page);
    headline_block(frame, &mut page);

    let cursor = match frame.screen {
        Screen::NowPlaying => {
            readings(frame, ui, &mut page, &mut hits);
            annotations(frame, &mut page);
            drill_block(frame, &mut page);
            editing(frame, &mut page);
            notes(frame, ui, &mut page, &mut hits);
            note_field(frame, field_row, &mut page, &mut hits)
        }
        Screen::AllNotes => {
            groups(frame, ui, &mut page, &mut hits);
            None
        }
    };
    if let Some(row) = help_row {
        page.put(row, 0, &KEYS_HELP.to_uppercase());
    }
    Drawn { cursor, hits }
}

/// A top-down writer over one column of rows. Writes plain cells, no style.
struct Page<'a> {
    area: Rect,
    buf: &'a mut Buffer,
    top: u16,
    bottom: u16,
}

impl<'a> Page<'a> {
    fn new(area: Rect, buf: &'a mut Buffer) -> Self {
        Page {
            area,
            buf,
            top: area.y,
            bottom: area.y + area.height,
        }
    }

    fn remaining(&self) -> u16 {
        self.bottom.saturating_sub(self.top)
    }

    /// The next row, or `None` when the page is full.
    fn take(&mut self) -> Option<u16> {
        if self.remaining() == 0 {
            return None;
        }
        let y = self.top;
        self.top += 1;
        Some(y)
    }

    /// The last row, or `None` when the page is full.
    fn take_bottom(&mut self) -> Option<u16> {
        if self.remaining() == 0 {
            return None;
        }
        self.bottom -= 1;
        Some(self.bottom)
    }

    fn skip(&mut self) {
        self.take();
    }

    /// Writes `text` at column `x` of row `y`, clipped to the page. Returns the
    /// rect the text occupies.
    fn put(&mut self, y: u16, x: u16, text: &str) -> Rect {
        let start = self.area.x + x;
        let right = self.area.x + self.area.width;
        let mut column = start;
        for grapheme in text.chars() {
            if column >= right {
                break;
            }
            self.buf[(column, y)].set_symbol(&grapheme.to_string());
            self.buf[(column, y)].set_style(Style::default());
            column += 1;
        }
        Rect {
            x: start,
            y,
            width: column.saturating_sub(start),
            height: 1,
        }
    }

    /// One `LABEL  value` line on the next row.
    fn line(&mut self, label: &str, value: &str) {
        if let Some(y) = self.take() {
            self.put(y, 0, &format!("{:<14} {}", label.to_uppercase(), value));
        }
    }

    /// A whole row of text.
    fn text(&mut self, text: &str) {
        if let Some(y) = self.take() {
            self.put(y, 0, text);
        }
    }

    /// The rect of rows `from..to` across the page.
    fn span(&self, from: u16, to: u16) -> Rect {
        Rect {
            x: self.area.x,
            y: from,
            width: self.area.width,
            height: to.saturating_sub(from),
        }
    }
}

fn or_absent(value: Option<&str>) -> &str {
    match value {
        Some(text) if !text.is_empty() => text,
        _ => ABSENT,
    }
}

/// Pads `text` to `width` columns, cutting it when it is longer.
fn cell(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        if used >= width {
            break;
        }
        out.push(c);
        used += 1;
    }
    while used < width {
        out.push(' ');
        used += 1;
    }
    out
}

// MARK: - Chrome

/// The running head, the plates, the banner, the status, and the tabs.
fn head(frame: &Frame, page: &mut Page, hits: &mut Hits) {
    let total = format!("NOTES TOTAL {}", frame.notes_total);
    // The displayed chord's own count, beside the total, only when it has notes.
    let chord_notes = if frame.notes.is_empty() {
        String::new()
    } else {
        format!("   NOTES {}", frame.notes.len())
    };
    page.text(&format!("WATCHORD   {total}{chord_notes}"));
    let inputs = if frame.inputs.is_empty() {
        ABSENT.to_string()
    } else {
        frame.inputs.join(", ")
    };
    page.text(&format!(
        "INPUT {}   STATE {}   INPUTS {}",
        frame.input,
        frame.state.label(),
        inputs
    ));
    page.line("banner", or_absent(frame.banner.as_deref()));
    page.line("status", or_absent(frame.status.as_deref()));

    if let Some(y) = page.take() {
        let mut x = page.put(y, 0, "SCREEN").width + 9;
        for screen in Screen::ALL {
            let label = if screen == frame.screen {
                format!("[{}]", screen.title())
            } else {
                format!(" {} ", screen.title())
            };
            let rect = page.put(y, x, &label);
            hits.tabs.push((rect, screen));
            x += label.width() as u16 + 3;
        }
    }
    page.skip();
}

/// The three pedal plates, the settle plate, and the arpeggio plate — its own
/// fn, called from one line in `draw_into`, so four parts editing `head`
/// merge cleanly.
fn pedals(frame: &Frame, page: &mut Page) {
    page.text(&format!(
        "SUSTAIN {}   SOSTENUTO {}   SOFT {}   SETTLE {} ms   ARPEGGIO {}",
        pedal_word(frame.sustain),
        pedal_word(frame.sostenuto),
        pedal_word(frame.soft),
        frame.settle_ms,
        mode_word(frame.arpeggio),
    ));
}

fn pedal_word(down: bool) -> &'static str {
    if down { "DOWN" } else { "UP" }
}

fn mode_word(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}

/// The input picker: `i` opens it, ↑↓ highlight, enter chooses, esc closes.
/// The device names themselves are always on screen in the running head
/// (`INPUTS`); this is only the chooser.
fn input_picker(frame: &Frame, ui: &UiState, page: &mut Page) {
    if !ui.input_picker_open {
        page.text("PICKER   press i to choose an input");
        return;
    }
    page.text("PICKER   OPEN — up/down choose, enter select, esc close");
    if frame.inputs.is_empty() {
        page.text(&format!("{UNSELECTED}none"));
    } else {
        for (index, name) in frame.inputs.iter().enumerate() {
            let mark = if index == ui.input_picker_index {
                SELECTED
            } else {
                UNSELECTED
            };
            page.text(&format!("{mark}{name}"));
        }
    }
    page.skip();
}

/// The headline as one line, and every fact beside it.
fn headline_block(frame: &Frame, page: &mut Page) {
    page.line("headline", &frame.headline_text);
    page.line("approximation", or_absent(frame.headline_approximation()));
    page.line("spoken", or_absent(frame.headline_spoken()));
    page.line("fit", or_absent(frame.headline_fit_note()));
    page.line("declined", or_absent(frame.declined.as_deref()));
    page.line("keys", or_absent(Some(frame.keys.as_str())));
    page.line("key", or_absent(Some(frame.key.raw())));
    page.line("sounding", or_absent(Some(frame.sounding_row().as_str())));
    page.skip();
}

// MARK: - Now Playing

const RANK_WIDTH: usize = 5;
const NAME_WIDTH: usize = 14;
const ORIGIN_WIDTH: usize = 11;
const FIT_WIDTH: usize = 20;
const SCORE_WIDTH: usize = 6;
const ROOT_WIDTH: usize = 5;
const CLAIMED_WIDTH: usize = 22;

fn reading_row(rank: usize, reading: &FrameReading) -> String {
    let name = match &reading.approximation {
        Some(mark) => format!("{} {mark}", reading.name),
        None => reading.name.clone(),
    };
    let fit = match &reading.fit_detail {
        Some(detail) => format!("{} {detail}", reading.fit_name()),
        None => reading.fit_name().to_string(),
    };
    format!(
        "{}{}{}{}{}{}{}{}",
        UNSELECTED,
        cell(&rank.to_string(), RANK_WIDTH),
        cell(&name, NAME_WIDTH),
        cell(reading.origin.raw_value(), ORIGIN_WIDTH),
        cell(&fit, FIT_WIDTH),
        cell(&reading.score.to_string(), SCORE_WIDTH),
        cell(reading.root_name(), ROOT_WIDTH),
        cell(&reading.claimed_row(), CLAIMED_WIDTH),
    ) + &reading.spoken
}

/// Every reading, the headline first, with every fact the engine ranked on.
fn readings(frame: &Frame, ui: &UiState, page: &mut Page, hits: &mut Hits) {
    let all = frame.readings();
    page.text(&format!(
        "READINGS {} · HEADLINE {} · ALTERNATES {}",
        all.len(),
        frame.headline.iter().count(),
        frame.alternates.len()
    ));
    page.text(&format!(
        "{}{}{}{}{}{}{}{}SPOKEN",
        UNSELECTED,
        cell("RANK", RANK_WIDTH),
        cell("NAME", NAME_WIDTH),
        cell("ORIGIN", ORIGIN_WIDTH),
        cell("FIT", FIT_WIDTH),
        cell("SCORE", SCORE_WIDTH),
        cell("ROOT", ROOT_WIDTH),
        cell("CLAIMED", CLAIMED_WIDTH),
    ));
    let from = page.top;
    if all.is_empty() {
        page.text(&format!("{UNSELECTED}none"));
    }
    let first = ui.scroll.alternates.min(all.len().saturating_sub(1));
    for (index, reading) in all.iter().enumerate().skip(first) {
        page.text(&reading_row(index + 1, reading));
    }
    let rect = page.span(from, page.top);
    if rect.height > 0 {
        hits.scroll_areas.push((rect, ScrollTarget::Alternates));
    }
    page.skip();
}

/// The slot the theory crate fills. Empty until it does.
fn annotations(frame: &Frame, page: &mut Page) {
    let value = if frame.annotations.is_empty() {
        "none yet"
    } else {
        ""
    };
    page.line("annotations", value);
    page.skip();
}

const CHORD_WIDTH: usize = 14;
const ATTEMPTS_WIDTH: usize = 10;
const EXACT_WIDTH: usize = 8;

/// Drill: a mode on Now Playing, not a screen (spec #9, ticket #14). Kept to
/// two guaranteed rows — one line folding target, next target, and grade
/// together, one stats header — so a 30-row terminal still fits everything
/// below it (the note field, `editing`); the per-chord stat rows beneath the
/// header are the part that is free to run out of room, the same way the
/// notes and group tables already do.
fn drill_block(frame: &Frame, page: &mut Page) {
    let drill = &frame.drill;
    let status = if drill.active { "on" } else { "off" };
    page.line(
        "drill",
        &format!(
            "{status}   TARGET {}   NEXT TARGET {}   GRADE {}   DRILL STATS {}",
            or_absent(drill.target.as_deref()),
            or_absent(drill.next_target.as_deref()),
            or_absent(drill.grade_display()),
            drill.stats.len(),
        ),
    );
    if !drill.stats.is_empty() {
        page.text(&format!(
            "{}{}{}{}LAST",
            UNSELECTED,
            cell("CHORD", CHORD_WIDTH),
            cell("ATTEMPTS", ATTEMPTS_WIDTH),
            cell("EXACT", EXACT_WIDTH),
        ));
        for row in &drill.stats {
            let last = row
                .last_at
                .map(when::format)
                .unwrap_or_else(|| ABSENT.to_string());
            page.text(&format!(
                "{}{}{}{}{}",
                UNSELECTED,
                cell(&row.chord, CHORD_WIDTH),
                cell(&row.attempts.to_string(), ATTEMPTS_WIDTH),
                cell(&row.exact.to_string(), EXACT_WIDTH),
                last,
            ));
        }
    }
}

/// Whether the note field is editing an existing note or drafting a new one.
fn editing(frame: &Frame, page: &mut Page) {
    page.line("editing", if frame.editing { "yes" } else { ABSENT });
}

/// The notes on the displayed chord, newest first, a `delete` on each row.
fn notes(frame: &Frame, ui: &UiState, page: &mut Page, hits: &mut Hits) {
    page.text(&format!("NOTES {}", frame.notes.len()));
    if frame.notes.is_empty() {
        page.text(&format!("{UNSELECTED}none yet"));
        return;
    }
    let selected = ui.selected_now_playing;
    let visible = page.remaining() as usize;
    let first = scroll_to(selected, visible)
        .max(ui.scroll.notes)
        .min(frame.notes.len().saturating_sub(1));
    let from = page.top;
    for (index, note) in frame.notes.iter().enumerate().skip(first) {
        let Some(y) = page.take() else { break };
        let mark = if selected == Some(index) {
            SELECTED
        } else {
            UNSELECTED
        };
        let row = page.put(y, 0, &format!("{mark}{}", single_line(&note.text)));
        let delete = page.put(y, page.area.width.saturating_sub(6), "delete");
        hits.note_rows.push((
            Rect {
                width: page.area.width,
                ..row
            },
            index,
        ));
        hits.delete_cells.push((delete, index));
    }
    let rect = page.span(from, page.top);
    if rect.height > 0 {
        hits.scroll_areas.push((rect, ScrollTarget::Notes));
    }
}

/// The first row to draw so that `selected` is inside `visible` rows.
fn scroll_to(selected: Option<usize>, visible: usize) -> usize {
    match selected {
        Some(index) if visible > 0 && index >= visible => index + 1 - visible,
        _ => 0,
    }
}

/// The note field on its anchored row. Returns the cursor position in it.
fn note_field(
    frame: &Frame,
    row: Option<u16>,
    page: &mut Page,
    hits: &mut Hits,
) -> Option<(u16, u16)> {
    let y = row?;
    // Editing an existing note needs no live chord to commit against.
    let enabled = frame.note_target_key().is_some() || frame.editing;
    let label = if frame.editing { "EDIT" } else { "DRAFT" };
    let shown = single_line(&frame.draft);
    let text = if !frame.draft.is_empty() {
        shown.clone()
    } else if enabled {
        "add a note…".to_string()
    } else {
        "play a chord to write a note against it".to_string()
    };
    let rect = page.put(y, 0, &format!("{label:<14} {text}"));
    hits.field = Some(Rect {
        width: page.area.width,
        ..rect
    });
    if enabled {
        let x = page.area.x + 15 + (shown.width() as u16).min(page.area.width.saturating_sub(16));
        Some((x, y))
    } else {
        None
    }
}

// MARK: - All Notes

const WRITTEN_AS_WIDTH: usize = 12;
const WHEN_WIDTH: usize = 17;

/// Every note, grouped under the chord it belongs to.
fn groups(frame: &Frame, ui: &UiState, page: &mut Page, hits: &mut Hits) {
    page.text(&format!(
        "GROUPS {} · NOTES TOTAL {}",
        frame.groups.len(),
        frame.notes_total
    ));
    page.line(
        "search",
        if frame.search.is_empty() {
            ABSENT
        } else {
            frame.search.as_str()
        },
    );
    page.line("sort", frame.notes_sort.label());
    if frame.groups.is_empty() {
        page.text(&format!("{UNSELECTED}no notes yet"));
        return;
    }
    // One flat list of rows: a group head, then its notes. The selection is an
    // ordinal over notes only.
    enum Row<'a> {
        Group(&'a watchord_model::NoteGroup),
        Note(usize, &'a watchord_core::ChordNote),
    }
    let mut rows = Vec::new();
    let mut ordinal = 0;
    let mut selected_row = None;
    for group in &frame.groups {
        rows.push(Row::Group(group));
        for note in &group.notes {
            if ui.selected_all_notes == Some(ordinal) {
                selected_row = Some(rows.len());
            }
            rows.push(Row::Note(ordinal, note));
            ordinal += 1;
        }
    }
    let visible = page.remaining() as usize;
    let first = scroll_to(selected_row, visible)
        .max(ui.scroll.all_notes)
        .min(rows.len().saturating_sub(1));
    let note_width =
        page.area
            .width
            .saturating_sub((2 + WRITTEN_AS_WIDTH + WHEN_WIDTH + 7) as u16) as usize;
    let from = page.top;
    for row in rows.iter().skip(first) {
        let Some(y) = page.take() else { break };
        match row {
            Row::Group(group) => {
                let tags: BTreeSet<String> = tags_of(group);
                let tags_text = if tags.is_empty() {
                    ABSENT.to_string()
                } else {
                    tags.iter()
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                page.put(
                    y,
                    0,
                    &format!(
                        "GROUP {}   KEY {}   NOTES {}   TAGS {}",
                        group.heading,
                        group.key.raw(),
                        group.notes.len(),
                        tags_text,
                    ),
                );
                let Some(y) = page.take() else { break };
                page.put(
                    y,
                    0,
                    &format!(
                        "{}{}{}{}",
                        UNSELECTED,
                        cell("NOTE", note_width),
                        cell("WRITTEN AS", WRITTEN_AS_WIDTH),
                        cell("WHEN", WHEN_WIDTH),
                    ),
                );
            }
            Row::Note(ordinal, note) => {
                let mark = if ui.selected_all_notes == Some(*ordinal) {
                    SELECTED
                } else {
                    UNSELECTED
                };
                let text = format!(
                    "{}{}{}{}",
                    mark,
                    cell(&single_line(&note.text), note_width),
                    cell(&note.spelling_when_written, WRITTEN_AS_WIDTH),
                    cell(&when::format(note.created_at), WHEN_WIDTH),
                );
                let rect = page.put(y, 0, &text);
                let delete = page.put(y, page.area.width.saturating_sub(6), "delete");
                hits.note_rows.push((
                    Rect {
                        width: page.area.width,
                        ..rect
                    },
                    *ordinal,
                ));
                hits.delete_cells.push((delete, *ordinal));
            }
        }
    }
    let rect = page.span(from, page.top);
    if rect.height > 0 {
        hits.scroll_areas.push((rect, ScrollTarget::AllNotes));
    }
}
