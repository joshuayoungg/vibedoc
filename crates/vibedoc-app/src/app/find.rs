//! Find: a small search pill floating over the top-right of the pages.

use eframe::egui::{self, Align2, Frame, Key, Margin, Rect, RichText, Stroke, TextEdit, Ui};
use vibedoc_core::find::find_all;
use vibedoc_core::{Position, Selection};

use super::App;
use crate::ui::icons::Icon;
use crate::ui::theme::palette;
use crate::ui::widgets::{icon, icon_button};

#[derive(Default)]
pub struct FindBar {
    pub open: bool,
    query: String,
    pub matches: Vec<(Position, Position)>,
    current: usize,
    pub has_focus: bool,
    wants_focus: bool,
    /// The query and document revision `matches` was computed for. Cleared
    /// when switching tabs.
    pub searched: Option<(String, u64)>,
}

impl App {
    pub(super) fn find_overlay(&mut self, ui: &Ui, canvas: Rect) {
        self.refresh_matches();
        let p = palette(ui);
        egui::Area::new(egui::Id::new("find bar"))
            .order(egui::Order::Foreground)
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(canvas.right_top() + egui::vec2(-12.0, 12.0))
            .show(ui.ctx(), |ui| {
                let frame = Frame::popup(ui.style())
                    .fill(p.surface)
                    .stroke(Stroke::new(1.0, p.border))
                    .corner_radius(12)
                    .inner_margin(Margin { left: 10, right: 4, top: 2, bottom: 2 });
                frame.show(ui, |ui| ui.horizontal(|ui| self.find_controls(ui)));
            });
    }

    fn find_controls(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        icon(ui, Icon::Search, 14.0, p.text_faint);
        let field = TextEdit::singleline(&mut self.find.query)
            .hint_text("Find in document")
            .frame(Frame::NONE)
            .desired_width(180.0);
        let response = ui.add(field);
        if std::mem::take(&mut self.find.wants_focus) {
            response.request_focus();
        }
        self.find.has_focus = response.has_focus();
        let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

        let count = self.find.matches.len();
        let summary = match count {
            0 if self.find.query.is_empty() => String::new(),
            0 => "No results".into(),
            _ => format!("{} of {count}", self.find.current + 1),
        };
        ui.label(RichText::new(summary).small().color(p.text_muted));
        ui.add_enabled_ui(count > 0, |ui| {
            if icon_button(ui, Icon::ChevronUp, false).on_hover_text("Previous").clicked() {
                self.go_to_match(self.find.current + count - 1);
            }
            if icon_button(ui, Icon::ChevronDown, false).on_hover_text("Next (Enter)").clicked() {
                self.go_to_match(self.find.current + 1);
            }
        });
        if enter_pressed && count > 0 {
            self.go_to_match(self.find.current + 1);
            self.find.wants_focus = true;
        }
        if icon_button(ui, Icon::Close, false).on_hover_text("Close (Esc)").clicked() {
            self.close_find();
        }
    }

    pub(super) fn open_find(&mut self) {
        self.find.open = true;
        self.find.wants_focus = true;
        let editor = &self.tab().editor;
        if !editor.selection().is_empty() {
            self.find.query = editor.selected_text();
        }
    }

    pub(super) fn close_find(&mut self) {
        self.find.open = false;
        self.find.has_focus = false;
    }

    fn refresh_matches(&mut self) {
        let key = (self.find.query.clone(), self.tab().editor.revision());
        if self.find.searched.as_ref() == Some(&key) {
            return;
        }
        let query_changed = self.find.searched.as_ref().is_none_or(|(query, _)| *query != key.0);
        self.find.matches = find_all(self.tab().editor.document(), &self.find.query);
        self.find.current = self.find.current.min(self.find.matches.len().saturating_sub(1));
        self.find.searched = Some(key);
        // A new search jumps to the first match at or after the caret, like most editors.
        // (Editing the document only refreshes the highlights.)
        if query_changed {
            let caret = self.tab().editor.selection().start();
            let index = self.find.matches.iter().position(|(start, _)| *start >= caret).unwrap_or(0);
            self.go_to_match(index);
        }
    }

    fn go_to_match(&mut self, index: usize) {
        let count = self.find.matches.len();
        if count == 0 {
            return;
        }
        self.find.current = index % count;
        let (start, end) = self.find.matches[self.find.current];
        let tab = self.tab_mut();
        tab.editor.set_selection(Selection { anchor: start, cursor: end });
        tab.view.reveal_caret();
    }
}
