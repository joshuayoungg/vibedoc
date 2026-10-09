//! The File menu: documents (new, open, save, export, copy, rename, close),
//! the library folder, and appearance (theme and page color).

use eframe::egui::{pos2, vec2, Color32, CursorIcon, FontId, Rect, Response, RichText, Sense, Stroke, Ui};

use super::preferences::{PageColor, Theme};
use super::App;
use crate::library::sidebar::reveal_in_file_manager;
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette};

impl App {
    pub(super) fn file_menu(&mut self, ui: &mut Ui) {
        ui.set_min_width(250.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        if menu_item(ui, Icon::Plus, "New document", &shortcut("N")).clicked() {
            self.new_tab();
            ui.close();
        }
        if menu_item(ui, Icon::FolderOpen, "Open…", &shortcut("O")).clicked() {
            ui.close();
            self.open();
        }
        ui.separator();
        if menu_item(ui, Icon::Save, "Save", &shortcut("S")).clicked() {
            ui.close();
            self.save();
        }
        if menu_item(ui, Icon::Save, "Save as…", &shortcut("Shift+S")).clicked() {
            ui.close();
            self.save_as();
        }
        if menu_item(ui, Icon::File, "Export as PDF…", &shortcut("P")).clicked() {
            ui.close();
            self.export_pdf(ui.ctx());
        }
        if menu_item(ui, Icon::Copy, "Make a copy", "").clicked() {
            ui.close();
            self.make_copy();
        }
        if menu_item(ui, Icon::Pencil, "Rename", "").clicked() {
            ui.close();
            self.start_rename();
        }
        if let Some(file) = self.tab().file.clone() {
            let label = if cfg!(target_os = "macos") { "Show in Finder" } else { "Show in folder" };
            if menu_item(ui, Icon::Folder, label, "").clicked() {
                ui.close();
                reveal_in_file_manager(&file);
            }
        }
        ui.separator();
        if menu_item(ui, Icon::Close, "Close tab", &shortcut("W")).clicked() {
            ui.close();
            self.close_tab(self.active);
        }
        ui.separator();
        if menu_item(ui, Icon::FolderOpen, "Change library folder…", "").clicked() {
            ui.close();
            if let Some(root) = self.sidebar.pick_root() {
                self.preferences.library_folder = Some(root);
                self.save_preferences();
            }
        }
        let reveal = if cfg!(target_os = "macos") { "Show library in Finder" } else { "Show library folder" };
        if menu_item(ui, Icon::Folder, reveal, "").clicked() {
            ui.close();
            let root = self.sidebar.root().to_path_buf();
            let _ = std::fs::create_dir_all(&root);
            reveal_in_file_manager(&root);
        }
        ui.separator();
        self.appearance(ui);
        ui.separator();
        if menu_item(ui, Icon::Settings, "Assistant settings…", "").clicked() {
            ui.close();
            self.assistant.open_settings();
        }
    }

    fn appearance(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        ui.add_space(4.0);
        ui.label(RichText::new("   Theme").font(theme::semibold(11.5)).color(p.text_faint));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(6.0);
            for choice in Theme::ALL {
                let icon = match choice {
                    Theme::Light => Icon::Sun,
                    Theme::Dark => Icon::Moon,
                    Theme::System => Icon::Monitor,
                };
                let selected = self.preferences.theme == choice;
                if segment(ui, icon, choice.label(), selected).clicked() && !selected {
                    self.preferences.theme = choice;
                    self.save_preferences();
                }
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new("   Page color").font(theme::semibold(11.5)).color(p.text_faint));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add_space(10.0);
            for page in PageColor::ALL {
                let selected = self.preferences.page == page;
                if page_swatch(ui, page.fill(), selected).on_hover_text(page.label()).clicked() {
                    self.preferences.page = page;
                    self.save_preferences();
                }
            }
        });
        ui.add_space(4.0);
    }
}

/// "⌘S" on a Mac, "Ctrl+S" elsewhere.
fn shortcut(keys: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{}", keys.replace("Shift+", "⇧"))
    } else {
        format!("Ctrl+{keys}")
    }
}

/// A menu row: icon, label, and its shortcut on the right.
fn menu_item(ui: &mut Ui, icon: Icon, label: &str, keys: &str) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, p.hover);
    }
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(15.0, 15.0));
    paint_icon(ui.painter(), icon_rect, icon, p.text_muted);
    let font = FontId::proportional(13.5);
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        eframe::egui::Align2::LEFT_CENTER,
        label,
        font,
        p.text,
    );
    if !keys.is_empty() {
        let font = FontId::proportional(12.0);
        let pos = pos2(rect.right() - 10.0, rect.center().y);
        ui.painter().text(pos, eframe::egui::Align2::RIGHT_CENTER, keys, font, p.text_faint);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// One button of the theme switch.
fn segment(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(74.0, 30.0), Sense::click());
    let fill = if selected {
        p.accent_soft
    } else if response.hovered() {
        p.hover
    } else {
        p.field
    };
    let color = if selected { p.accent } else { p.text_muted };
    ui.painter().rect_filled(rect, 8.0, fill);
    let icon_rect = Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), vec2(13.0, 13.0));
    paint_icon(ui.painter(), icon_rect, icon, color);
    let font = theme::semibold(12.0);
    ui.painter().text(
        pos2(rect.left() + 29.0, rect.center().y),
        eframe::egui::Align2::LEFT_CENTER,
        label,
        font,
        color,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A round swatch of paper color, ringed when chosen.
fn page_swatch(ui: &mut Ui, fill: Color32, selected: bool) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let center = rect.center();
    if selected {
        ui.painter().circle_stroke(center, 12.0, Stroke::new(2.0, p.accent));
    }
    ui.painter().circle(center, 8.5, fill, Stroke::new(1.0, p.border));
    response.on_hover_cursor(CursorIcon::PointingHand)
}
