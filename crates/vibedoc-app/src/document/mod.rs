//! The pages on screen: turning mouse and keyboard input into editor commands.
//! Painting lives in `paint`, page placement in `geometry`.

mod fonts;
mod geometry;
mod paint;

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use eframe::egui::{
    self, ColorImage, CursorIcon, Event, EventFilter, Key, Modifiers, Rect, Response, ScrollArea, Sense,
    TextureHandle, TextureOptions, Ui, Vec2,
};
use vibedoc_core::{Document, Editor, Image, Layout, PageSetup, Position};

use fonts::EguiMeasure;
use geometry::PageGeometry;
pub use paint::{PageLook, Paper};

pub struct DocumentView {
    pub zoom: f32,
    /// Keeps each tab's scroll position separate.
    id: u64,
    /// Layout is rebuilt only when the document's revision changes.
    cached_layout: Option<(u64, Rc<Layout>)>,
    /// Column the caret tries to stay in while pressing Up/Down.
    goal_x: Option<f32>,
    scroll_to_caret: bool,
    /// A paragraph to bring into view, e.g. one the assistant changed.
    scroll_to_paragraph: Option<usize>,
    /// Decoded images, uploaded to the GPU once. `None` if the file couldn't be decoded.
    textures: HashMap<ImageKey, Option<TextureHandle>>,
}

/// Identifies image bytes by their shared allocation, so an undone image and a
/// new one that reuses its placeholder character never share a texture.
type ImageKey = (usize, usize);

fn image_key(image: &Image) -> ImageKey {
    (Arc::as_ptr(&image.bytes) as usize, image.bytes.len())
}

impl Default for DocumentView {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        DocumentView {
            zoom: 1.0,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            cached_layout: None,
            goal_x: None,
            scroll_to_caret: false,
            scroll_to_paragraph: None,
            textures: HashMap::new(),
        }
    }
}

impl DocumentView {
    pub fn reveal_caret(&mut self) {
        self.scroll_to_caret = true;
    }

    pub fn reveal_paragraph(&mut self, paragraph: usize) {
        self.scroll_to_paragraph = Some(paragraph);
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

    fn load_textures(&mut self, ctx: &egui::Context, document: &Document) {
        for image in document.images.values() {
            self.textures.entry(image_key(image)).or_insert_with(|| {
                let decoded = image::load_from_memory(&image.bytes).ok()?.to_rgba8();
                let size = [decoded.width() as usize, decoded.height() as usize];
                let pixels = ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
                Some(ctx.load_texture("document image", pixels, TextureOptions::LINEAR))
            });
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        editor: &mut Editor,
        look: &PageLook,
        keep_focus: bool,
        editable: bool,
    ) {
        ScrollArea::both().id_salt(("pages", self.id)).auto_shrink(false).show(ui, |ui| {
            let layout = self.layout(ui.ctx(), editor);
            let pages = PageGeometry::new(ui.cursor().min, ui.available_width(), &layout, self.zoom);
            let response = ui.allocate_rect(pages.total_rect, Sense::click_and_drag());

            if keep_focus && !response.has_focus() {
                response.request_focus();
            }
            self.handle_pointer(ui, &response, editor, &layout, &pages, editable);
            if response.has_focus() && editable {
                self.handle_keyboard(ui, &response, editor, &layout);
            }

            // Input may have edited the document, so paint the fresh layout.
            let layout = self.layout(ui.ctx(), editor);
            let pages = PageGeometry::new(response.rect.min, response.rect.width(), &layout, self.zoom);
            self.load_textures(ui.ctx(), editor.document());
            paint::paint(ui, editor, &layout, &pages, look, &self.textures, response.has_focus());

            if std::mem::take(&mut self.scroll_to_caret) {
                self.scroll_to(ui, &layout, &pages, editor.selection().cursor, None);
            } else if let Some(paragraph) = self.scroll_to_paragraph.take() {
                let start = editor.document().clamp(Position::new(paragraph, 0));
                self.scroll_to(ui, &layout, &pages, start, Some(egui::Align::Center));
            }
        });
    }

    fn scroll_to(
        &self,
        ui: &Ui,
        layout: &Layout,
        pages: &PageGeometry,
        position: Position,
        align: Option<egui::Align>,
    ) {
        let caret = layout.caret(position);
        let top = pages.to_screen(caret.page, caret.x, caret.y);
        let rect = Rect::from_min_size(top, Vec2::new(1.0, caret.height * self.zoom));
        ui.scroll_to_rect(rect.expand(48.0), align);
    }

    fn handle_pointer(
        &mut self,
        ui: &Ui,
        response: &Response,
        editor: &mut Editor,
        layout: &Layout,
        pages: &PageGeometry,
        editable: bool,
    ) {
        if let Some(hover) = response.hover_pos() {
            let (page, x, y) = pages.to_layout(hover);
            let over_checkbox = layout.checkbox_at(page, x, y).is_some();
            ui.ctx().set_cursor_icon(if over_checkbox { CursorIcon::PointingHand } else { CursorIcon::Text });
        }
        let Some(pointer) = response.interact_pointer_pos() else { return };
        let (page, x, y) = pages.to_layout(pointer);
        let position = layout.position_at(page, x, y);
        let (pressed, shift) = ui.input(|i| (i.pointer.primary_pressed(), i.modifiers.shift));
        if let (true, true, Some(item)) = (pressed, editable, layout.checkbox_at(page, x, y)) {
            editor.toggle_checked(item);
            return;
        }

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
