//! The two screens and the chrome around them, element for element with
//! note-view's `RootView.swift`, `NowPlayingScreen.swift` and
//! `AllNotesScreen.swift`.
//!
//! Everything drawn here is read off [`AppModel`]. The screens make no
//! decisions; they apply PUSH's roles and elements from [`crate::push`] and
//! never write a rule or a box themselves.
//!
//! Every mark on the screen reports something: the INPUT plate reads the live
//! device set, the STATE plate reads the sounding stream, the folio counts
//! notes, the keys line lists the notes down. There is no ornament.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;
use watchord_core::SpellingOrigin;
use watchord_model::{AppModel, Screen};

use crate::figure;
use crate::push::{
    self, Column, Plate, PlateTone, PlateVariant, RuleWeight, Section, Table, TableRow,
};
use crate::when;

/// What the skin holds that the model does not: which note row the keys have
/// selected on each screen, and how far the wheel has scrolled each table.
/// Nothing else — the draft text lives on the model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiState {
    /// The selected note on Now Playing, an index into
    /// `notes_for_displayed_chord`.
    pub selected_now_playing: Option<usize>,
    /// The selected note on All Notes, an ordinal over every note in every
    /// group, in the order drawn.
    pub selected_all_notes: Option<usize>,
    /// Rows scrolled off the top of each table by the wheel.
    pub scroll: Scroll,
}

/// The three tables the wheel can scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollTarget {
    Alternates,
    Notes,
    AllNotes,
}

/// Rows scrolled off the top of each table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub alternates: usize,
    pub notes: usize,
    pub all_notes: usize,
}

impl Scroll {
    pub fn get(&self, target: ScrollTarget) -> usize {
        match target {
            ScrollTarget::Alternates => self.alternates,
            ScrollTarget::Notes => self.notes,
            ScrollTarget::AllNotes => self.all_notes,
        }
    }

    pub fn set(&mut self, target: ScrollTarget, rows: usize) {
        match target {
            ScrollTarget::Alternates => self.alternates = rows,
            ScrollTarget::Notes => self.notes = rows,
            ScrollTarget::AllNotes => self.all_notes = rows,
        }
    }
}

/// Where the last frame put the things a mouse can hit. Filled by [`draw`]
/// from the rects ratatui laid out; the event loop tests a click against it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hits {
    /// Each tab's rect and the screen it opens.
    pub tabs: Vec<(Rect, Screen)>,
    /// Each drawn note row and the note's ordinal on this screen.
    pub note_rows: Vec<(Rect, usize)>,
    /// Each drawn `delete` cell and the note's ordinal.
    pub delete_cells: Vec<(Rect, usize)>,
    /// The note field.
    pub field: Option<Rect>,
    /// Each scrollable table's rect.
    pub scroll_areas: Vec<(Rect, ScrollTarget)>,
}

impl Hits {
    fn at<T: Copy>(items: &[(Rect, T)], x: u16, y: u16) -> Option<T> {
        let position = Position::new(x, y);
        items
            .iter()
            .find(|(rect, _)| rect.contains(position))
            .map(|(_, item)| *item)
    }

    pub fn tab_at(&self, x: u16, y: u16) -> Option<Screen> {
        Self::at(&self.tabs, x, y)
    }

    pub fn note_row_at(&self, x: u16, y: u16) -> Option<usize> {
        Self::at(&self.note_rows, x, y)
    }

    pub fn delete_at(&self, x: u16, y: u16) -> Option<usize> {
        Self::at(&self.delete_cells, x, y)
    }

    pub fn field_at(&self, x: u16, y: u16) -> bool {
        self.field
            .is_some_and(|rect| rect.contains(Position::new(x, y)))
    }

    pub fn scroll_at(&self, x: u16, y: u16) -> Option<ScrollTarget> {
        Self::at(&self.scroll_areas, x, y)
    }
}

/// What one frame produced besides pixels.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Drawn {
    /// Where the text cursor goes, when the note field is on screen.
    pub cursor: Option<(u16, u16)>,
    pub hits: Hits,
}

impl UiState {
    pub fn selected(&self, screen: Screen) -> Option<usize> {
        match screen {
            Screen::NowPlaying => self.selected_now_playing,
            Screen::AllNotes => self.selected_all_notes,
        }
    }

    pub fn set_selected(&mut self, screen: Screen, selected: Option<usize>) {
        match screen {
            Screen::NowPlaying => self.selected_now_playing = selected,
            Screen::AllNotes => self.selected_all_notes = selected,
        }
    }
}

/// Below this many rows the chrome packs: one-row plates, tabs and section
/// heads, the keys as a line rather than a panel, no blank rows between
/// blocks. Nothing the screen says changes.
pub const COMPACT_BELOW: u16 = 44;

/// From this many rows the headline takes the tall figure; below it, the short
/// one. Independent of the chrome packing, so a 40-row terminal still gets the
/// headline drawn large.
pub const TALL_FROM: u16 = 30;

/// The one-line key reference at the foot of every screen.
const KEYS_HELP: &str = "tab screen · ↑↓ select · d delete · enter save · q quit";

/// Draws the whole frame. Returns what the mouse can hit in it.
pub fn draw(frame: &mut Frame, model: &AppModel, ui: &UiState) -> Hits {
    let area = frame.area();
    let buf = frame.buffer_mut();
    buf.set_style(area, push::page());
    let drawn = draw_into(area, buf, model, ui);
    if let Some(position) = drawn.cursor {
        frame.set_cursor_position(position);
    }
    drawn.hits
}

/// Draws the whole frame into a buffer.
pub fn draw_into(area: Rect, buf: &mut Buffer, model: &AppModel, ui: &UiState) -> Drawn {
    let mut hits = Hits::default();
    let compact = area.height < COMPACT_BELOW;
    let tall = area.height >= TALL_FROM;
    let margin = if compact { 1 } else { 2 };
    let inner = Rect {
        x: area.x + margin,
        width: area.width.saturating_sub(margin * 2),
        ..area
    };
    let mut cursor = Cursor::new(inner);

    // c-running-head: three slots, folio = note count for the screen it is over.
    let folio = match model.screen {
        Screen::NowPlaying => format!("{} notes", model.notes_for_displayed_chord().len()),
        Screen::AllNotes => format!("{} notes", model.total_note_count()),
    };
    let head = cursor.take(push::RUNNING_HEAD_HEIGHT);
    push::running_head(
        &["watchord", "chord displayer", model.screen.title()],
        &folio,
        head,
        buf,
    );

    head_band(model, compact, &mut cursor, buf);

    if let Some(status) = model.status_message() {
        error_row(status, &mut cursor, buf);
    }

    let tabs = cursor.take(push::tabs_height(compact));
    let selected_tab = Screen::ALL
        .iter()
        .position(|s| *s == model.screen)
        .unwrap_or(0);
    let tab_rects = push::tabs(
        &[Screen::NowPlaying.title(), Screen::AllNotes.title()],
        selected_tab,
        compact,
        tabs,
        buf,
    );
    hits.tabs = tab_rects.into_iter().zip(Screen::ALL).collect();
    if tall {
        cursor.skip(1);
    }

    // The key reference is anchored at the foot, under everything.
    let help = cursor.take_bottom(1);
    Paragraph::new(Line::from(push::meta(KEYS_HELP))).render(help, buf);

    let cursor = match model.screen {
        Screen::NowPlaying => now_playing(model, ui, compact, tall, cursor, buf, &mut hits),
        Screen::AllNotes => {
            all_notes(model, ui, compact, cursor, buf, &mut hits);
            None
        }
    };
    Drawn { cursor, hits }
}

/// A top-down allocator over one column of rows, with a bottom edge that
/// anchored blocks can take from.
struct Cursor {
    area: Rect,
    top: u16,
    bottom: u16,
}

impl Cursor {
    fn new(area: Rect) -> Self {
        Cursor {
            area,
            top: area.y,
            bottom: area.y + area.height,
        }
    }

    fn remaining(&self) -> u16 {
        self.bottom.saturating_sub(self.top)
    }

    /// The next `rows` rows, or as many as are left.
    fn take(&mut self, rows: u16) -> Rect {
        let rows = rows.min(self.remaining());
        let rect = Rect {
            y: self.top,
            height: rows,
            ..self.area
        };
        self.top += rows;
        rect
    }

    /// The last `rows` rows, or as many as are left.
    fn take_bottom(&mut self, rows: u16) -> Rect {
        let rows = rows.min(self.remaining());
        self.bottom -= rows;
        Rect {
            y: self.bottom,
            height: rows,
            ..self.area
        }
    }

    fn skip(&mut self, rows: u16) {
        self.top += rows.min(self.remaining());
    }
}

// MARK: - Chrome

/// The head band: the INPUT and STATE plates. Both read live state.
fn head_band(model: &AppModel, compact: bool, cursor: &mut Cursor, buf: &mut Buffer) {
    let input_tone = if model.banner().is_some() {
        PlateTone::Signal4
    } else if model.has_no_input() {
        PlateTone::Signal2
    } else {
        PlateTone::Accent
    };
    let input = Plate::new(model.input_label())
        .title("input")
        .tone(input_tone);
    let state_label = if model.displayed().is_none() {
        "idle"
    } else if model.is_released() {
        "released"
    } else {
        "held"
    };
    let state = Plate::new(state_label).title("state");

    let height = if compact { 1 } else { Plate::HEIGHT };
    let band = cursor.take(height);
    let mut x = band.x;
    for plate in [&input, &state] {
        let width = if compact {
            plate.compact_width()
        } else {
            plate.width()
        };
        let at = Rect {
            x,
            width: width.min((band.x + band.width).saturating_sub(x)),
            ..band
        };
        if compact {
            plate.render_compact(at, buf);
        } else {
            plate.render(at, buf);
        }
        x += width + 2;
    }
    if !compact {
        let rule = cursor.take(1);
        push::rule(RuleWeight::Hairline, rule, buf);
    }
}

/// The error surface: the danger pair on the chip, `danger_text` on the
/// message, on a row marked in the danger ink.
fn error_row(status: &str, cursor: &mut Cursor, buf: &mut Buffer) {
    let area = cursor.take(1);
    let block = push::row(push::DANGER_TEXT);
    let inner = block.inner(area);
    block.render(area, buf);
    let chip = Plate::new("error")
        .tone(PlateTone::Danger)
        .variant(PlateVariant::Solid);
    let chip_width = chip.compact_width();
    chip.render_compact(inner, buf);
    let message = Rect {
        x: inner.x + chip_width + 1,
        width: inner.width.saturating_sub(chip_width + 1),
        ..inner
    };
    Paragraph::new(Line::from(Span::styled(
        status.to_string(),
        Style::default().fg(push::DANGER_TEXT.color()),
    )))
    .render(message, buf);
}

// MARK: - Now Playing

fn now_playing(
    model: &AppModel,
    ui: &UiState,
    compact: bool,
    tall: bool,
    mut cursor: Cursor,
    buf: &mut Buffer,
    hits: &mut Hits,
) -> Option<(u16, u16)> {
    // The note field is anchored at the foot, above the key reference, so it is
    // never the thing a short terminal clips.
    let hint = if model.note_target_key().is_none() {
        1
    } else {
        0
    };
    let field_area = {
        let block = cursor.take_bottom(1 + hint);
        cursor.take_bottom(1);
        block
    };

    display(model, compact, tall, &mut cursor, buf);
    if !compact {
        cursor.skip(1);
    }
    alternates(model, ui, compact, &mut cursor, buf, hits);
    particulars(model, compact, &mut cursor, buf);
    notes_on_this_chord(model, ui, compact, &mut cursor, buf, hits);

    hits.field = Some(Rect {
        height: 1.min(field_area.height),
        ..field_area
    });
    note_field(model, field_area, buf)
}

/// The display panel: the headline figure with `≈` beside it, the spoken name,
/// the fit note, the decline sentence, the `released` plate, and the waiting
/// line. The one panel on the screen that is live.
fn display(model: &AppModel, compact: bool, tall: bool, cursor: &mut Cursor, buf: &mut Buffer) {
    let text = model.headline_text();
    let figure_text = match model.headline_approximation() {
        Some(mark) => format!("{text} {mark}"),
        None => text,
    };
    let spoken = model.headline_spoken();
    let fit = model.headline_fit_note();
    let decline = model.decline_reason();
    let released = model.is_released();
    let waiting = model.displayed().is_none();

    let pad = if compact { 0 } else { 1 };
    let block = push::panel();
    let probe = block.inner(cursor.take(0));
    let inner_width = probe.width.saturating_sub(2);
    let figure_width = figure::width(&figure_text) as u16;
    let figure_height = if tall && figure_width <= inner_width {
        figure::TALL_HEIGHT
    } else if figure_width <= inner_width {
        figure::SHORT_HEIGHT
    } else {
        1
    };
    let plate = Plate::new("released").tone(PlateTone::Signal4);
    let plate_height = if compact { 1 } else { Plate::HEIGHT };
    let content_rows = figure_height
        + spoken.is_some() as u16
        + fit.is_some() as u16
        + decline.is_some() as u16
        + if released { plate_height } else { 0 }
        + waiting as u16;
    let height = content_rows + 2 + pad * 2;

    let area = cursor.take(height);
    let inner = block.inner(area);
    block.render(area, buf);
    let mut y = inner.y + pad;
    let centre = |width: u16| inner.x + 1 + inner_width.saturating_sub(width) / 2;

    let headline_style = Style::default().fg(push::FOREGROUND.color());
    let figure_lines = match figure_height {
        figure::TALL_HEIGHT => figure::tall(&figure_text, headline_style),
        figure::SHORT_HEIGHT => figure::short(&figure_text, headline_style),
        _ => vec![Line::from(push::content(&figure_text))],
    };
    let width = figure_lines
        .first()
        .map_or(0, |line| line.width() as u16)
        .min(inner_width);
    Paragraph::new(figure_lines).render(
        Rect {
            x: centre(width),
            y,
            width,
            height: figure_height,
        },
        buf,
    );
    y += figure_height;

    let centred = |line: Line<'static>, y: &mut u16, buf: &mut Buffer| {
        Paragraph::new(line).alignment(Alignment::Center).render(
            Rect {
                y: *y,
                height: 1,
                ..inner
            },
            buf,
        );
        *y += 1;
    };
    if let Some(spoken) = &spoken {
        centred(Line::from(push::quiet(spoken)), &mut y, buf);
    }
    if let Some(fit) = &fit {
        centred(Line::from(push::meta(fit)), &mut y, buf);
    }
    if let Some(decline) = &decline {
        centred(Line::from(push::quiet(decline)), &mut y, buf);
    }
    if released {
        let width = if compact {
            plate.compact_width()
        } else {
            plate.width()
        };
        let at = Rect {
            x: centre(width),
            y,
            width,
            height: plate_height,
        };
        if compact {
            plate.render_compact(at, buf);
        } else {
            plate.render(at, buf);
        }
        y += plate_height;
    }
    if waiting {
        centred(Line::from(push::quiet("play something")), &mut y, buf);
    }
}

fn axis_label(origin: SpellingOrigin) -> &'static str {
    match origin {
        SpellingOrigin::Headline => "headline",
        SpellingOrigin::ReRooted => "re-rooted",
        SpellingOrigin::Enharmonic => "enharmonic",
    }
}

const ALTERNATE_COLUMNS: [Column; 4] = [
    Column::new("reading", Constraint::Length(18)),
    Column::new("axis", Constraint::Length(11)),
    Column::new("fit", Constraint::Length(12)),
    Column::new("said aloud", Constraint::Fill(1)),
];

/// Section 01: a real table of the readings beneath the headline.
fn alternates(
    model: &AppModel,
    ui: &UiState,
    compact: bool,
    cursor: &mut Cursor,
    buf: &mut Buffer,
    hits: &mut Hits,
) {
    let readings = model.alternates();
    if readings.is_empty() {
        return;
    }
    let head = cursor.take(push::section_head_height(compact));
    push::section_head(
        "01",
        "Alternate readings",
        &readings.len().to_string(),
        compact,
        head,
        buf,
    );
    let rows: Vec<TableRow> = readings
        .iter()
        .map(|reading| {
            let mut name = vec![push::content(&reading.name)];
            if let Some(mark) = &reading.approximation {
                name.push(Span::raw(" "));
                name.push(push::content(mark));
            }
            TableRow::Cells(vec![
                Line::from(name),
                Line::from(push::meta(axis_label(reading.origin))),
                Line::from(push::meta(reading.fit_detail.as_deref().unwrap_or(""))),
                Line::from(push::quiet(&reading.spoken)),
            ])
        })
        .collect();
    let row_count = rows.len();
    let sections = [Section { label: None, rows }];
    let table = Table {
        columns: &ALTERNATE_COLUMNS,
        sections: &sections,
        head_rule: RuleWeight::Rule,
        first_row: ui.scroll.alternates.min(row_count.saturating_sub(1)),
        selected: None,
    };
    let area = cursor.take(table.full_height());
    table.render(area, buf);
    hits.scroll_areas.push((area, ScrollTarget::Alternates));
    if !compact {
        cursor.skip(1);
    }
}

/// The keys cluster: one term, one description, on a panel.
fn particulars(model: &AppModel, compact: bool, cursor: &mut Cursor, buf: &mut Buffer) {
    let keys = model.keys_row();
    if keys.is_empty() {
        return;
    }
    let line = push::definition("keys", &keys);
    if compact {
        let area = cursor.take(1);
        Paragraph::new(line).render(
            Rect {
                x: area.x + 1,
                width: area.width.saturating_sub(1),
                ..area
            },
            buf,
        );
    } else {
        let block = push::panel();
        let area = cursor.take(3);
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(line).render(
            Rect {
                x: inner.x + 1,
                width: inner.width.saturating_sub(1),
                ..inner
            },
            buf,
        );
        cursor.skip(1);
    }
}

const NOTE_COLUMNS: [Column; 2] = [
    Column::new("note", Constraint::Fill(1)),
    Column::new("", Constraint::Length(8)).right(),
];

/// Section 02: the notes on the displayed chord, with a `delete` per row.
fn notes_on_this_chord(
    model: &AppModel,
    ui: &UiState,
    compact: bool,
    cursor: &mut Cursor,
    buf: &mut Buffer,
    hits: &mut Hits,
) {
    let notes = model.notes_for_displayed_chord();
    let head = cursor.take(push::section_head_height(compact));
    push::section_head(
        "02",
        "Notes on this chord",
        &notes.len().to_string(),
        compact,
        head,
        buf,
    );
    if notes.is_empty() {
        let area = cursor.take(1);
        Paragraph::new(Line::from(vec![Span::raw("  "), push::quiet("none yet")]))
            .render(area, buf);
        return;
    }
    let selected = ui.selected_now_playing;
    let rows: Vec<TableRow> = notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            TableRow::Cells(vec![
                Line::from(push::content(&note.text)),
                Line::from(push::ghost_button("delete", selected == Some(index))),
            ])
        })
        .collect();
    let sections = [Section { label: None, rows }];
    let visible = cursor.remaining();
    let table = Table {
        columns: &NOTE_COLUMNS,
        sections: &sections,
        head_rule: RuleWeight::Rule,
        first_row: scroll_to(selected, visible.saturating_sub(2) as usize)
            .max(ui.scroll.notes)
            .min(notes.len().saturating_sub(1)),
        selected,
    };
    let area = cursor.take(table.full_height());
    let drawn = table.render(area, buf);
    hits.scroll_areas.push((area, ScrollTarget::Notes));
    for row in drawn {
        hits.note_rows.push((row.area, row.index));
        if let Some(cell) = row.cells.last() {
            hits.delete_cells.push((*cell, row.index));
        }
    }
}

/// The first row to draw so that `selected` is inside `visible` rows.
fn scroll_to(selected: Option<usize>, visible: usize) -> usize {
    match selected {
        Some(index) if visible > 0 && index >= visible => index + 1 - visible,
        _ => 0,
    }
}

/// The always-present note field, marked at its start like a row. Returns the
/// cursor position inside it.
fn note_field(model: &AppModel, area: Rect, buf: &mut Buffer) -> Option<(u16, u16)> {
    if area.height == 0 {
        return None;
    }
    let enabled = model.note_target_key().is_some();
    let ink = if enabled { push::RING } else { push::HAIRLINE };
    let field_area = Rect { height: 1, ..area };
    let block = push::field(ink);
    let inner = block.inner(field_area);
    block.render(field_area, buf);
    let draft = &model.draft_note_text;
    let line = if draft.is_empty() {
        Line::from(push::quiet("add a note…"))
    } else {
        Line::from(push::content(draft))
    };
    Paragraph::new(line).render(inner, buf);
    if area.height > 1 && !enabled {
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            push::quiet("play a chord to write a note against it"),
        ]))
        .render(
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
            buf,
        );
    }
    if enabled {
        let x = inner.x + (draft.width() as u16).min(inner.width.saturating_sub(1));
        Some((x, inner.y))
    } else {
        None
    }
}

// MARK: - All Notes

const ALL_NOTE_COLUMNS: [Column; 4] = [
    Column::new("note", Constraint::Fill(1)),
    Column::new("written as", Constraint::Length(12)),
    Column::new("when", Constraint::Length(16)),
    Column::new("", Constraint::Length(8)).right(),
];

/// Every note, grouped under the chord it belongs to. The gutter carries the
/// chord's headline as the engine names it today; the subhead is the stored
/// key.
fn all_notes(
    model: &AppModel,
    ui: &UiState,
    compact: bool,
    mut cursor: Cursor,
    buf: &mut Buffer,
    hits: &mut Hits,
) {
    let groups = model.note_groups();
    if groups.is_empty() {
        let area = cursor.take(1);
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            push::quiet("no notes yet"),
        ]))
        .render(area, buf);
        return;
    }
    let head = cursor.take(push::section_head_height(compact));
    push::section_head(
        "§",
        "Every note",
        &format!("{} · {} chords", model.total_note_count(), groups.len()),
        compact,
        head,
        buf,
    );

    // The selection is an ordinal over notes; the table counts subheads too.
    let selected_note = ui.selected_all_notes;
    let mut note_ordinal = 0usize;
    let mut table_row = 0usize;
    let mut selected_row = None;
    // Table row index -> note ordinal, for the rows that are notes.
    let mut ordinal_of_row: Vec<Option<usize>> = Vec::new();
    let sections: Vec<Section> = groups
        .iter()
        .map(|group| {
            let mut rows = vec![TableRow::Subhead(group.key.raw().to_string())];
            ordinal_of_row.push(None);
            table_row += 1;
            for note in &group.notes {
                let is_selected = selected_note == Some(note_ordinal);
                if is_selected {
                    selected_row = Some(table_row);
                }
                ordinal_of_row.push(Some(note_ordinal));
                rows.push(TableRow::Cells(vec![
                    Line::from(push::content(&note.text)),
                    Line::from(push::meta(&note.spelling_when_written)),
                    Line::from(push::content(&when::format(note.created_at))),
                    Line::from(push::ghost_button("delete", is_selected)),
                ]));
                note_ordinal += 1;
                table_row += 1;
            }
            Section {
                label: Some(group.heading.clone()),
                rows,
            }
        })
        .collect();
    let visible = cursor.remaining();
    let head_rule = if compact {
        RuleWeight::Rule
    } else {
        RuleWeight::Display
    };
    let table = Table {
        columns: &ALL_NOTE_COLUMNS,
        sections: &sections,
        head_rule,
        first_row: scroll_to(
            selected_row,
            visible.saturating_sub(1 + head_rule.height()) as usize,
        )
        .max(ui.scroll.all_notes)
        .min(table_row.saturating_sub(1)),
        selected: selected_row,
    };
    let area = cursor.take(table.full_height());
    let drawn = table.render(area, buf);
    hits.scroll_areas.push((area, ScrollTarget::AllNotes));
    for row in drawn {
        let Some(Some(ordinal)) = ordinal_of_row.get(row.index) else {
            continue;
        };
        hits.note_rows.push((row.area, *ordinal));
        if let Some(cell) = row.cells.last() {
            hits.delete_cells.push((*cell, *ordinal));
        }
    }
}

/// How many notes the keys can select on `screen`.
pub fn selectable_count(model: &AppModel, screen: Screen) -> usize {
    match screen {
        Screen::NowPlaying => model.notes_for_displayed_chord().len(),
        Screen::AllNotes => model.total_note_count(),
    }
}

/// The id of the note the keys have selected on `screen`, if any.
pub fn selected_note_id(model: &AppModel, ui: &UiState, screen: Screen) -> Option<String> {
    let index = ui.selected(screen)?;
    match screen {
        Screen::NowPlaying => model
            .notes_for_displayed_chord()
            .get(index)
            .map(|n| n.id.clone()),
        Screen::AllNotes => model
            .note_groups()
            .iter()
            .flat_map(|g| g.notes.iter())
            .nth(index)
            .map(|n| n.id.clone()),
    }
}
