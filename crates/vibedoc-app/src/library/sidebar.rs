//! The library sidebar: one tree of folders, subfolders, and documents, with
//! search, drag-and-drop to move things, and naming in place.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    self, pos2, vec2, Align, CentralPanel, Color32, CursorIcon, FontId, Frame, Id, Key, LayerId, Layout,
    Margin, Order, Panel, Popup, Rect, Response, RichText, ScrollArea, Sense, Shadow, Stroke, StrokeKind,
    TextEdit, Ui,
};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use super::{display_name, Folder};
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette, Palette, HEADER_HEIGHT, ROW_HEIGHT, SUBHEADER_HEIGHT};
use crate::ui::widgets::{icon, icon_button};

/// How often the tree is re-read, to pick up changes made in Finder or Explorer.
const RESCAN_INTERVAL: Duration = Duration::from_secs(3);
const INDENT: f32 = 16.0;

/// What the app should do after the user worked in the sidebar.
pub enum SidebarAction {
    Open(PathBuf),
    /// A document or folder was renamed or moved. Open documents inside it follow along.
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    Deleted(PathBuf),
}

/// The drag-and-drop payload: the document or folder being dragged.
struct Dragged(PathBuf);

enum NameTarget {
    NewDocument { folder: PathBuf },
    NewFolder { parent: PathBuf },
    Rename(PathBuf),
}

struct NameField {
    target: NameTarget,
    text: String,
    focused: bool,
}

pub struct Sidebar {
    pub open: bool,
    root: PathBuf,
    tree: Folder,
    scanned: Option<Instant>,
    expanded: HashSet<PathBuf>,
    filter: String,
    naming: Option<NameField>,
    error: Option<String>,
    /// The open document the tree was last expanded to show.
    revealed: Option<PathBuf>,
    has_focus: bool,
    actions: Vec<SidebarAction>,
}

/// How one row of the tree looks.
struct Row<'a> {
    path: &'a Path,
    label: &'a str,
    depth: usize,
    leading: Leading,
    selected: bool,
    trailing: Trailing,
}

enum Leading {
    Tile(Color32),
    Icon(Icon),
}

enum Trailing {
    None,
    /// A document with unsaved changes.
    UnsavedDot,
    /// A folder's document count and expand arrow.
    Folder {
        count: usize,
        expanded: bool,
    },
    /// Muted text, like the folder a search result is in.
    Detail(String),
}

impl Sidebar {
    pub fn new(root: PathBuf) -> Sidebar {
        Sidebar {
            open: true,
            root,
            tree: Folder::default(),
            scanned: None,
            expanded: HashSet::new(),
            filter: String::new(),
            naming: None,
            error: None,
            revealed: None,
            has_focus: false,
            actions: Vec::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// True while the search box or a name field has the keyboard.
    pub fn wants_keyboard(&self) -> bool {
        self.has_focus
    }

    /// Re-reads the folder tree, e.g. after the app saved a new file into it.
    pub fn rescan(&mut self) {
        self.tree = Folder::scan(&self.root);
        self.scanned = Some(Instant::now());
    }

    pub fn show(&mut self, ui: &mut Ui, current: Option<&Path>, dirty: bool) -> Vec<SidebarAction> {
        if self.scanned.is_none_or(|at| at.elapsed() > RESCAN_INTERVAL) {
            self.rescan();
        }
        self.reveal(current);
        self.has_focus = false;

        Panel::top("library header").frame(Frame::NONE).show_separator_line(false).show(ui, |ui| {
            self.brand(ui);
            self.search_row(ui);
            ui.add_space(8.0);
        });
        CentralPanel::no_frame().show(ui, |ui| {
            if let Some(error) = self.error.clone() {
                let text = RichText::new(error).color(palette(ui).danger).small();
                if ui
                    .add(egui::Label::new(text).sense(Sense::click()))
                    .on_hover_text("Click to dismiss")
                    .clicked()
                {
                    self.error = None;
                }
            }
            let tree = self.tree.clone();
            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if self.filter.trim().is_empty() {
                    self.tree_rows(ui, &tree, current, dirty);
                } else {
                    self.search_results(ui, &tree, current, dirty);
                }
                // The empty space under the tree takes drops, to move things to the top level.
                let (rect, _) =
                    ui.allocate_exact_size(ui.available_size().max(vec2(1.0, 40.0)), Sense::hover());
                let response = ui.interact(rect, Id::new("library root drop"), Sense::hover());
                self.accept_drop(ui, &response, &self.root.clone());
            });
        });
        std::mem::take(&mut self.actions)
    }

    // ---- Header, search, and footer ----

    fn brand(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), HEADER_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(2.0);
                let (tile, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                ui.painter().rect_filled(tile, 8.0, p.accent);
                paint_icon(ui.painter(), tile.shrink(6.0), Icon::Sparkle, Color32::WHITE);
                ui.label(RichText::new("VibeDoc").font(theme::semibold(16.0)).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, Icon::Sidebar, false).on_hover_text("Hide sidebar (Ctrl+\\)").clicked()
                    {
                        self.open = false;
                    }
                });
            },
        );
    }

    /// The search box, with a button for new documents and folders beside it.
    fn search_row(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), SUBHEADER_HEIGHT),
            Layout::right_to_left(Align::Center),
            |ui| {
                let add = icon_button(ui, Icon::Plus, false).on_hover_text("New document or folder");
                Popup::menu(&add).show(|ui| {
                    ui.set_min_width(180.0);
                    if ui.button("New document").clicked() {
                        self.start_naming(NameTarget::NewDocument { folder: self.root.clone() });
                    }
                    if ui.button("New folder").clicked() {
                        self.start_naming(NameTarget::NewFolder { parent: self.root.clone() });
                    }
                });
                let frame = Frame::new().fill(p.field).corner_radius(8).inner_margin(Margin::symmetric(8, 0));
                frame.show(ui, |ui| {
                    ui.set_height(SUBHEADER_HEIGHT - 2.0);
                    ui.horizontal_centered(|ui| {
                        icon(ui, Icon::Search, 14.0, p.text_faint);
                        let field = TextEdit::singleline(&mut self.filter)
                            .hint_text("Search")
                            .frame(Frame::NONE)
                            .desired_width(f32::INFINITY);
                        let response = ui.add(field);
                        if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                            self.filter.clear();
                            response.surrender_focus();
                        }
                        self.has_focus |= response.has_focus();
                    });
                });
            },
        );
    }

    // ---- The tree ----

    /// Folders first, then documents at the top level, all in one list.
    fn tree_rows(&mut self, ui: &mut Ui, tree: &Folder, current: Option<&Path>, dirty: bool) {
        let root = &tree.path;
        self.name_field_for(ui, |t| matches!(t, NameTarget::NewFolder { parent } if parent == root), 0);
        for folder in &tree.folders {
            self.folder_rows(ui, folder, 0, current, dirty);
        }
        for document in &tree.documents {
            self.document_row(ui, document, 0, current, dirty, None);
        }
        self.name_field_for(ui, |t| matches!(t, NameTarget::NewDocument { folder } if folder == root), 0);
        if tree.folders.is_empty() && tree.documents.is_empty() && self.naming.is_none() {
            hint_row(ui, 0, "No documents yet. Use + to make one.");
        }
    }

    fn folder_rows(
        &mut self,
        ui: &mut Ui,
        folder: &Folder,
        depth: usize,
        current: Option<&Path>,
        dirty: bool,
    ) {
        let renaming = matches!(&self.naming, Some(f) if matches!(&f.target, NameTarget::Rename(p) if *p == folder.path));
        let expanded = self.expanded.contains(&folder.path);
        if renaming {
            self.name_field(ui, depth, Icon::Folder);
        } else {
            let p = palette(ui);
            let leading = if depth == 0 {
                Leading::Tile(tile_color(p, &folder.name))
            } else {
                Leading::Icon(Icon::Folder)
            };
            let trailing = Trailing::Folder { count: folder.document_count(), expanded };
            let row_spec =
                Row { path: &folder.path, label: &folder.name, depth, leading, selected: false, trailing };
            let response = row(ui, row_spec);
            if response.clicked() {
                self.toggle(&folder.path);
            }
            self.accept_drop(ui, &response, &folder.path);
            response.context_menu(|ui| self.folder_menu(ui, folder));
        }
        if !expanded {
            return;
        }
        let path = &folder.path;
        self.name_field_for(
            ui,
            |t| matches!(t, NameTarget::NewFolder { parent } if parent == path),
            depth + 1,
        );
        for child in &folder.folders {
            self.folder_rows(ui, child, depth + 1, current, dirty);
        }
        for document in &folder.documents {
            self.document_row(ui, document, depth + 1, current, dirty, None);
        }
        self.name_field_for(
            ui,
            |t| matches!(t, NameTarget::NewDocument { folder } if folder == path),
            depth + 1,
        );
        if folder.folders.is_empty() && folder.documents.is_empty() && self.naming.is_none() {
            hint_row(ui, depth + 1, "Empty. Drag documents here.");
        }
    }

    fn document_row(
        &mut self,
        ui: &mut Ui,
        path: &Path,
        depth: usize,
        current: Option<&Path>,
        dirty: bool,
        detail: Option<String>,
    ) {
        if matches!(&self.naming, Some(f) if matches!(&f.target, NameTarget::Rename(p) if p == path)) {
            self.name_field(ui, depth, Icon::Document);
            return;
        }
        let selected = current == Some(path);
        let trailing = match detail {
            Some(detail) => Trailing::Detail(detail),
            None if selected && dirty => Trailing::UnsavedDot,
            None => Trailing::None,
        };
        let label = display_name(path);
        let leading = Leading::Icon(Icon::Document);
        let response = row(ui, Row { path, label: &label, depth, leading, selected, trailing });
        if response.clicked() && !selected {
            self.actions.push(SidebarAction::Open(path.to_path_buf()));
        }
        response.context_menu(|ui| self.document_menu(ui, path));
    }

    fn search_results(&mut self, ui: &mut Ui, tree: &Folder, current: Option<&Path>, dirty: bool) {
        let query = self.filter.trim().to_lowercase();
        let matches: Vec<&Path> = tree
            .all_documents()
            .into_iter()
            .filter(|d| display_name(d).to_lowercase().contains(&query))
            .collect();
        if matches.is_empty() {
            hint_row(ui, 0, "No documents match.");
        }
        for document in matches {
            let folder = document.parent().and_then(|p| p.strip_prefix(&self.root).ok());
            let detail = folder.filter(|f| !f.as_os_str().is_empty()).map(|f| f.display().to_string());
            self.document_row(ui, document, 0, current, dirty, Some(detail.unwrap_or_default()));
        }
    }

    // ---- Menus ----

    fn document_menu(&mut self, ui: &mut Ui, path: &Path) {
        if ui.button("Open").clicked() {
            self.actions.push(SidebarAction::Open(path.to_path_buf()));
            ui.close();
        }
        if ui.button("Rename").clicked() {
            self.start_naming(NameTarget::Rename(path.to_path_buf()));
            ui.close();
        }
        self.move_menu(ui, path);
        ui.separator();
        if ui.button(RichText::new("Delete").color(palette(ui).danger)).clicked() {
            ui.close();
            let question = format!("Delete “{}”? This can't be undone.", display_name(path));
            if confirm("Delete document", &question) {
                self.delete(path);
            }
        }
    }

    fn folder_menu(&mut self, ui: &mut Ui, folder: &Folder) {
        if ui.button("New document").clicked() {
            self.start_naming(NameTarget::NewDocument { folder: folder.path.clone() });
            ui.close();
        }
        if ui.button("New subfolder").clicked() {
            self.start_naming(NameTarget::NewFolder { parent: folder.path.clone() });
            ui.close();
        }
        if ui.button("Rename").clicked() {
            self.start_naming(NameTarget::Rename(folder.path.clone()));
            ui.close();
        }
        self.move_menu(ui, &folder.path);
        ui.separator();
        if ui.button(RichText::new("Delete folder").color(palette(ui).danger)).clicked() {
            ui.close();
            let question = match folder.document_count() {
                0 => format!("Delete the folder “{}”?", folder.name),
                1 => format!("Delete “{}” and the document in it? This can't be undone.", folder.name),
                count => {
                    format!("Delete “{}” and the {count} documents in it? This can't be undone.", folder.name)
                }
            };
            if confirm("Delete folder", &question) {
                self.delete(&folder.path);
            }
        }
    }

    /// "Move to ▸" with every folder in the library, indented to show nesting.
    fn move_menu(&mut self, ui: &mut Ui, path: &Path) {
        let library_name = display_name(&self.root);
        let targets: Vec<(usize, PathBuf, String)> = self
            .tree
            .all_folders()
            .into_iter()
            .filter(|(_, folder)| !folder.path.starts_with(path))
            .map(|(depth, folder)| {
                let name =
                    if depth == 0 { format!("{library_name} (top level)") } else { folder.name.clone() };
                (depth, folder.path.clone(), name)
            })
            .collect();
        let parent = path.parent().map(Path::to_path_buf);
        ui.menu_button("Move to", |ui| {
            for (depth, folder, name) in targets {
                let label = format!("{}{name}", "    ".repeat(depth));
                let here = parent.as_ref() == Some(&folder);
                if ui.add_enabled(!here, egui::Button::new(label)).clicked() {
                    self.move_item(path, &folder);
                    ui.close();
                }
            }
        });
    }

    // ---- Changing the library ----

    fn start_naming(&mut self, target: NameTarget) {
        let text = match &target {
            NameTarget::Rename(path) => display_name(path),
            _ => String::new(),
        };
        if let NameTarget::NewDocument { folder: parent } | NameTarget::NewFolder { parent } = &target {
            self.expanded.insert(parent.clone());
            self.filter.clear();
        }
        self.naming = Some(NameField { target, text, focused: false });
    }

    /// Shows the name field here if it's for a new item matching `is_here`.
    fn name_field_for(&mut self, ui: &mut Ui, is_here: impl Fn(&NameTarget) -> bool, depth: usize) {
        if let Some(field) = &self.naming {
            if is_here(&field.target) {
                let icon = match field.target {
                    NameTarget::NewFolder { .. } => Icon::Folder,
                    _ => Icon::Document,
                };
                self.name_field(ui, depth, icon);
            }
        }
    }

    /// An inline text box for naming. Enter saves, Escape or clicking away cancels.
    fn name_field(&mut self, ui: &mut Ui, depth: usize, icon: Icon) {
        let p = palette(ui);
        let Some(field) = self.naming.as_mut() else { return };
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
        ui.painter().rect(rect, 8.0, p.surface, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        let icon_x = rect.left() + 8.0 + depth as f32 * INDENT;
        let icon_rect = Rect::from_min_size(pos2(icon_x + 1.0, rect.center().y - 8.0), vec2(16.0, 16.0));
        paint_icon(ui.painter(), icon_rect, icon, p.accent);
        let hint = match field.target {
            NameTarget::NewFolder { .. } => "Folder name",
            _ => "Document name",
        };
        let edit_rect = Rect::from_min_max(
            pos2(icon_rect.right() + 6.0, rect.top() + 3.0),
            pos2(rect.right() - 6.0, rect.bottom() - 3.0),
        );
        let edit = TextEdit::singleline(&mut field.text)
            .hint_text(hint)
            .frame(Frame::NONE)
            .margin(Margin::symmetric(2, 4));
        let response = ui.put(edit_rect, edit);
        if !field.focused {
            response.request_focus();
            field.focused = true;
        }
        self.has_focus |= response.has_focus();
        if response.lost_focus() {
            if ui.input(|i| i.key_pressed(Key::Enter)) {
                self.finish_naming();
            } else {
                self.naming = None;
            }
        }
    }

    fn finish_naming(&mut self) {
        let Some(field) = self.naming.take() else { return };
        let text = field.text.trim();
        let result = match field.target {
            NameTarget::NewDocument { folder } => {
                super::create_document(&folder, if text.is_empty() { "Untitled" } else { text })
                    .map(|path| self.actions.push(SidebarAction::Open(path)))
            }
            NameTarget::NewFolder { parent } => {
                super::create_folder(&parent, if text.is_empty() { "New folder" } else { text }).map(|path| {
                    self.expanded.insert(path);
                })
            }
            NameTarget::Rename(path) => super::rename(&path, text).map(|to| self.moved(path, to)),
        };
        self.finish(result);
    }

    fn move_item(&mut self, from: &Path, into: &Path) {
        let result = super::move_into(from, into).map(|to| {
            self.expanded.insert(into.to_path_buf());
            self.moved(from.to_path_buf(), to);
        });
        self.finish(result);
    }

    fn moved(&mut self, from: PathBuf, to: PathBuf) {
        if from != to {
            if self.expanded.remove(&from) {
                self.expanded.insert(to.clone());
            }
            self.actions.push(SidebarAction::Moved { from, to });
        }
    }

    fn delete(&mut self, path: &Path) {
        let result =
            super::delete(path).map(|()| self.actions.push(SidebarAction::Deleted(path.to_path_buf())));
        self.finish(result);
    }

    fn finish(&mut self, result: Result<(), String>) {
        self.error = result.err();
        self.rescan();
    }

    fn accept_drop(&mut self, ui: &Ui, response: &Response, folder: &Path) {
        let can_drop =
            |dragged: &Dragged| !folder.starts_with(&dragged.0) && dragged.0.parent() != Some(folder);
        if response.dnd_hover_payload::<Dragged>().is_some_and(|d| can_drop(&d)) {
            let stroke = Stroke::new(1.5, palette(ui).accent);
            ui.painter().rect_stroke(response.rect, 8.0, stroke, StrokeKind::Inside);
        }
        if let Some(dragged) = response.dnd_release_payload::<Dragged>() {
            if can_drop(&dragged) {
                self.move_item(&dragged.0, folder);
            }
        }
    }

    /// Asks for a different library folder. Returns it if one was picked.
    pub fn pick_root(&mut self) -> Option<PathBuf> {
        let root = FileDialog::new().set_directory(&self.root).pick_folder()?;
        self.root = root.clone();
        self.expanded.clear();
        self.rescan();
        Some(root)
    }

    fn toggle(&mut self, folder: &Path) {
        if !self.expanded.remove(folder) {
            self.expanded.insert(folder.to_path_buf());
        }
    }

    /// Expands the folders around a newly opened document so it's visible.
    fn reveal(&mut self, current: Option<&Path>) {
        if self.revealed.as_deref() == current {
            return;
        }
        self.revealed = current.map(Path::to_path_buf);
        let Some(current) = current else { return };
        for folder in current.ancestors().skip(1).take_while(|f| f.starts_with(&self.root)) {
            self.expanded.insert(folder.to_path_buf());
        }
    }
}

/// One row of the tree. Rows can be clicked, and dragged onto folders.
fn row(ui: &mut Ui, row: Row) -> Response {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let id = Id::new(("library row", row.path));
    let response = ui.interact(rect, id, Sense::click_and_drag());
    response.dnd_set_drag_payload(Dragged(row.path.to_path_buf()));
    let painter = ui.painter();

    // Guide lines connect children to their folder.
    for level in 0..row.depth {
        let x = rect.left() + 8.0 + level as f32 * INDENT + 9.0;
        painter.line_segment(
            [pos2(x, rect.top() - 1.0), pos2(x, rect.bottom() + 1.0)],
            Stroke::new(1.0, p.border),
        );
    }
    let content_left = rect.left() + 8.0 + row.depth as f32 * INDENT;
    let background = Rect::from_min_max(pos2(content_left - 4.0, rect.top()), rect.max);
    if row.selected {
        painter.rect_filled(background, 8.0, p.accent_soft);
    } else if response.hovered() {
        painter.rect_filled(background, 8.0, p.hover);
    }

    let leading = Rect::from_min_size(pos2(content_left + 1.0, rect.center().y - 9.0), vec2(18.0, 18.0));
    match row.leading {
        Leading::Tile(color) => {
            painter.rect_filled(leading, 5.0, color);
            let initial = row.label.chars().next().unwrap_or('?').to_uppercase().to_string();
            painter.text(
                leading.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                theme::semibold(10.5),
                Color32::WHITE,
            );
        }
        Leading::Icon(icon) => {
            let color = if row.selected { p.accent } else { p.text_muted };
            paint_icon(painter, leading.shrink(1.5), icon, color);
        }
    }

    let mut right = rect.right() - 8.0;
    match &row.trailing {
        Trailing::None => {}
        Trailing::UnsavedDot => {
            painter.circle_filled(pos2(right - 4.0, rect.center().y), 3.5, p.accent);
            right -= 14.0;
        }
        Trailing::Folder { count, expanded } => {
            let chevron = Rect::from_center_size(pos2(right - 6.0, rect.center().y), vec2(13.0, 13.0));
            paint_icon(
                painter,
                chevron,
                if *expanded { Icon::ChevronDown } else { Icon::ChevronRight },
                p.text_faint,
            );
            right -= 20.0;
            right -= paint_count(painter, p, pos2(right, rect.center().y), *count) + 6.0;
        }
        Trailing::Detail(detail) => {
            if !detail.is_empty() {
                let font = FontId::proportional(11.5);
                let width =
                    painter.layout_no_wrap(detail.clone(), font.clone(), p.text_faint).size().x.min(90.0);
                paint_truncated(
                    painter,
                    pos2(right - width, rect.center().y),
                    detail,
                    width,
                    font,
                    p.text_faint,
                );
                right -= width + 8.0;
            }
        }
    }

    let color = if row.selected { p.text } else { p.text.gamma_multiply(0.86) };
    let font = if row.selected { theme::semibold(13.5) } else { FontId::proportional(13.5) };
    let label_left = leading.right() + 9.0;
    paint_truncated(painter, pos2(label_left, rect.center().y), row.label, right - label_left, font, color);

    if ui.ctx().is_being_dragged(id) {
        paint_drag_ghost(ui, p, id, row.label, &row.leading);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// The floating label that follows the pointer while dragging.
fn paint_drag_ghost(ui: &Ui, p: &Palette, id: Id, label: &str, leading: &Leading) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else { return };
    let painter = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, id.with("ghost")));
    let galley = painter.layout_no_wrap(label.to_string(), theme::semibold(13.0), p.text);
    let rect = Rect::from_min_size(pointer + vec2(14.0, 8.0), galley.size() + vec2(42.0, 16.0));
    let shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(40) };
    painter.add(shadow.as_shape(rect, 9.0));
    painter.rect(rect, 9.0, p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let icon = match leading {
        Leading::Tile(_) => Icon::Folder,
        Leading::Icon(icon) => *icon,
    };
    let icon_rect = Rect::from_min_size(rect.min + vec2(12.0, rect.height() / 2.0 - 7.0), vec2(14.0, 14.0));
    paint_icon(&painter, icon_rect, icon, p.accent);
    painter.galley(rect.min + vec2(32.0, 8.0), galley, p.text);
}

/// A muted, non-interactive line of help text inside the tree.
fn hint_row(ui: &mut Ui, depth: usize, text: &str) {
    let color = palette(ui).text_faint;
    ui.horizontal(|ui| {
        ui.add_space(12.0 + depth as f32 * INDENT);
        ui.label(RichText::new(text).color(color).size(12.0));
    });
}

/// Paints single-line text vertically centered on `left_center`, cut off with "…" past `max_width`.
fn paint_truncated(
    painter: &egui::Painter,
    left_center: egui::Pos2,
    text: &str,
    max_width: f32,
    font: FontId,
    color: Color32,
) {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = TextWrapping::truncate_at_width(max_width.max(10.0));
    let galley = painter.layout_job(job);
    painter.galley(left_center - vec2(0.0, galley.size().y / 2.0), galley, color);
}

/// A count badge ending at `right_center`. Returns its width.
fn paint_count(painter: &egui::Painter, p: &Palette, right_center: egui::Pos2, count: usize) -> f32 {
    let galley = painter.layout_no_wrap(count.to_string(), theme::semibold(10.5), p.text_muted);
    let width = galley.size().x + 12.0;
    let rect = Rect::from_min_size(right_center - vec2(width, 9.0), vec2(width, 18.0));
    painter.rect_filled(rect, 9.0, p.field);
    painter.galley(rect.center() - galley.size() / 2.0, galley, p.text_muted);
    width
}

fn tile_color(p: &Palette, name: &str) -> Color32 {
    let sum: usize = name.bytes().map(usize::from).sum();
    p.tiles[sum % p.tiles.len()]
}

fn confirm(title: &str, question: &str) -> bool {
    MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title(title)
        .set_description(question)
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

pub fn reveal_in_file_manager(path: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    // Revealing a file opens its folder; Finder can select it with -R.
    let mut command = std::process::Command::new(program);
    if path.is_file() && cfg!(target_os = "macos") {
        command.arg("-R");
    }
    let _ = command.arg(path).spawn();
}
