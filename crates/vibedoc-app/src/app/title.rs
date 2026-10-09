//! The document card's top row: File, Undo and Redo on the left, then the
//! document's name (click it to rename) and whether it's saved, with Find
//! and the assistant on the right.

use eframe::egui::{
    self, vec2, Align, Color32, CursorIcon, Frame, Key, Label, Layout, Popup, RichText, Sense, TextEdit, Ui,
    ViewportCommand,
};

use super::App;
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette, HEADER_HEIGHT};
use crate::ui::widgets::{button, icon_button, vertical_divider, ButtonKind};

const TITLE_SIZE: f32 = 20.0;

/// The name being typed while renaming a document in place.
pub struct TitleEdit {
    text: String,
    focused: bool,
}

impl App {
    pub(super) fn start_rename(&mut self) {
        self.title_edit = Some(TitleEdit { text: self.tab().name.clone(), focused: false });
    }

    pub(super) fn title_row(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        let size = vec2(ui.available_width(), HEADER_HEIGHT);
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            if !self.sidebar.open
                && icon_button(ui, Icon::Sidebar, false).on_hover_text("Show library (Ctrl+\\)").clicked()
            {
                self.sidebar.open = true;
            }
            let file = button(ui, ButtonKind::Outline, Some(Icon::File), "File");
            Popup::menu(&file).show(|ui| self.file_menu(ui));
            self.history_buttons(ui);
            vertical_divider(ui, 20.0);
            ui.add_space(6.0);
            if self.title_edit.is_some() {
                self.title_field(ui);
            } else {
                let title = RichText::new(&self.tab().name).font(theme::semibold(TITLE_SIZE)).color(p.text);
                let response = ui.add(Label::new(title).sense(Sense::click()).truncate());
                if response.hovered() {
                    let rect = egui::Rect::from_min_size(
                        response.rect.right_center() + vec2(6.0, -7.0),
                        vec2(14.0, 14.0),
                    );
                    paint_icon(ui.painter(), rect, Icon::Pencil, p.text_faint);
                }
                let response = response.on_hover_text("Click to rename").on_hover_cursor(CursorIcon::Text);
                if response.clicked() {
                    self.start_rename();
                }
                ui.add_space(18.0);
            }
            self.save_state(ui);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if assistant_button(ui, self.assistant.panel_open).clicked() {
                    self.assistant.panel_open = !self.assistant.panel_open;
                }
                ui.add_space(2.0);
                if icon_button(ui, Icon::Search, self.find.open).on_hover_text("Find (Ctrl+F)").clicked() {
                    if self.find.open {
                        self.close_find();
                    } else {
                        self.open_find();
                    }
                }
            });
        });
    }

    /// Undo and Redo, always at hand beside File, like Word's quick-access bar.
    fn history_buttons(&mut self, ui: &mut Ui) {
        let editable = !self.tab().conversation.is_running();
        let editor = &mut self.tabs[self.active].editor;
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_enabled_ui(editable && editor.can_undo(), |ui| {
            if icon_button(ui, Icon::Undo, false).on_hover_text("Undo (Ctrl+Z)").clicked() {
                editor.undo();
            }
        });
        ui.add_enabled_ui(editable && editor.can_redo(), |ui| {
            if icon_button(ui, Icon::Redo, false).on_hover_text("Redo (Ctrl+Shift+Z)").clicked() {
                editor.redo();
            }
        });
        ui.spacing_mut().item_spacing.x = 8.0;
    }

    /// The name as a text box. Enter or clicking away renames; Escape cancels.
    fn title_field(&mut self, ui: &mut Ui) {
        let Some(edit) = self.title_edit.as_mut() else { return };
        let field = TextEdit::singleline(&mut edit.text)
            .font(theme::semibold(TITLE_SIZE))
            .frame(Frame::NONE)
            .desired_width(ui.available_width().min(420.0));
        let response = ui.add(field);
        if !edit.focused {
            response.request_focus();
            edit.focused = true;
        }
        if response.lost_focus() {
            let cancelled = ui.input(|i| i.key_pressed(Key::Escape));
            let text = edit.text.trim().to_string();
            self.title_edit = None;
            if !cancelled && !text.is_empty() && text != self.tab().name {
                self.rename_active(&text);
            }
        }
    }

    /// A dot and a word: saved, edited, or never saved.
    fn save_state(&self, ui: &mut Ui) {
        let p = palette(ui);
        let (color, label) = if self.tab().file.is_none() {
            (p.text_faint, "Not saved yet")
        } else if self.tab().is_dirty() {
            (p.accent, "Edited")
        } else {
            (p.success, "Saved")
        };
        let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.5, color);
        ui.label(RichText::new(label).color(p.text_muted));
        if !self.status.is_empty() {
            ui.label(RichText::new(format!("· {}", self.status)).color(p.text_faint));
        }
    }

    pub(super) fn update_title(&mut self, ctx: &egui::Context) {
        let unsaved = if self.tab().is_dirty() { " •" } else { "" };
        let title = format!("{}{unsaved} — VibeDoc", self.tab().name);
        if title != self.window_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}

/// The accent-colored sparkle button that shows or hides the assistant.
fn assistant_button(ui: &mut Ui, open: bool) -> egui::Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    let (fill, color) = match (open, response.hovered()) {
        (true, _) => (p.accent, Color32::WHITE),
        (false, true) => (p.accent_soft.gamma_multiply(1.2), p.accent),
        (false, false) => (p.accent_soft, p.accent),
    };
    ui.painter().rect_filled(rect, 10.0, fill);
    paint_icon(ui.painter(), rect.shrink(8.5), Icon::Sparkle, color);
    let tip = if cfg!(target_os = "macos") { "Assistant (⌘K to ask)" } else { "Assistant (Ctrl+K to ask)" };
    response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip)
}
