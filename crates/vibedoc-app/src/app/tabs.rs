//! Open documents as tabs: each tab has its own editor, view, file, and
//! assistant conversation. "+" opens a new untitled document beside the others.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{pos2, vec2, Align, CursorIcon, FontId, Layout, Rect, Sense, Ui};
use vibedoc_core::{Document, Editor};

use super::App;
use crate::assistant::Conversation;
use crate::document::DocumentView;
use crate::library::display_name;
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette, SUBHEADER_HEIGHT};
use crate::ui::widgets::icon_button;

const MAX_TAB_WIDTH: f32 = 200.0;
const MIN_TAB_WIDTH: f32 = 96.0;

pub struct Tab {
    pub editor: Editor,
    pub view: DocumentView,
    pub file: Option<PathBuf>,
    /// The file's name without ".docx", or "Untitled 2" before it's saved.
    pub name: String,
    pub saved_revision: u64,
    pub conversation: Conversation,
    /// The revision last seen and when it appeared, so saving waits until typing pauses.
    pub changed: (u64, Instant),
}

impl Tab {
    pub fn untitled(name: String) -> Tab {
        Tab::new(name, Document::default(), None)
    }

    pub fn new(name: String, document: Document, file: Option<PathBuf>) -> Tab {
        let editor = Editor::new(document);
        let saved_revision = editor.revision();
        Tab {
            editor,
            view: DocumentView::default(),
            file,
            name,
            saved_revision,
            conversation: Conversation::default(),
            changed: (saved_revision, Instant::now()),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.editor.revision() != self.saved_revision
    }

    /// Whether there's anything worth saving: untitled documents wait for their first words.
    pub fn has_content(&self) -> bool {
        self.file.is_some() || self.editor.document().paragraphs.iter().any(|p| !p.is_empty())
    }

    /// A blank untitled document nobody has touched, which opening a file can replace.
    fn is_pristine(&self) -> bool {
        self.file.is_none() && !self.is_dirty() && !self.conversation.is_running()
    }
}

/// How long after the last edit a document saves itself.
const AUTOSAVE_DELAY: Duration = Duration::from_millis(1200);

impl App {
    /// Saves each changed document once typing has paused for a moment.
    /// New documents save into the library once they have some text.
    pub(super) fn autosave(&mut self, ctx: &eframe::egui::Context) {
        for index in 0..self.tabs.len() {
            let tab = &mut self.tabs[index];
            let revision = tab.editor.revision();
            if tab.changed.0 != revision {
                tab.changed = (revision, Instant::now());
            }
            // While the assistant works, its result is still to come.
            if !tab.is_dirty() || !tab.has_content() || tab.conversation.is_running() {
                continue;
            }
            let waited = tab.changed.1.elapsed();
            if waited >= AUTOSAVE_DELAY {
                self.save_tab(index);
            } else {
                ctx.request_repaint_after(AUTOSAVE_DELAY - waited);
            }
        }
    }

    pub(super) fn new_tab(&mut self) {
        let name = self.unused_untitled_name();
        self.push_tab(Tab::untitled(name));
    }

    pub(super) fn push_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.activate(self.tabs.len() - 1);
    }

    pub(super) fn activate(&mut self, index: usize) {
        if index < self.tabs.len() && index != self.active {
            self.active = index;
            self.title_edit = None;
            self.find.searched = None;
        }
    }

    /// "Untitled", then "Untitled 2", ... skipping names already open.
    fn unused_untitled_name(&self) -> String {
        (1..)
            .map(|n| if n == 1 { "Untitled".to_string() } else { format!("Untitled {n}") })
            .find(|name| !self.tabs.iter().any(|t| t.file.is_none() && t.name == *name))
            .expect("some numbered name is free")
    }

    /// Switches to the file if it's already open, or opens it in a new tab
    /// (reusing a blank untitled one).
    pub(super) fn open_in_tab(&mut self, path: PathBuf) {
        if let Some(index) = self.tabs.iter().position(|t| t.file.as_deref() == Some(path.as_path())) {
            self.activate(index);
            return;
        }
        match vibedoc_docx::open(&path) {
            Ok(document) => {
                let tab = Tab::new(display_name(&path), document, Some(path));
                if self.tab().is_pristine() {
                    self.tabs[self.active] = tab;
                    self.title_edit = None;
                    self.find.searched = None;
                } else {
                    self.push_tab(tab);
                }
                self.status.clear();
            }
            Err(error) => self.status = format!("Couldn't open {}: {error}", path.display()),
        }
    }

    pub(super) fn next_tab(&mut self, step: isize) {
        let count = self.tabs.len() as isize;
        self.activate(((self.active as isize + step).rem_euclid(count)) as usize);
    }

    pub(super) fn tab_bar(&mut self, ui: &mut Ui) {
        let mut activate = None;
        let mut close = None;
        let size = vec2(ui.available_width(), SUBHEADER_HEIGHT);
        ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Tabs fit their names, and share the room evenly when there are many.
            let room = ui.available_width() - 40.0;
            let share = (room / self.tabs.len() as f32 - 4.0).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
            for (index, tab) in self.tabs.iter().enumerate() {
                match tab_pill(ui, tab, index == self.active, share) {
                    TabClick::Activate => activate = Some(index),
                    TabClick::Close => close = Some(index),
                    TabClick::None => {}
                }
            }
            if icon_button(ui, Icon::Plus, false).on_hover_text("New document (Ctrl+N)").clicked() {
                self.new_tab();
            }
        });
        if let Some(index) = activate {
            self.activate(index);
        }
        if let Some(index) = close {
            self.close_tab(index);
        }
    }
}

enum TabClick {
    None,
    Activate,
    Close,
}

/// One tab: a colored marker, the name, and a close button (or a dot when unsaved).
fn tab_pill(ui: &mut Ui, tab: &Tab, active: bool, max_width: f32) -> TabClick {
    let p = palette(ui);
    let name_width = ui.painter().layout_no_wrap(tab.name.clone(), theme::semibold(13.0), p.text).size().x;
    let width = (name_width + 56.0).min(max_width);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let close_rect = Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y), vec2(18.0, 18.0));
    let over_close = response.hover_pos().is_some_and(|pos| close_rect.contains(pos));
    let painter = ui.painter();
    let fill = if active {
        p.field
    } else if response.hovered() {
        p.hover
    } else {
        eframe::egui::Color32::TRANSPARENT
    };
    painter.rect_filled(rect, 8.0, fill);

    let marker = Rect::from_center_size(pos2(rect.left() + 12.0, rect.center().y), vec2(3.0, 14.0));
    painter.rect_filled(marker, 1.5, marker_color(p, tab));

    let color = if active { p.text } else { p.text_muted };
    let font = if active { theme::semibold(13.0) } else { FontId::proportional(13.0) };
    let mut job = LayoutJob::simple_singleline(tab.name.clone(), font, color);
    job.wrap = TextWrapping::truncate_at_width(width - 52.0);
    let galley = painter.layout_job(job);
    painter.galley(pos2(rect.left() + 22.0, rect.center().y - galley.size().y / 2.0), galley, color);

    // Unsaved tabs show a dot until you reach for the close button.
    if tab.is_dirty() && !over_close {
        painter.circle_filled(close_rect.center(), 3.5, p.accent);
    } else if active || response.hovered() {
        if over_close {
            painter.rect_filled(close_rect, 5.0, p.pressed);
        }
        paint_icon(painter, close_rect.shrink(4.0), Icon::Close, p.text_muted);
    }

    let hint =
        tab.file.as_deref().map(Path::display).map_or_else(|| "Not saved yet".to_string(), |d| d.to_string());
    let response = response.on_hover_text(hint).on_hover_cursor(CursorIcon::PointingHand);
    let middle_clicked = response.middle_clicked();
    if (response.clicked() && over_close) || middle_clicked {
        TabClick::Close
    } else if response.clicked() {
        TabClick::Activate
    } else {
        TabClick::None
    }
}

/// Each document keeps one marker color, picked from its name.
fn marker_color(p: &theme::Palette, tab: &Tab) -> eframe::egui::Color32 {
    let sum: usize = tab.name.bytes().map(usize::from).sum();
    p.tiles[sum % p.tiles.len()]
}
