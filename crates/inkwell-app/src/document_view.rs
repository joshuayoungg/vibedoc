//! The pages on screen: painting the layout and turning mouse and keyboard
//! input into editor commands.

use std::rc::Rc;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Color32, CursorIcon, Event, EventFilter, FontId, Key, Modifiers, Painter, Pos2, Rect, Response,
    ScrollArea, Sense, Stroke, Ui, Vec2,
};
use inkwell_core::layout::Line;
use inkwell_core::{Editor, Layout, PageSetup, Position};

use crate::fonts::{font_size, is_bold, EguiMeasure, FAKE_BOLD_SHIFT};

/// Gap around and between pages, in screen points.
const PAGE_GAP: f32 = 24.0;
const TEXT_COLOR: Color32 = Color32::from_rgb(30, 30, 30);
const SELECTION_COLOR: Color32 = Color32::from_rgb(179, 212, 252);
const FIND_COLOR: Color32 = Color32::from_rgb(255, 230, 120);

pub struct DocumentView {
    pub zoom: f32,
    /// Layout is rebuilt only when the document's revision changes.
    cached_layout: Option<(u64, Rc<Layout>)>,
    /// Column the caret tries to stay in while pressing Up/Down.
    goal_x: Option<f32>,
    scroll_to_caret: bool,
}

impl Default for DocumentView {
    fn default() -> Self {
        DocumentView { zoom: 1.25, cached_layout: None, goal_x: None, scroll_to_caret: false }
    }
}

impl DocumentView {
    pub fn reveal_caret(&mut self) {
        self.scroll_to_caret = true;
    }

    pub fn layout(&mut self, ctx: &egui::Context, editor: &Editor) -> Rc<Layout> {
        if let Some((revision, layout)) = &self.cached_layout {
            if *revision == editor.revision() {
                return layout.clone();
            }
        }
        let layout = ctx
            .fonts_mut(|fonts| Layout::build(editor.document(), PageSetup::LETTER, &EguiMeasure::new(fonts)));
        let layout = Rc::new(layout);
        self.cached_layout = Some((editor.revision(), layout.clone()));
        layout
    }

    /// Forget the cached layout (e.g. after opening a different file).
    pub fn reset(&mut self) {
        self.cached_layout = None;
        self.goal_x = None;
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        editor: &mut Editor,
        highlights: &[(Position, Position)],
        keep_focus: bool,
    ) {
        ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let layout = self.layout(ui.ctx(), editor);
            let pages = PageGeometry::new(ui.cursor().min, ui.available_width(), &layout, self.zoom);
            let response = ui.allocate_rect(pages.total_rect, Sense::click_and_drag());

            if keep_focus && !response.has_focus() {
                response.request_focus();
            }
            self.handle_pointer(ui, &response, editor, &layout, &pages);
            if response.has_focus() {
                self.handle_keyboard(ui, &response, editor, &layout);
            }

            // Input may have edited the document, so paint the fresh layout.
            let layout = self.layout(ui.ctx(), editor);
            let pages = PageGeometry::new(response.rect.min, response.rect.width(), &layout, self.zoom);
            paint(ui, editor, &layout, &pages, highlights, response.has_focus());

            if std::mem::take(&mut self.scroll_to_caret) {
                let caret = layout.caret(editor.selection().cursor);
                let top = pages.to_screen(caret.page, caret.x, caret.y);
                let rect = Rect::from_min_size(top, Vec2::new(1.0, caret.height * self.zoom));
                ui.scroll_to_rect(rect.expand(48.0), None);
            }
        });
    }

    fn handle_pointer(
        &mut self,
        ui: &Ui,
        response: &Response,
        editor: &mut Editor,
        layout: &Layout,
        pages: &PageGeometry,
    ) {
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
        let Some(pointer) = response.interact_pointer_pos() else { return };
        let (page, x, y) = pages.to_layout(pointer);
        let position = layout.position_at(page, x, y);
        let (pressed, shift) = ui.input(|i| (i.pointer.primary_pressed(), i.modifiers.shift));

        if response.double_clicked() {
            editor.select_word_at(position);
        } else if pressed {
            editor.move_caret(position, shift);
        } else if response.dragged() {
            editor.move_caret(position, true);
        }
        self.goal_x = None;
    }

    fn handle_keyboard(&mut self, ui: &Ui, response: &Response, editor: &mut Editor, layout: &Layout) {
        // Keep arrows and Tab in the editor instead of moving focus between widgets.
        let filter = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false };
        ui.memory_mut(|m| m.set_focus_lock_filter(response.id, filter));

        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::Text(text) => editor.type_text(&text),
                Event::Copy if !editor.selection().is_empty() => ui.ctx().copy_text(editor.selected_text()),
                Event::Cut if !editor.selection().is_empty() => {
                    ui.ctx().copy_text(editor.selected_text());
                    editor.delete_selected();
                }
                Event::Paste(text) => editor.paste(&text),
                Event::Key { key, pressed: true, modifiers, .. } => {
                    if !self.handle_key(key, modifiers, editor, layout) {
                        continue;
                    }
                }
                _ => continue,
            }
            self.scroll_to_caret = true;
        }
    }

    /// Returns false if the key isn't one the editor uses.
    fn handle_key(&mut self, key: Key, modifiers: Modifiers, editor: &mut Editor, layout: &Layout) -> bool {
        let extend = modifiers.shift;
        let cursor = editor.selection().cursor;
        let (document_start, document_end) = (editor.document().start(), editor.document().end());
        let mut keeps_goal_x = false;
        match key {
            Key::ArrowLeft => editor.move_left(extend),
            Key::ArrowRight => editor.move_right(extend),
            Key::ArrowUp | Key::ArrowDown => {
                let goal_x = *self.goal_x.get_or_insert_with(|| layout.caret(cursor).x);
                let target = layout.vertical_move(cursor, key == Key::ArrowDown, goal_x);
                editor.move_caret(target, extend);
                keeps_goal_x = true;
            }
            Key::Home if modifiers.command => editor.move_caret(document_start, extend),
            Key::End if modifiers.command => editor.move_caret(document_end, extend),
            Key::Home => editor.move_caret(layout.line_start(cursor), extend),
            Key::End => editor.move_caret(layout.line_end(cursor), extend),
            Key::Backspace => editor.delete_backward(),
            Key::Delete => editor.delete_forward(),
            Key::Enter => editor.new_paragraph(),
            Key::Tab => editor.type_text("\t"),
            _ => return false,
        }
        if !keeps_goal_x {
            self.goal_x = None;
        }
        true
    }
}

/// Where the pages sit on screen, and converting between screen points and
/// layout coordinates (points inside a page's content area).
struct PageGeometry {
    total_rect: Rect,
    left: f32,
    top: f32,
    zoom: f32,
    setup: PageSetup,
}

impl PageGeometry {
    fn new(origin: Pos2, available_width: f32, layout: &Layout, zoom: f32) -> Self {
        let setup = layout.page_setup;
        let page_width = setup.width * zoom;
        let page_stride = setup.height * zoom + PAGE_GAP;
        let width = (page_width + 2.0 * PAGE_GAP).max(available_width);
        let height = PAGE_GAP + layout.page_count as f32 * page_stride;
        let total_rect = Rect::from_min_size(origin, Vec2::new(width, height));
        PageGeometry {
            total_rect,
            left: total_rect.center().x - page_width / 2.0,
            top: total_rect.top() + PAGE_GAP,
            zoom,
            setup,
        }
    }

    fn page_stride(&self) -> f32 {
        self.setup.height * self.zoom + PAGE_GAP
    }

    fn page_rect(&self, page: usize) -> Rect {
        let min = Pos2::new(self.left, self.top + page as f32 * self.page_stride());
        Rect::from_min_size(min, Vec2::new(self.setup.width, self.setup.height) * self.zoom)
    }

    fn to_screen(&self, page: usize, x: f32, y: f32) -> Pos2 {
        let corner = self.page_rect(page).min;
        corner + Vec2::new(x + self.setup.margin, y + self.setup.margin) * self.zoom
    }

    fn to_layout(&self, point: Pos2) -> (usize, f32, f32) {
        let page = ((point.y - self.top) / self.page_stride()).floor().max(0.0) as usize;
        let corner = self.page_rect(page).min;
        let x = (point.x - corner.x) / self.zoom - self.setup.margin;
        let y = (point.y - corner.y) / self.zoom - self.setup.margin;
        (page, x, y)
    }
}

fn paint(
    ui: &Ui,
    editor: &Editor,
    layout: &Layout,
    pages: &PageGeometry,
    highlights: &[(Position, Position)],
    focused: bool,
) {
    let painter = ui.painter();
    let visible = ui.clip_rect();

    for page in 0..layout.page_count {
        let rect = pages.page_rect(page);
        if rect.intersects(visible) {
            painter.rect_filled(rect.translate(Vec2::new(1.0, 2.0)), 0.0, Color32::from_black_alpha(40));
            painter.rect_filled(rect, 0.0, Color32::WHITE);
        }
    }

    let selection = editor.selection();
    for line in &layout.lines {
        let top_left = pages.to_screen(line.page, 0.0, line.y);
        let line_rect = Rect::from_min_size(
            top_left,
            Vec2::new(layout.page_setup.content_width(), line.height) * pages.zoom,
        );
        if !line_rect.intersects(visible) {
            continue;
        }
        for &(start, end) in highlights {
            paint_range(painter, line, pages, start, end, FIND_COLOR);
        }
        if !selection.is_empty() {
            paint_range(painter, line, pages, selection.start(), selection.end(), SELECTION_COLOR);
        }
        paint_line_text(ui, painter, line, pages);
    }

    if focused {
        let caret = layout.caret(selection.cursor);
        let top = pages.to_screen(caret.page, caret.x, caret.y);
        let bottom = top + Vec2::new(0.0, caret.height * pages.zoom);
        painter.line_segment([top, bottom], Stroke::new(1.5, TEXT_COLOR));
    }
}

/// Shades the part of `start..end` that falls on this line.
fn paint_range(
    painter: &Painter,
    line: &Line,
    pages: &PageGeometry,
    start: Position,
    end: Position,
    color: Color32,
) {
    if line.paragraph < start.paragraph || line.paragraph > end.paragraph {
        return;
    }
    let from = if line.paragraph == start.paragraph { start.offset.max(line.start) } else { line.start };
    let to = if line.paragraph == end.paragraph { end.offset.min(line.end) } else { line.end };
    let mut right = line.x_of(to);
    // Show the paragraph break as selected when the selection continues past it.
    if line.last_in_paragraph && line.paragraph < end.paragraph {
        right += 4.0;
    }
    let left = line.x_of(from);
    if from > to || right <= left {
        return;
    }
    let min = pages.to_screen(line.page, left, line.y);
    let max = pages.to_screen(line.page, right, line.y + line.height);
    painter.rect_filled(Rect::from_min_max(min, max), 0.0, color);
}

fn paint_line_text(ui: &Ui, painter: &Painter, line: &Line, pages: &PageGeometry) {
    let size = font_size(line.kind) * pages.zoom;
    let font = FontId::proportional(size);
    let row_height = ui.ctx().fonts_mut(|f| f.row_height(&font));
    let text_top = (line.height * pages.zoom - row_height) / 2.0;
    let baseline = text_top + row_height * 0.82;

    if let Some(marker) = &line.marker {
        let x = line.indent - inkwell_core::layout::LIST_INDENT * 0.75;
        let pos = pages.to_screen(line.page, x, line.y) + Vec2::new(0.0, text_top);
        painter.text(pos, egui::Align2::LEFT_TOP, marker, font.clone(), TEXT_COLOR);
    }

    for glyph in &line.glyphs {
        let pos = pages.to_screen(line.page, glyph.x, line.y) + Vec2::new(0.0, text_top);
        if !glyph.ch.is_whitespace() {
            let format = TextFormat {
                font_id: font.clone(),
                color: TEXT_COLOR,
                italics: glyph.style.italic,
                ..Default::default()
            };
            let job = LayoutJob::single_section(glyph.ch.to_string(), format);
            let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
            if is_bold(glyph.style, line.kind) {
                let shift = Vec2::new(size * FAKE_BOLD_SHIFT, 0.0);
                painter.galley(pos + shift, galley.clone(), TEXT_COLOR);
            }
            painter.galley(pos, galley, TEXT_COLOR);
        }
        if glyph.style.underline {
            let y = pos.y - text_top + baseline;
            let end = pos.x + glyph.width * pages.zoom;
            painter.line_segment([Pos2::new(pos.x, y), Pos2::new(end, y)], Stroke::new(1.0, TEXT_COLOR));
        }
    }
}
