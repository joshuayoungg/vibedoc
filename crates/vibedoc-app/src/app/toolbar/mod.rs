//! The formatting ribbon: two rows of controls in groups (styles, font,
//! paragraph, insert), like Word's Home tab, centered in its card.

mod pickers;

use eframe::egui::{vec2, Color32, CursorIcon, Frame, Margin, Rect, Sense, Shadow, Stroke, Ui, Vec2};
use rfd::FileDialog;
use vibedoc_core::{Alignment, Image, PageSetup, ParagraphKind, Script, StyleFlag};

use super::App;
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette};
use crate::ui::widgets::{icon_button, vertical_divider, ICON_BUTTON};

/// Two rows of buttons and the gap between them.
const RIBBON_HEIGHT: f32 = ICON_BUTTON * 2.0 + 6.0;

impl App {
    pub(super) fn toolbar(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        let frame = Frame::new()
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(12)
            .inner_margin(Margin::symmetric(10, 8))
            .shadow(Shadow { offset: [0, 2], blur: 10, spread: 0, color: p.shadow });
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Center the groups, using the width they took last frame.
            let width_id = ui.id().with("ribbon width");
            let last_width = ui.data(|d| d.get_temp::<f32>(width_id)).unwrap_or(0.0);
            let indent = ((ui.available_width() - last_width) / 2.0).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(indent);
                let editable = !self.tab().conversation.is_running();
                let groups = ui.add_enabled_ui(editable, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 6.0);
                        self.ribbon_groups(ui);
                    })
                    .response
                    .rect
                    .width()
                });
                if (groups.inner - last_width).abs() > 0.5 {
                    ui.data_mut(|d| d.insert_temp(width_id, groups.inner));
                    ui.ctx().request_repaint();
                }
            });
        });
    }

    fn ribbon_groups(&mut self, ui: &mut Ui) {
        let divider = |ui: &mut Ui| {
            ui.add_space(4.0);
            vertical_divider(ui, RIBBON_HEIGHT);
            ui.add_space(4.0);
        };
        self.styles_group(ui);
        divider(ui);
        self.font_group(ui);
        divider(ui);
        self.paragraph_group(ui);
        divider(ui);
        self.insert_group(ui);
    }

    fn styles_group(&mut self, ui: &mut Ui) {
        let editor = &mut self.tabs[self.active].editor;
        ui.vertical(|ui| {
            pickers::paragraph_style(ui, editor);
            ui.horizontal(|ui| {
                let kind = editor.current_paragraph_kind();
                let blocks = [
                    (ParagraphKind::Quote, Icon::Quote, "Quote"),
                    (ParagraphKind::Code, Icon::Code, "Code block"),
                ];
                for (block, icon, tip) in blocks {
                    let active = kind == block;
                    if icon_button(ui, icon, active).on_hover_text(tip).clicked() {
                        editor.set_paragraph_kind(if active { ParagraphKind::Normal } else { block });
                    }
                }
                if icon_button(ui, Icon::ClearFormatting, false).on_hover_text("Clear formatting").clicked() {
                    editor.clear_formatting();
                }
            });
        });
    }

    fn font_group(&mut self, ui: &mut Ui) {
        let editor = &mut self.tabs[self.active].editor;
        let style = editor.current_style();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                pickers::font_family(ui, editor);
                pickers::font_size(ui, editor);
                let scripts = [
                    (Script::Superscript, Icon::Superscript, "Superscript"),
                    (Script::Subscript, Icon::Subscript, "Subscript"),
                ];
                for (script, icon, tip) in scripts {
                    let active = style.script == script;
                    if icon_button(ui, icon, active).on_hover_text(tip).clicked() {
                        editor.apply_style(|s| s.script = if active { Script::Normal } else { script });
                    }
                }
            });
            ui.horizontal(|ui| {
                let toggles = [
                    (style.bold, Icon::Bold, StyleFlag::Bold, "Bold (Ctrl+B)"),
                    (style.italic, Icon::Italic, StyleFlag::Italic, "Italic (Ctrl+I)"),
                    (style.underline, Icon::Underline, StyleFlag::Underline, "Underline (Ctrl+U)"),
                    (
                        style.strikethrough,
                        Icon::Strikethrough,
                        StyleFlag::Strikethrough,
                        "Strikethrough (Ctrl+Shift+X)",
                    ),
                ];
                for (active, icon, flag, tip) in toggles {
                    if icon_button(ui, icon, active).on_hover_text(tip).clicked() {
                        editor.toggle_style(flag);
                    }
                }
                pickers::text_color(ui, editor);
                pickers::highlight(ui, editor);
            });
        });
    }

    fn paragraph_group(&mut self, ui: &mut Ui) {
        let editor = &mut self.tabs[self.active].editor;
        let paragraph = editor.current_paragraph();
        let (kind, alignment) = (paragraph.kind, paragraph.alignment);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                // List buttons toggle: clicking the active one goes back to normal text.
                let lists = [
                    (ParagraphKind::Bullet, Icon::BulletList, "Bulleted list"),
                    (ParagraphKind::Numbered, Icon::NumberedList, "Numbered list"),
                    (ParagraphKind::Checklist, Icon::Checklist, "Checklist"),
                ];
                for (list, icon, tip) in lists {
                    let active = kind == list;
                    if icon_button(ui, icon, active).on_hover_text(tip).clicked() {
                        editor.set_paragraph_kind(if active { ParagraphKind::Normal } else { list });
                    }
                }
                pickers::line_spacing(ui, editor);
            });
            ui.horizontal(|ui| {
                let alignments = [
                    (Alignment::Left, Icon::AlignLeft, "Align left (Ctrl+L)"),
                    (Alignment::Center, Icon::AlignCenter, "Center (Ctrl+E)"),
                    (Alignment::Right, Icon::AlignRight, "Align right (Ctrl+R)"),
                    (Alignment::Justify, Icon::AlignJustify, "Justify (Ctrl+J)"),
                ];
                for (choice, icon, tip) in alignments {
                    if icon_button(ui, icon, alignment == choice).on_hover_text(tip).clicked() {
                        editor.format_paragraphs(|p| p.alignment = choice);
                    }
                }
            });
        });
    }

    fn insert_group(&mut self, ui: &mut Ui) {
        if big_button(ui, Icon::Picture, "Picture").on_hover_text("Insert a picture from a file").clicked() {
            self.insert_picture();
        }
    }

    fn insert_picture(&mut self) {
        let Some(path) = FileDialog::new().add_filter("Picture", &["png", "jpg", "jpeg"]).pick_file() else {
            return;
        };
        let loaded = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
            let reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format();
            let (width, height) =
                reader.map_err(|e| e.to_string())?.into_dimensions().map_err(|e| e.to_string())?;
            Ok((bytes, width, height))
        });
        match loaded {
            Ok((bytes, width, height)) => {
                // Pictures are 96 pixels per inch; a point is 1/72 inch.
                let (width, height) = (width as f32 * 0.75, height as f32 * 0.75);
                let page = PageSetup::LETTER;
                let image =
                    Image::fitted(bytes, width, height, page.content_width(), page.content_height() * 0.7);
                self.tab_mut().editor.insert_image(image);
                self.tab_mut().view.reveal_caret();
            }
            Err(error) => self.status = format!("Couldn't insert the picture: {error}"),
        }
    }
}

/// A tall button spanning both rows: an icon with a label under it, like Word's.
fn big_button(ui: &mut Ui, icon: Icon, label: &str) -> eframe::egui::Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(52.0, RIBBON_HEIGHT), Sense::click());
    let enabled = ui.is_enabled();
    let fill = if enabled && response.hovered() { p.hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 8.0, fill);
    let color = if enabled { p.text_muted } else { p.text_faint };
    let icon_rect = Rect::from_center_size(rect.center() - vec2(0.0, 9.0), Vec2::splat(22.0));
    paint_icon(ui.painter(), icon_rect, icon, color);
    let text_pos = rect.center() + vec2(0.0, 16.0);
    ui.painter().text(text_pos, eframe::egui::Align2::CENTER_CENTER, label, theme::semibold(11.5), color);
    response.on_hover_cursor(CursorIcon::PointingHand)
}
