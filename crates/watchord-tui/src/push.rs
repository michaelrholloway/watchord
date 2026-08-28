//! PUSH, drawn in a terminal.
//!
//! Every token below is the hex value from note-view's `Style.swift`, which is
//! itself a copy of `rnrh-ui@2af4dc7`'s `:root[data-theme='push']` block. The
//! names are PUSH's names — `accent_background` is `--color-accent-background`
//! — so this file can be checked line by line against the source.
//!
//! **This is the only module that draws a rule or a box.** A screen applies a
//! structural role — [`row`], [`cell`], [`panel`], [`frame`], [`field`],
//! [`control`] — and the role decides what it draws. On PUSH a row is a bar
//! marked at its start; on another skin the same call would draw a rule beneath
//! or a box around. ADR-0003 lists the terminal departures.
//!
//! The mono/sans rule becomes colour + case (ADR-0003 (a)): [`meta`] is
//! `muted_foreground` and UPPERCASE, content is `foreground` and normal case.

use std::sync::OnceLock;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

// MARK: - Tokens

/// One PUSH colour: its 24-bit value, and the 256-colour index that stands in
/// for it on a terminal without truecolor (ADR-0003 (c)). The index is chosen
/// once here, never at a call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    /// The upstream hex value, `0xRRGGBB`.
    pub hex: u32,
    /// The nearest xterm-256 index.
    pub fallback: u8,
}

impl Token {
    const fn new(hex: u32, fallback: u8) -> Self {
        Token { hex, fallback }
    }

    /// The ratatui colour for this token on the terminal the app is running in.
    pub fn color(self) -> Color {
        if truecolor() {
            let [_, r, g, b] = self.hex.to_be_bytes();
            Color::Rgb(r, g, b)
        } else {
            Color::Indexed(self.fallback)
        }
    }
}

/// True when the terminal announces 24-bit colour through `COLORTERM`. Read
/// once per process. Tests set nothing and get the fallback, which is also a
/// valid rendering.
pub fn truecolor() -> bool {
    static TRUECOLOR: OnceLock<bool> = OnceLock::new();
    *TRUECOLOR.get_or_init(|| {
        std::env::var("COLORTERM")
            .map(|value| {
                let value = value.to_ascii_lowercase();
                value.contains("truecolor") || value.contains("24bit")
            })
            .unwrap_or(false)
    })
}

/// `--color-background`
pub const BACKGROUND: Token = Token::new(0x000000, 16);
/// `--color-foreground`
pub const FOREGROUND: Token = Token::new(0xE9EDE7, 255);
/// `--color-raised-background` — a plate lifted off the ground.
pub const RAISED_BACKGROUND: Token = Token::new(0x0B0F0B, 232);
/// `--color-raised-foreground`
pub const RAISED_FOREGROUND: Token = Token::new(0xE9EDE7, 255);
/// `--color-sunken-background`
pub const SUNKEN_BACKGROUND: Token = Token::new(0x000000, 16);
/// `--color-sunken-foreground`
pub const SUNKEN_FOREGROUND: Token = Token::new(0xE9EDE7, 255);
/// `--color-muted-background`
pub const MUTED_BACKGROUND: Token = Token::new(0x151A14, 234);
/// `--color-muted-foreground` — secondary text.
pub const MUTED_FOREGROUND: Token = Token::new(0x9AA896, 246);
/// `--color-inverse-background`
pub const INVERSE_BACKGROUND: Token = Token::new(0xE9EDE7, 255);
/// `--color-inverse-foreground`
pub const INVERSE_FOREGROUND: Token = Token::new(0x000000, 16);

/// `--color-accent-background`. The one hue; on PUSH used as a field far more
/// than as a mark.
pub const ACCENT_BACKGROUND: Token = Token::new(0xC6F000, 190);
/// `--color-accent-foreground` — what reads ON the acid field.
pub const ACCENT_FOREGROUND: Token = Token::new(0x000000, 16);
/// `--color-accent-quiet`
pub const ACCENT_QUIET: Token = Token::new(0x2B3300, 58);
/// `--color-accent-text` — the acid as an ink on the page ground.
pub const ACCENT_TEXT: Token = Token::new(0xC6F000, 190);
/// `--color-mark-background`
pub const MARK_BACKGROUND: Token = Token::new(0xC6F000, 190);
/// `--color-mark-foreground`
pub const MARK_FOREGROUND: Token = Token::new(0x000000, 16);

/// `--color-signal-1`
pub const SIGNAL_1: Token = Token::new(0xC6F000, 190);
/// `--color-signal-2`
pub const SIGNAL_2: Token = Token::new(0xFF0055, 197);
/// `--color-signal-3`
pub const SIGNAL_3: Token = Token::new(0x00E5FF, 45);
/// `--color-signal-4`
pub const SIGNAL_4: Token = Token::new(0xFFB000, 214);
/// The ink every signal fill carries.
pub const SIGNAL_FOREGROUND: Token = Token::new(0x000000, 16);

/// `--color-danger-background` — a fill, for a chip.
pub const DANGER_BACKGROUND: Token = Token::new(0xFF0033, 197);
/// `--color-danger-foreground`
pub const DANGER_FOREGROUND: Token = Token::new(0x000000, 16);
/// `--color-danger-text` — an ink, for a message.
pub const DANGER_TEXT: Token = Token::new(0xFF566F, 204);

/// `--color-hairline` — the structural stroke colour.
pub const HAIRLINE: Token = Token::new(0x6B776A, 243);
/// `--color-rule` — a drawn divider.
pub const RULE: Token = Token::new(0x9AA898, 246);
/// `--color-edge` — the brightest line.
pub const EDGE: Token = Token::new(0xE9EDE7, 255);
/// `--color-ring` — focus.
pub const RING: Token = Token::new(0xC6F000, 190);

// MARK: - Text registers

/// Metadata: labels, column heads, timestamps, statuses. Muted and UPPERCASE.
pub fn meta(text: &str) -> Span<'static> {
    Span::styled(
        text.to_uppercase(),
        Style::default().fg(MUTED_FOREGROUND.color()),
    )
}

/// Metadata in a colour of its own — a plate's ink, a folio in the foreground.
/// Still UPPERCASE: the case is what says it is metadata.
pub fn meta_in(text: &str, ink: Token) -> Span<'static> {
    Span::styled(text.to_uppercase(), Style::default().fg(ink.color()))
}

/// Content: the headline, readings, note text. Full ink, normal case.
pub fn content(text: &str) -> Span<'static> {
    Span::styled(text.to_string(), Style::default().fg(FOREGROUND.color()))
}

/// Secondary running copy — a spoken name, a decline sentence. Muted, normal
/// case: it is still content, only quieter.
pub fn quiet(text: &str) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().fg(MUTED_FOREGROUND.color()),
    )
}

/// The page ground and its ink, for the whole frame.
pub fn page() -> Style {
    Style::default()
        .bg(BACKGROUND.color())
        .fg(FOREGROUND.color())
}

// MARK: - Structural roles
//
// PUSH's `--stroke-*` tokens: `row` and `field` are `0 0 0 2px`, a leading
// edge and nothing else; the other four are `1px`, four edges. Radius does not
// exist in a terminal (ADR-0003 (b)).

/// The 2-column leading bar that `0 0 0 2px` resolves to.
const BAR: border::Set = border::Set {
    vertical_left: border::QUADRANT_LEFT_HALF,
    ..border::PLAIN
};

fn leading_bar(ink: Token) -> Block<'static> {
    Block::new()
        .borders(Borders::LEFT)
        .border_set(BAR)
        .border_style(Style::default().fg(ink.color()))
        .padding(Padding::left(1))
}

fn light_box(ink: Token) -> Block<'static> {
    Block::new()
        .borders(Borders::ALL)
        .border_set(border::PLAIN)
        .border_style(Style::default().fg(ink.color()))
}

/// `--stroke-row: 0 0 0 2px` — a row is a bar, marked at its start.
pub fn row(ink: Token) -> Block<'static> {
    leading_bar(ink)
}

/// `--stroke-cell: 1px`
pub fn cell() -> Block<'static> {
    light_box(HAIRLINE)
}

/// `--stroke-panel: 1px`, on the raised ground.
pub fn panel() -> Block<'static> {
    light_box(HAIRLINE).style(
        Style::default()
            .bg(RAISED_BACKGROUND.color())
            .fg(RAISED_FOREGROUND.color()),
    )
}

/// `--stroke-frame: 1px`
pub fn frame() -> Block<'static> {
    light_box(HAIRLINE)
}

/// `--stroke-field: 0 0 0 2px` — a field is marked at its start, like a row.
/// The ink is the ring on focus and the hairline otherwise.
pub fn field(ink: Token) -> Block<'static> {
    leading_bar(ink).style(Style::default().bg(RAISED_BACKGROUND.color()))
}

/// `--stroke-control: 1px`, in the ink the control asks for — a button takes
/// the hairline, a plate takes its tone.
pub fn control(ink: Token) -> Block<'static> {
    light_box(ink)
}

// MARK: - p-rule

/// PUSH's rule weights. `Display` is the Swiss section marker: a heavy bar
/// over a hairline, two rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleWeight {
    Hairline,
    Rule,
    Display,
}

impl RuleWeight {
    /// How many rows the rule takes.
    pub fn height(self) -> u16 {
        match self {
            RuleWeight::Hairline | RuleWeight::Rule => 1,
            RuleWeight::Display => 2,
        }
    }
}

/// Draws a rule across `area`. The only place a horizontal line is written.
pub fn rule(weight: RuleWeight, area: Rect, buf: &mut Buffer) {
    let width = area.width as usize;
    let lines: Vec<Line<'static>> = match weight {
        RuleWeight::Hairline => vec![Line::styled(
            "─".repeat(width),
            Style::default().fg(HAIRLINE.color()),
        )],
        RuleWeight::Rule => vec![Line::styled(
            "─".repeat(width),
            Style::default().fg(RULE.color()),
        )],
        RuleWeight::Display => vec![
            Line::styled("━".repeat(width), Style::default().fg(EDGE.color())),
            Line::styled("─".repeat(width), Style::default().fg(HAIRLINE.color())),
        ],
    };
    Paragraph::new(lines).render(area, buf);
}

// MARK: - p-plate

/// Tones set both halves of a pair plus what the tone is as ink on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateTone {
    Ink,
    Accent,
    Signal1,
    Signal2,
    Signal3,
    Signal4,
    Danger,
}

impl PlateTone {
    /// The tone's fill.
    pub fn plate(self) -> Token {
        match self {
            PlateTone::Ink => RULE,
            PlateTone::Accent => ACCENT_BACKGROUND,
            PlateTone::Signal1 => SIGNAL_1,
            PlateTone::Signal2 => SIGNAL_2,
            PlateTone::Signal3 => SIGNAL_3,
            PlateTone::Signal4 => SIGNAL_4,
            PlateTone::Danger => DANGER_BACKGROUND,
        }
    }

    /// What reads ON that fill.
    pub fn on(self) -> Token {
        match self {
            PlateTone::Ink => BACKGROUND,
            PlateTone::Accent => ACCENT_FOREGROUND,
            PlateTone::Signal1 | PlateTone::Signal2 | PlateTone::Signal3 | PlateTone::Signal4 => {
                SIGNAL_FOREGROUND
            }
            PlateTone::Danger => DANGER_FOREGROUND,
        }
    }

    /// What the tone looks like as ink on the page ground. The signals have no
    /// legible ink form, so they keep the tone on the border and set the label
    /// in the page foreground.
    pub fn ink(self) -> Token {
        match self {
            PlateTone::Ink => RULE,
            PlateTone::Accent => ACCENT_TEXT,
            PlateTone::Signal1 | PlateTone::Signal2 | PlateTone::Signal3 | PlateTone::Signal4 => {
                FOREGROUND
            }
            PlateTone::Danger => DANGER_TEXT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateVariant {
    Outline,
    Solid,
}

/// `p-plate` — an outlined label plate. Non-interactive: it names a thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plate {
    pub label: String,
    /// The small line above the value. Drawn in the plate's top edge, which is
    /// the terminal's reading of "on its own rule".
    pub title: Option<String>,
    pub tone: PlateTone,
    pub variant: PlateVariant,
}

impl Plate {
    pub fn new(label: impl Into<String>) -> Self {
        Plate {
            label: label.into(),
            title: None,
            tone: PlateTone::Ink,
            variant: PlateVariant::Outline,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn tone(mut self, tone: PlateTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn variant(mut self, variant: PlateVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The columns this plate takes, drawn in full.
    pub fn width(&self) -> u16 {
        let label = self.label.width() + 2;
        let title = self.title.as_deref().map_or(0, |t| t.width() + 4);
        (label.max(title) + 2) as u16
    }

    /// The rows this plate takes, drawn in full: the box plus its label.
    pub const HEIGHT: u16 = 3;

    /// The columns this plate takes in one row: `[ TITLE LABEL ]`.
    pub fn compact_width(&self) -> u16 {
        let title = self.title.as_deref().map_or(0, |t| t.width() + 1);
        (title + self.label.width() + 4) as u16
    }

    fn ink(&self) -> Token {
        match self.variant {
            PlateVariant::Solid => self.tone.on(),
            PlateVariant::Outline => self.tone.ink(),
        }
    }

    fn fill(&self) -> Style {
        match self.variant {
            PlateVariant::Solid => Style::default().bg(self.tone.plate().color()),
            PlateVariant::Outline => Style::default(),
        }
    }

    /// Draws the plate at `area`'s origin, `HEIGHT` rows tall.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = Rect {
            width: area.width.min(self.width()),
            height: area.height.min(Self::HEIGHT),
            ..area
        };
        let mut block = control(self.tone.plate()).style(self.fill());
        if let Some(title) = &self.title {
            block = block.title_top(Line::from(vec![
                Span::raw(" "),
                meta_in(title, self.ink()),
                Span::raw(" "),
            ]));
        }
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            meta_in(&self.label, self.ink()),
        ]))
        .render(inner, buf);
    }

    /// The plate in one row, for a short terminal: the box collapses to a pair
    /// of brackets in the tone, `[ INPUT FAKE ]`, so it still reads as a plate
    /// and not as a line of text. Nothing it reports changes.
    pub fn render_compact(&self, area: Rect, buf: &mut Buffer) {
        let area = Rect {
            width: area.width.min(self.compact_width()),
            height: area.height.min(1),
            ..area
        };
        let bracket = Style::default().fg(self.tone.plate().color());
        let mut spans = vec![Span::styled("[", bracket), Span::raw(" ")];
        if let Some(title) = &self.title {
            spans.push(meta_in(title, self.ink()).add_modifier(Modifier::DIM));
            spans.push(Span::raw(" "));
        }
        spans.push(meta_in(&self.label, self.ink()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled("]", bracket));
        Paragraph::new(Line::from(spans))
            .style(self.fill())
            .render(area, buf);
    }
}

// MARK: - p-button

/// `p-button`, ghost variant — the only variant this app calls. A word in the
/// muted ink; the foreground when it is the selected row's, so a keyboard user
/// can see which `delete` the `d` key would press.
pub fn ghost_button(label: &str, selected: bool) -> Span<'static> {
    if selected {
        Span::styled(
            label.to_uppercase(),
            Style::default()
                .fg(ACCENT_FOREGROUND.color())
                .bg(ACCENT_BACKGROUND.color()),
        )
    } else {
        meta(label)
    }
}

// MARK: - p-tabs

/// `p-tabs`, underline variant: the selected tab is a label in the foreground
/// with the accent indicator beneath it; the others are muted. One row when
/// `compact`, where the indicator moves to the label's leading edge. Never a
/// fill — the acid is a field for data, not for chrome. Returns one rect per
/// tab for hit-testing a click.
/// The accent mark before a selected tab's label in the one-row form.
const INDICATOR: &str = "▌";

pub fn tabs(
    items: &[&str],
    selected: usize,
    compact: bool,
    area: Rect,
    buf: &mut Buffer,
) -> Vec<Rect> {
    let mut label_spans: Vec<Span<'static>> = Vec::new();
    let mut underline_spans: Vec<Span<'static>> = Vec::new();
    let mut rects = Vec::with_capacity(items.len());
    let mut x = area.x;
    for (index, item) in items.iter().enumerate() {
        let is_selected = index == selected;
        let text = if compact && is_selected {
            format!("{}{} ", INDICATOR, item.to_uppercase())
        } else {
            format!(" {} ", item.to_uppercase())
        };
        let width = text.width();
        rects.push(Rect {
            x,
            width: (width as u16).min((area.x + area.width).saturating_sub(x)),
            ..area
        });
        x += width as u16 + 2;
        if is_selected {
            if compact {
                label_spans.push(Span::styled(
                    INDICATOR.to_string(),
                    Style::default().fg(ACCENT_BACKGROUND.color()),
                ));
                label_spans.push(Span::styled(
                    text[INDICATOR.len()..].to_string(),
                    Style::default().fg(FOREGROUND.color()),
                ));
            } else {
                label_spans.push(Span::styled(text, Style::default().fg(FOREGROUND.color())));
            }
        } else {
            label_spans.push(Span::styled(
                text,
                Style::default().fg(MUTED_FOREGROUND.color()),
            ));
        }
        label_spans.push(Span::raw("  "));
        let underline = if is_selected { "━" } else { "─" }.repeat(width);
        let ink = if is_selected {
            ACCENT_BACKGROUND
        } else {
            HAIRLINE
        };
        underline_spans.push(Span::styled(underline, Style::default().fg(ink.color())));
        underline_spans.push(Span::styled("──", Style::default().fg(HAIRLINE.color())));
    }
    let mut lines = vec![Line::from(label_spans)];
    if !compact {
        lines.push(Line::from(underline_spans));
    }
    Paragraph::new(lines).render(area, buf);
    rects
}

/// Rows `tabs` takes.
pub fn tabs_height(compact: bool) -> u16 {
    if compact { 1 } else { 2 }
}

// MARK: - c-running-head

/// `c-running-head` — small uppercase labels on equal tracks, a folio hard
/// right, a rule beneath. Two rows.
pub fn running_head(slots: &[&str], folio: &str, area: Rect, buf: &mut Buffer) {
    let folio_width = folio.width() as u16 + 1;
    let track_width = area.width.saturating_sub(folio_width) / slots.len().max(1) as u16;
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (index, slot) in slots.iter().enumerate() {
        let mut text = slot.to_uppercase();
        let width = track_width.saturating_sub(1) as usize;
        if text.width() > width {
            text = text
                .chars()
                .take(width.saturating_sub(1))
                .collect::<String>()
                + "…";
        }
        let padding = (track_width as usize).saturating_sub(text.width());
        let ink = if index == 0 {
            FOREGROUND
        } else {
            MUTED_FOREGROUND
        };
        spans.push(Span::styled(text, Style::default().fg(ink.color())));
        spans.push(Span::raw(" ".repeat(padding)));
    }
    let head = Rect { height: 1, ..area };
    Paragraph::new(Line::from(spans)).render(head, buf);
    Paragraph::new(Line::from(meta_in(folio, FOREGROUND)))
        .alignment(Alignment::Right)
        .render(head, buf);
    if area.height > 1 {
        rule(
            RuleWeight::Rule,
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
            buf,
        );
    }
}

/// Rows `running_head` takes.
pub const RUNNING_HEAD_HEIGHT: u16 = 2;

// MARK: - c-section-head

/// `c-section-head` — a rule, a marker in the gutter, the title, and
/// right-aligned meta. The rule is the display pair where there is room, a
/// plain rule where there is less, and `None` where the title row must stand
/// alone.
pub fn section_head(
    marker: &str,
    title: &str,
    meta_text: &str,
    head_rule: Option<RuleWeight>,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height == 0 {
        return;
    }
    let mut y = area.y;
    if let Some(weight) = head_rule {
        rule(
            weight,
            Rect {
                height: weight.height().min(area.height),
                ..area
            },
            buf,
        );
        y += weight.height();
    }
    if y >= area.y + area.height {
        return;
    }
    let line_area = Rect {
        y,
        height: 1,
        ..area
    };
    Paragraph::new(Line::from(vec![
        meta(marker),
        Span::raw("  "),
        content(title),
    ]))
    .render(line_area, buf);
    Paragraph::new(Line::from(meta(meta_text)))
        .alignment(Alignment::Right)
        .render(line_area, buf);
}

/// Rows `section_head` takes.
pub fn section_head_height(head_rule: Option<RuleWeight>) -> u16 {
    1 + head_rule.map_or(0, RuleWeight::height)
}

// MARK: - p-table

/// One column of a `p-table`: its head, its width, and whether it sets figures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub head: &'static str,
    pub width: Constraint,
    pub align: Alignment,
}

impl Column {
    pub const fn new(head: &'static str, width: Constraint) -> Self {
        Column {
            head,
            width,
            align: Alignment::Left,
        }
    }

    pub const fn right(mut self) -> Self {
        self.align = Alignment::Right;
        self
    }
}

/// One body row: its cells, already styled, plus which section it is in.
#[derive(Clone, Debug)]
pub enum TableRow {
    /// A spanning sub-head inside a section, in the mono voice — here, meta.
    Subhead(String),
    /// A row of cells, one per column.
    Cells(Vec<Line<'static>>),
}

/// A grouped row set. `label` is the gutter marker, printed once against the
/// group's first row so it reads as a bracket down the side.
#[derive(Clone, Debug)]
pub struct Section {
    pub label: Option<String>,
    pub rows: Vec<TableRow>,
}

/// One body row as it was drawn: which row, where, and where each of its
/// cells went. What a click is tested against — the rects the table laid
/// out, never a second layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawnRow {
    /// The row's index, counting every row in every section.
    pub index: usize,
    /// The whole row, bar included.
    pub area: Rect,
    /// One rect per column, at this row.
    pub cells: Vec<Rect>,
}

/// `p-table`. The head row takes `head_rule` beneath it; body rows take the
/// `row` role. Rows past `area` are clipped; `first_row` scrolls them.
pub struct Table<'a> {
    pub columns: &'a [Column],
    pub sections: &'a [Section],
    pub head_rule: RuleWeight,
    /// The index of the first body row drawn, counting every row in every
    /// section, subheads included.
    pub first_row: usize,
    /// The body row (same count) drawn as selected, if any.
    pub selected: Option<usize>,
}

impl Table<'_> {
    const GUTTER: u16 = 12;

    fn has_gutter(&self) -> bool {
        self.sections.iter().any(|s| s.label.is_some())
    }

    /// Rows the whole table takes when nothing is clipped.
    pub fn full_height(&self) -> u16 {
        let body: usize = self.sections.iter().map(|s| s.rows.len()).sum();
        1 + self.head_rule.height() + body as u16
    }

    fn column_areas(&self, area: Rect) -> Vec<Rect> {
        let constraints: Vec<Constraint> = self.columns.iter().map(|c| c.width).collect();
        ratatui::layout::Layout::horizontal(constraints)
            .spacing(2)
            .split(area)
            .to_vec()
    }

    /// Draws the table and returns every body row it drew.
    pub fn render(&self, area: Rect, buf: &mut Buffer) -> Vec<DrawnRow> {
        let mut drawn = Vec::new();
        if area.height == 0 {
            return drawn;
        }
        let gutter = if self.has_gutter() { Self::GUTTER } else { 0 };
        // Every body row carries the row role: a 2-column leading bar.
        let bar = 2;
        let columns_area = Rect {
            x: area.x + gutter + bar,
            width: area.width.saturating_sub(gutter + bar),
            ..area
        };
        let column_areas = self.column_areas(Rect {
            height: 1,
            ..columns_area
        });

        // Head.
        for (column, cell_area) in self.columns.iter().zip(&column_areas) {
            Paragraph::new(Line::from(meta(column.head)))
                .alignment(column.align)
                .render(*cell_area, buf);
        }
        let rule_area = Rect {
            y: area.y + 1,
            height: self.head_rule.height().min(area.height.saturating_sub(1)),
            ..area
        };
        rule(self.head_rule, rule_area, buf);

        // Body.
        let mut y = area.y + 1 + self.head_rule.height();
        let bottom = area.y + area.height;
        let mut index = 0usize;
        for section in self.sections {
            for (row_in_section, table_row) in section.rows.iter().enumerate() {
                let this = index;
                index += 1;
                if this < self.first_row || y >= bottom {
                    continue;
                }
                let row_area = Rect {
                    x: area.x + gutter,
                    y,
                    width: area.width.saturating_sub(gutter),
                    height: 1,
                };
                if row_in_section == 0
                    && let Some(label) = &section.label
                {
                    Paragraph::new(Line::from(content(label))).render(
                        Rect {
                            x: area.x,
                            y,
                            width: gutter.saturating_sub(1),
                            height: 1,
                        },
                        buf,
                    );
                }
                drawn.push(DrawnRow {
                    index: this,
                    area: row_area,
                    cells: column_areas.iter().map(|c| Rect { y, ..*c }).collect(),
                });
                let selected = self.selected == Some(this);
                let ink = if selected {
                    ACCENT_BACKGROUND
                } else {
                    HAIRLINE
                };
                let block = row(ink);
                let inner = block.inner(row_area);
                block.render(row_area, buf);
                match table_row {
                    TableRow::Subhead(text) => {
                        Paragraph::new(Line::from(meta(text))).render(inner, buf);
                    }
                    TableRow::Cells(cells) => {
                        for ((column, cell_area), line) in
                            self.columns.iter().zip(&column_areas).zip(cells)
                        {
                            Paragraph::new(line.clone())
                                .alignment(column.align)
                                .render(Rect { y, ..*cell_area }, buf);
                        }
                    }
                }
                y += 1;
            }
        }
        drawn
    }
}

// MARK: - p-definition-list

/// `p-definition-list`, grid layout, one item: the term in meta, the
/// description as content set as figures.
pub fn definition(term: &str, description: &str) -> Line<'static> {
    Line::from(vec![meta(term), Span::raw("  "), content(description)])
}
