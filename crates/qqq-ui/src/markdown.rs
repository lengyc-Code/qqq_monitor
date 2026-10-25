//! Markdown of notes: syntax colors for the editor, and the rendered view with headings,
//! emphasis, links, lists with task boxes, quotes, code and tables.
//!
//! The fonts have one weight, so bold text is painted a second time one physical pixel to
//! the right.

use crate::{pixel, theme};
use eframe::egui::{
    self, Color32, FontId, Pos2, Sense, Stroke, TextFormat, Vec2,
    text::{ByteIndex, CCursor, LayoutJob},
    vec2,
};
use pulldown_cmark::{Alignment, Event, Options, Parser, Tag, TagEnd};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
};

/// Color of the Markdown marks in the editor, such as `#`, `**` and list bullets.
const MARK: Color32 = Color32::from_rgb(28, 132, 152);
/// Text of inline code.
const CODE: Color32 = Color32::from_rgb(184, 36, 92);
/// Text of links: the theme's blue, dark enough for small text.
const LINK: Color32 = Color32::from_rgb(36, 98, 200);
const BODY: f32 = 14.0;
const MONO: f32 = 13.0;
/// Room for a list marker, in points.
const MARKER: f32 = 22.0;
/// Space between blocks, and between list items, in points.
const BLOCK_GAP: f32 = 10.0;
const ITEM_GAP: f32 = 4.0;

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

fn heading_size(level: u8) -> f32 {
    match level {
        1 => 22.0,
        2 => 19.0,
        3 => 17.0,
        _ => 15.0,
    }
}

/// How a run of text looks.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    /// Heading level, or 0.
    heading: u8,
    strong: bool,
    emphasis: bool,
    strike: bool,
    /// Inline code.
    code: bool,
    /// A code block.
    block: bool,
    link: bool,
    quote: bool,
    /// A Markdown mark the editor shows.
    mark: bool,
    /// A finished task.
    done: bool,
}

impl Style {
    fn format(self) -> TextFormat {
        let size = match self.heading {
            0 => BODY,
            level => heading_size(level),
        };
        let font_id = if self.code || self.block {
            FontId::monospace(size.min(MONO).max(size - 1.0))
        } else {
            FontId::proportional(size)
        };
        let color = if self.mark {
            MARK
        } else if self.code {
            CODE
        } else if self.link {
            LINK
        } else if self.quote || self.done {
            theme::MUTED
        } else {
            theme::INK
        };
        TextFormat {
            font_id,
            color,
            background: if self.code && !self.block {
                theme::WELL
            } else {
                Color32::TRANSPARENT
            },
            italics: self.emphasis,
            underline: if self.link && !self.mark {
                Stroke::new(1.0, LINK)
            } else {
                Stroke::NONE
            },
            strikethrough: if (self.strike || self.done) && !self.mark {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        }
    }

    fn bold(self) -> bool {
        (self.strong || self.heading > 0) && !self.mark
    }

    /// This style inside a block styled `base`.
    fn within(self, base: Self) -> Self {
        Self {
            heading: self.heading.max(base.heading),
            strong: self.strong || base.strong,
            quote: self.quote || base.quote,
            done: self.done || base.done,
            ..self
        }
    }
}

/// Text with a style per section.
#[derive(Default)]
struct Styled {
    text: String,
    sections: Vec<(Range<usize>, Style)>,
}

impl Styled {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        match self.sections.last_mut() {
            Some((range, last)) if *last == style => range.end = self.text.len(),
            _ => self.sections.push((start..self.text.len(), style)),
        }
    }

    /// The text laid out within `wrap`, with whether each section is bold.
    fn job(&self, wrap: f32, base: Style) -> (LayoutJob, Vec<bool>) {
        let mut job = LayoutJob {
            text: self.text.clone(),
            ..Default::default()
        };
        job.wrap.max_width = wrap;
        let mut bold = Vec::with_capacity(self.sections.len());
        for (range, style) in &self.sections {
            let style = style.within(base);
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: ByteIndex(range.start)..ByteIndex(range.end),
                format: style.format(),
            });
            bold.push(style.bold());
        }
        (job, bold)
    }
}

/// Paints the bold sections of a laid-out job a second time, shifted right by about a
/// physical pixel.
pub fn embolden(painter: &egui::Painter, job: &LayoutJob, bold: &[bool], pos: Pos2) {
    if !bold.contains(&true) {
        return;
    }
    let mut job = job.clone();
    for (section, bold) in job.sections.iter_mut().zip(bold) {
        let format = &mut section.format;
        if !bold {
            format.color = Color32::TRANSPARENT;
        }
        format.background = Color32::TRANSPARENT;
        format.underline = Stroke::NONE;
        format.strikethrough = Stroke::NONE;
    }
    let galley = painter.layout_job(job);
    let shift = pixel::unit(painter.ctx(), 0.7);
    painter.galley(pos + vec2(shift, 0.0), galley, theme::INK);
}

/// The editor's view of `text`: Markdown marks in color, headings larger, emphasis,
/// links and code styled. Returns the job without a wrap width and whether each section
/// is bold.
pub fn highlight(text: &str) -> (LayoutJob, Vec<bool>) {
    let mut styles = vec![Style::default(); text.len()];
    // Bytes the parser reports as content; the rest of a construct are its marks.
    let mut content = vec![false; text.len()];
    let mut construct = vec![false; text.len()];
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        let apply = |styles: &mut [Style], change: fn(&mut Style)| {
            styles[range.clone()].iter_mut().for_each(change)
        };
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Heading { level, .. } => {
                        let level = level as u8;
                        styles[range.clone()]
                            .iter_mut()
                            .for_each(|s| s.heading = level);
                    }
                    Tag::Strong => apply(&mut styles, |s| s.strong = true),
                    Tag::Emphasis => apply(&mut styles, |s| s.emphasis = true),
                    Tag::Strikethrough => apply(&mut styles, |s| s.strike = true),
                    Tag::Link { .. } | Tag::Image { .. } => apply(&mut styles, |s| s.link = true),
                    Tag::BlockQuote(_) => apply(&mut styles, |s| s.quote = true),
                    Tag::CodeBlock(_) => apply(&mut styles, |s| s.block = true),
                    Tag::Paragraph | Tag::TableCell => continue,
                    _ => {}
                }
                construct[range].fill(true);
            }
            Event::Text(_) => content[range].fill(true),
            Event::Code(_) => {
                apply(&mut styles, |s| s.code = true);
                let source = &text.as_bytes()[range.clone()];
                let ticks = source.iter().take_while(|b| **b == b'`').count();
                let inner = range.start + ticks..range.end - ticks.min(range.len() - ticks);
                construct[range].fill(true);
                content[inner].fill(true);
            }
            Event::TaskListMarker(_) | Event::Rule | Event::Html(_) | Event::InlineHtml(_) => {
                construct[range].fill(true)
            }
            _ => {}
        }
    }
    let mut styled = Styled::default();
    let mut start = 0;
    let style_at = |i: usize| Style {
        mark: construct[i] && !content[i],
        ..styles[i]
    };
    for (i, _) in text.char_indices().skip(1) {
        if style_at(i) != style_at(start) {
            styled.push(&text[start..i], style_at(start));
            start = i;
        }
    }
    if start < text.len() {
        styled.push(&text[start..], style_at(start));
    }
    styled.job(f32::INFINITY, Style::default())
}

/// Text of a block with the links in it.
#[derive(Default)]
struct Inline {
    styled: Styled,
    /// Byte ranges of the text and where they link to.
    links: Vec<(Range<usize>, String)>,
}

enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Code(String),
    Rule,
    Table {
        aligns: Vec<Alignment>,
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
}

#[derive(Default)]
struct Item {
    /// Whether the task is done, and where its `[ ]` begins in the source.
    task: Option<(bool, usize)>,
    blocks: Vec<Block>,
}

/// Containers open while parsing.
enum Open {
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Item(Item),
    Table {
        aligns: Vec<Alignment>,
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
    Row(Vec<Inline>),
}

/// What is collecting inline text.
enum Collect {
    Paragraph,
    Heading(u8),
    Cell,
}

#[derive(Default)]
struct Builder {
    root: Vec<Block>,
    open: Vec<Open>,
    inline: Option<(Collect, Inline)>,
    style: Style,
    /// Targets of the links being read, innermost last.
    targets: Vec<String>,
    code: Option<String>,
}

impl Builder {
    fn push_block(&mut self, block: Block) {
        for open in self.open.iter_mut().rev() {
            match open {
                Open::Quote(blocks) => return blocks.push(block),
                Open::Item(item) => return item.blocks.push(block),
                _ => {}
            }
        }
        self.root.push(block);
    }

    fn text(&mut self, text: &str) {
        if let Some(code) = &mut self.code {
            code.push_str(text);
            return;
        }
        // A tight list item holds its text without a paragraph.
        let (_, inline) = self
            .inline
            .get_or_insert_with(|| (Collect::Paragraph, Inline::default()));
        let start = inline.styled.text.len();
        inline.styled.push(text, self.style);
        if let Some(target) = self.targets.last() {
            let end = inline.styled.text.len();
            match inline.links.last_mut() {
                Some((range, last)) if range.end == start && last == target => range.end = end,
                _ => inline.links.push((start..end, target.clone())),
            }
        }
    }

    /// Ends the text being collected into a paragraph or heading.
    fn finish_inline(&mut self) {
        match self.inline.take() {
            Some((Collect::Paragraph, inline)) => self.push_block(Block::Paragraph(inline)),
            Some((Collect::Heading(level), inline)) => {
                self.push_block(Block::Heading(level, inline))
            }
            Some((Collect::Cell, inline)) => match self.open.last_mut() {
                Some(Open::Row(cells)) => cells.push(inline),
                Some(Open::Table { head, .. }) => head.push(inline),
                _ => {}
            },
            None => {}
        }
    }

    fn event(&mut self, event: Event, range: Range<usize>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(text) => {
                let style = self.style;
                self.style.code = true;
                self.text(&text);
                self.style = style;
            }
            Event::Html(text) => {
                let code = self.code.get_or_insert_with(String::new);
                code.push_str(&text);
            }
            Event::InlineHtml(text) => {
                let style = self.style;
                self.style.mark = true;
                self.text(&text);
                self.style = style;
            }
            Event::SoftBreak | Event::HardBreak => self.text("\n"),
            Event::Rule => {
                self.finish_inline();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                if let Some(Open::Item(item)) = self.open.last_mut() {
                    item.task = Some((done, range.start));
                }
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                self.finish_inline();
                self.inline = Some((Collect::Paragraph, Inline::default()));
            }
            Tag::Heading { level, .. } => {
                self.finish_inline();
                self.inline = Some((Collect::Heading(level as u8), Inline::default()));
            }
            Tag::BlockQuote(_) => {
                self.finish_inline();
                self.open.push(Open::Quote(Vec::new()));
            }
            Tag::CodeBlock(_) | Tag::HtmlBlock => {
                self.finish_inline();
                self.code = Some(String::new());
            }
            Tag::List(start) => {
                self.finish_inline();
                self.open.push(Open::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.finish_inline();
                self.open.push(Open::Item(Item::default()));
            }
            Tag::Table(aligns) => {
                self.finish_inline();
                self.open.push(Open::Table {
                    aligns,
                    head: Vec::new(),
                    rows: Vec::new(),
                });
            }
            Tag::TableRow => self.open.push(Open::Row(Vec::new())),
            Tag::TableCell => self.inline = Some((Collect::Cell, Inline::default())),
            Tag::Emphasis => self.style.emphasis = true,
            Tag::Strong => self.style.strong = true,
            Tag::Strikethrough => self.style.strike = true,
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.style.link = true;
                self.targets.push(dest_url.into_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::TableCell => self.finish_inline(),
            TagEnd::BlockQuote(_) => {
                self.finish_inline();
                if let Some(Open::Quote(blocks)) = self.open.pop() {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::CodeBlock | TagEnd::HtmlBlock => {
                if let Some(code) = self.code.take() {
                    self.push_block(Block::Code(code.trim_end_matches('\n').into()));
                }
            }
            TagEnd::List(_) => {
                self.finish_inline();
                if let Some(Open::List { start, items }) = self.open.pop() {
                    self.push_block(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.finish_inline();
                if let Some(Open::Item(item)) = self.open.pop()
                    && let Some(Open::List { items, .. }) = self.open.last_mut()
                {
                    items.push(item);
                }
            }
            TagEnd::TableRow => {
                if let Some(Open::Row(cells)) = self.open.pop()
                    && let Some(Open::Table { rows, .. }) = self.open.last_mut()
                {
                    rows.push(cells);
                }
            }
            TagEnd::Table => {
                if let Some(Open::Table { aligns, head, rows }) = self.open.pop() {
                    self.push_block(Block::Table { aligns, head, rows });
                }
            }
            TagEnd::Emphasis => self.style.emphasis = false,
            TagEnd::Strong => self.style.strong = false,
            TagEnd::Strikethrough => self.style.strike = false,
            TagEnd::Link | TagEnd::Image => {
                self.targets.pop();
                self.style.link = !self.targets.is_empty();
            }
            _ => {}
        }
    }
}

fn parse(text: &str) -> Vec<Block> {
    let mut builder = Builder::default();
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        builder.event(event, range);
    }
    builder.finish_inline();
    builder.root
}

/// What the user asked for in the rendered view.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Tick or untick the task whose `[ ]` begins at this byte of the source.
    Toggle(usize),
    /// Open a web address.
    Open(String),
}

/// Whether a link goes to the web or to an e-mail address, the targets opened on a click.
fn is_web(target: &str) -> bool {
    let lower = target.trim().to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

/// The source with the task box at byte `at` ticked or unticked.
pub fn toggle(text: &mut String, at: usize) {
    let mark = at + 1;
    let replacement = match text.as_bytes().get(mark) {
        Some(b' ') => "x",
        Some(b'x' | b'X') => " ",
        _ => return,
    };
    if text.as_bytes().get(at) == Some(&b'[') {
        text.replace_range(mark..mark + 1, replacement);
    }
}

/// Hash of a source text, which tells whether it changed since it was parsed.
pub fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// A rendered text, parsed once and kept while the text stays the same.
#[derive(Default)]
pub struct Document {
    /// Hash of the text the blocks come from.
    source: Option<u64>,
    blocks: Vec<Block>,
}

impl Document {
    /// Renders `text`, parsing it again only after it changed, and returns what the user
    /// clicked.
    pub fn show(&mut self, ui: &mut egui::Ui, text: &str) -> Option<Action> {
        let hash = text_hash(text);
        if self.source != Some(hash) {
            self.blocks = parse(text);
            self.source = Some(hash);
        }
        let mut render = Render {
            action: None,
            tables: 0,
            depth: 0,
        };
        ui.spacing_mut().item_spacing.y = 0.0;
        render.blocks(ui, &self.blocks, Style::default());
        render.action
    }
}

struct Render {
    action: Option<Action>,
    /// Tables shown so far, for their identities.
    tables: usize,
    /// Lists the current block is nested in.
    depth: usize,
}

impl Render {
    fn blocks(&mut self, ui: &mut egui::Ui, blocks: &[Block], base: Style) {
        for (index, block) in blocks.iter().enumerate() {
            if index > 0 {
                ui.add_space(BLOCK_GAP);
            }
            match block {
                Block::Paragraph(inline) => self.inline(ui, inline, base),
                Block::Heading(level, inline) => {
                    if index > 0 {
                        ui.add_space(4.0);
                    }
                    let heading = Style {
                        heading: *level,
                        ..base
                    };
                    self.inline(ui, inline, heading);
                    if *level <= 2 {
                        ui.add_space(4.0);
                        rule(ui, theme::LINE, 2.0);
                    }
                }
                Block::Quote(blocks) => {
                    let response = ui.horizontal_top(|ui| {
                        ui.add_space(12.0);
                        ui.vertical(|ui| {
                            let quote = Style {
                                quote: true,
                                ..base
                            };
                            self.blocks(ui, blocks, quote)
                        });
                    });
                    let rect = response.response.rect;
                    let bar = egui::Rect::from_min_size(
                        rect.left_top(),
                        vec2(pixel::unit(ui.ctx(), 3.0), rect.height()),
                    );
                    ui.painter()
                        .rect_filled(pixel::snap_rect(ui.ctx(), bar), 0.0, theme::TEAL);
                }
                Block::List { start, items } => self.list(ui, *start, items, base),
                Block::Code(code) => {
                    let border = pixel::unit(ui.ctx(), 2.0);
                    egui::Frame::new()
                        .fill(theme::WELL)
                        .stroke(Stroke::new(border, theme::LINE))
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let mut styled = Styled::default();
                            styled.push(
                                code,
                                Style {
                                    block: true,
                                    ..Style::default()
                                },
                            );
                            label(ui, &styled, ui.available_width(), base);
                        });
                }
                Block::Rule => rule(ui, theme::SHADOW, 2.0),
                Block::Table { aligns, head, rows } => self.table(ui, aligns, head, rows, base),
            }
        }
    }

    fn inline(&mut self, ui: &mut egui::Ui, inline: &Inline, base: Style) {
        let (galley, pos, response) = label(ui, &inline.styled, ui.available_width(), base);
        let Some(pointer) = response.hover_pos() else {
            return;
        };
        let Some(index) = char_at(&galley, pointer - pos) else {
            return;
        };
        let Some(byte) = inline.styled.text.char_indices().nth(index).map(|(b, _)| b) else {
            return;
        };
        let Some((_, target)) = inline.links.iter().find(|(range, _)| range.contains(&byte)) else {
            return;
        };
        if is_web(target) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if response.clicked() {
                self.action = Some(Action::Open(target.trim().to_owned()));
            }
        }
        response.on_hover_text(target);
    }

    fn list(&mut self, ui: &mut egui::Ui, start: Option<u64>, items: &[Item], base: Style) {
        let depth = self.depth;
        self.depth += 1;
        for (n, item) in items.iter().enumerate() {
            if n > 0 {
                ui.add_space(ITEM_GAP);
            }
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let height = BODY + 6.0;
                let sense = if item.task.is_some() {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (rect, response) = ui.allocate_exact_size(vec2(MARKER, height), sense);
                let ctx = ui.ctx().clone();
                match (item.task, start) {
                    (Some((done, at)), _) => {
                        task_box(ui, rect, done, response.hovered());
                        if response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.action = Some(Action::Toggle(at));
                        }
                    }
                    (None, Some(first)) => {
                        let number = format!("{}.", first + n as u64);
                        ui.painter().text(
                            rect.right_center() - vec2(4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            number,
                            FontId::proportional(BODY),
                            MARK,
                        );
                    }
                    (None, None) => {
                        let size = pixel::unit(&ctx, 5.0);
                        let dot = pixel::snap_rect(
                            &ctx,
                            egui::Rect::from_center_size(
                                rect.center() + vec2(2.0, 0.0),
                                Vec2::splat(size),
                            ),
                        );
                        // Nested lists alternate filled and hollow squares.
                        if depth.is_multiple_of(2) {
                            ui.painter().rect_filled(dot, 0.0, theme::TEAL);
                        } else {
                            ui.painter().rect_stroke(
                                dot,
                                0.0,
                                Stroke::new(pixel::unit(&ctx, 1.5), theme::TEAL),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                }
                ui.vertical(|ui| {
                    let item_style = Style {
                        done: item.task.is_some_and(|(done, _)| done),
                        ..base
                    };
                    self.blocks(ui, &item.blocks, item_style);
                });
            });
        }
        self.depth = depth;
    }

    fn table(
        &mut self,
        ui: &mut egui::Ui,
        aligns: &[Alignment],
        head: &[Inline],
        rows: &[Vec<Inline>],
        base: Style,
    ) {
        self.tables += 1;
        let id = ui.id().with(("note-table", self.tables));
        egui::ScrollArea::horizontal()
            .id_salt(id)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // The header row is tinted, the body rows striped.
                egui::Grid::new(id)
                    .with_row_color(|row, _| match row {
                        0 => Some(theme::TEAL_SOFT),
                        row if row % 2 == 0 => Some(theme::WELL),
                        _ => None,
                    })
                    .spacing(vec2(16.0, 6.0))
                    .show(ui, |ui| {
                        let header = Style {
                            strong: true,
                            ..base
                        };
                        for (column, cell) in head.iter().enumerate() {
                            self.cell(ui, cell, aligns.get(column), header);
                        }
                        ui.end_row();
                        for row in rows {
                            for (column, cell) in row.iter().enumerate() {
                                self.cell(ui, cell, aligns.get(column), base);
                            }
                            ui.end_row();
                        }
                    });
            });
    }

    fn cell(&mut self, ui: &mut egui::Ui, cell: &Inline, align: Option<&Alignment>, base: Style) {
        let align = match align {
            Some(Alignment::Center) => egui::Align::Center,
            Some(Alignment::Right) => egui::Align::Max,
            _ => egui::Align::Min,
        };
        ui.with_layout(egui::Layout::top_down(align), |ui| {
            self.inline(ui, cell, base);
        });
    }
}

/// Lays out styled text as a selectable label, with bold sections painted heavier.
fn label(
    ui: &mut egui::Ui,
    styled: &Styled,
    wrap: f32,
    base: Style,
) -> (std::sync::Arc<egui::Galley>, Pos2, egui::Response) {
    let (job, bold) = styled.job(wrap, base);
    let galley = ui.painter().layout_job(job.clone());
    let (pos, galley, response) = egui::Label::new(galley)
        .selectable(true)
        .sense(Sense::click())
        .layout_in_ui(ui);
    if ui.is_rect_visible(response.rect) {
        egui::text_selection::LabelSelectionState::label_text_selection(
            ui,
            &response,
            pos,
            galley.clone(),
            theme::INK,
            Stroke::NONE,
        );
        embolden(ui.painter(), &job, &bold, pos);
    }
    (galley, pos, response)
}

/// Index of the character under `pos`, relative to the galley.
fn char_at(galley: &egui::Galley, pos: Vec2) -> Option<usize> {
    let cursor = galley.cursor_from_pos(pos);
    let index = cursor.index.0;
    [index.checked_sub(1), Some(index)]
        .into_iter()
        .flatten()
        .find(|&index| {
            let left = galley.pos_from_cursor(CCursor::new(index));
            let right = galley.pos_from_cursor(CCursor::new(index + 1));
            left.top() == right.top()
                && (left.left()..right.left()).contains(&pos.x)
                && (left.top()..left.bottom()).contains(&pos.y)
        })
}

/// A horizontal pixel line across the available width.
fn rule(ui: &mut egui::Ui, color: Color32, thickness: f32) {
    let ctx = ui.ctx().clone();
    let height = pixel::unit(&ctx, thickness);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    ui.painter()
        .rect_filled(pixel::snap_rect(&ctx, rect), 0.0, color);
}

/// A pixel check box, ticked with a teal fill when `done`.
fn task_box(ui: &egui::Ui, rect: egui::Rect, done: bool, hovered: bool) {
    let ctx = ui.ctx();
    let side = pixel::unit(ctx, 13.0);
    let face = pixel::snap_rect(
        ctx,
        egui::Rect::from_center_size(rect.center() + vec2(2.0, 0.0), Vec2::splat(side)),
    );
    let fill = match (done, hovered) {
        (true, _) => theme::TEAL,
        (false, true) => theme::TEAL_SOFT,
        (false, false) => theme::PAPER,
    };
    ui.painter()
        .add(pixel::panel(ctx, face, fill, theme::INK, None));
    if done {
        let unit = pixel::unit(ctx, 1.0);
        let mut shapes = Vec::new();
        pixel::icon_shapes(
            &mut shapes,
            pixel::snap(ctx, face.center() - Vec2::splat(4.5 * unit)),
            unit,
            &pixel::ICON_CHECK,
            theme::INK,
            theme::INK,
        );
        ui.painter().extend(shapes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(text: &str) -> String {
        let (job, _) = highlight(text);
        job.sections
            .iter()
            .filter(|s| s.format.color == MARK)
            .map(|s| &text[s.byte_range.start.0..s.byte_range.end.0])
            .collect()
    }

    #[test]
    fn the_editor_colors_marks_and_keeps_the_text() {
        let text =
            "# Title\n**bold** and *it* `code`\n- [ ] task\n> quote\n[site](https://example.com)";
        let (job, bold) = highlight(text);
        assert_eq!(job.text, text);
        let covered: usize = job
            .sections
            .iter()
            .map(|s| s.byte_range.end.0 - s.byte_range.start.0)
            .sum();
        assert_eq!(covered, text.len());
        let found = marks(text);
        for mark in [
            "#",
            "**",
            "*",
            "`",
            "-",
            "[ ]",
            ">",
            "](https://example.com)",
        ] {
            assert!(found.contains(mark), "{mark} in {found:?}");
        }
        let bold_text: String = job
            .sections
            .iter()
            .zip(&bold)
            .filter(|(_, b)| **b)
            .map(|(s, _)| &text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert_eq!(bold_text, "Titlebold");
        // Plain text and other languages stay plain.
        assert_eq!(marks("便签 note"), "");
    }

    #[test]
    fn rendering_reads_blocks_links_and_tasks() {
        let text = "## Plan\n\n- [x] done\n- [ ] next, see [docs](https://example.com/a)\n\n1. one\n2. two\n\n> quoted\n\n```\nlet x = 1;\n```\n\n| a | b |\n|---|:-:|\n| 1 | 2 |\n\n---\nline one\nline two";
        let blocks = parse(text);
        assert!(matches!(&blocks[0], Block::Heading(2, inline) if inline.styled.text == "Plan"));
        let Block::List { start: None, items } = &blocks[1] else {
            panic!("task list");
        };
        assert_eq!(items[0].task, Some((true, text.find("[x]").unwrap())));
        assert_eq!(items[1].task, Some((false, text.find("[ ]").unwrap())));
        let Block::Paragraph(next) = &items[1].blocks[0] else {
            panic!("item text");
        };
        assert_eq!(next.styled.text, "next, see docs");
        assert_eq!(next.links, [(10..14, "https://example.com/a".to_owned())]);
        assert!(matches!(&blocks[2], Block::List { start: Some(1), items } if items.len() == 2));
        assert!(matches!(&blocks[3], Block::Quote(inner) if inner.len() == 1));
        assert!(matches!(&blocks[4], Block::Code(code) if code == "let x = 1;"));
        let Block::Table { aligns, head, rows } = &blocks[5] else {
            panic!("table");
        };
        assert_eq!(aligns, &[Alignment::None, Alignment::Center]);
        assert_eq!(head.len(), 2);
        assert_eq!(rows[0][1].styled.text, "2");
        assert!(matches!(&blocks[6], Block::Rule));
        // Line breaks inside a paragraph are kept, as a note is written.
        assert!(matches!(&blocks[7], Block::Paragraph(p) if p.styled.text == "line one\nline two"));
    }

    #[test]
    fn task_boxes_toggle_in_the_source() {
        let mut text = String::from("- [ ] a\n- [X] b");
        toggle(&mut text, 2);
        assert_eq!(text, "- [x] a\n- [X] b");
        toggle(&mut text, 10);
        assert_eq!(text, "- [x] a\n- [ ] b");
        // A stale offset leaves the text alone.
        toggle(&mut text, 4);
        assert_eq!(text, "- [x] a\n- [ ] b");
    }

    #[test]
    fn only_web_and_mail_links_open() {
        assert!(is_web("https://example.com"));
        assert!(is_web(" HTTP://example.com"));
        assert!(is_web("mailto:me@example.com"));
        assert!(!is_web("C:\\Windows\\System32\\cmd.exe"));
        assert!(!is_web("file:///C:/x"));
        assert!(!is_web("notes.md"));
    }
}
