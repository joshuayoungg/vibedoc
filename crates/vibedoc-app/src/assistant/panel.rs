//! The assistant side panel: a card for each request, showing what changed,
//! with "See details" and Undo / Redo, and a box to ask for the next edit.

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, vec2, Align, CentralPanel, Color32, CursorIcon, Frame, Key, KeyboardShortcut, Layout, Margin,
    Modifiers, Panel, Rect, RichText, ScrollArea, Sense, Spinner, Stroke, StrokeKind, TextEdit, TextFormat,
    Ui,
};
use vibedoc_core::Editor;

use super::{diff_view, Action, Assistant, ChangeState, Conversation, Outcome};
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette, Palette, HEADER_HEIGHT, SUBHEADER_HEIGHT};
use crate::ui::widgets::{button, icon, icon_button, ButtonKind};

const EXAMPLES: [&str; 4] = [
    "Turn my notes into a bulleted list",
    "Make the selected text bold",
    "Draw a watercolor fox and put it here",
    "Add a title and a short intro",
];

impl Assistant {
    pub fn panel(&mut self, ui: &mut Ui, conversation: &mut Conversation, editor: &Editor) -> Option<Action> {
        let mut action = None;
        Panel::top("assistant header").frame(Frame::NONE).show_separator_line(false).show(ui, |ui| {
            self.header(ui, conversation, editor);
        });
        Panel::bottom("assistant composer")
            .frame(Frame::NONE.inner_margin(Margin { top: 10, ..Default::default() }))
            .show_separator_line(false)
            .show(ui, |ui| self.composer(ui, conversation, editor));
        CentralPanel::no_frame().show(ui, |ui| {
            ScrollArea::vertical().stick_to_bottom(true).auto_shrink(false).show(ui, |ui| {
                if let Some(notice) = &self.notice {
                    ui.label(RichText::new(notice).color(palette(ui).danger).small());
                    ui.add_space(8.0);
                }
                if conversation.entries.is_empty() {
                    self.empty_state(ui, conversation, editor);
                } else {
                    action = self.cards(ui, conversation, editor);
                }
            });
        });
        action
    }

    fn header(&mut self, ui: &mut Ui, conversation: &mut Conversation, editor: &Editor) {
        let p = palette(ui);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), HEADER_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Assistant").font(theme::semibold(16.0)).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, Icon::Close, false).on_hover_text("Close").clicked() {
                        self.panel_open = false;
                    }
                    let tip = format!(
                        "Settings · {} · images: {}",
                        self.config.agent.model, self.config.images.model
                    );
                    if icon_button(ui, Icon::Settings, false).on_hover_text(tip).clicked() {
                        self.open_settings();
                    }
                });
            },
        );

        // A status strip, like a review tool's: how many edits are in, and what's happening now.
        let applied = conversation
            .entries
            .iter_mut()
            .map(|e| match &mut e.outcome {
                Outcome::Changed(change) => matches!(change.state(editor), ChangeState::Applied(_)),
                _ => false,
            })
            .filter(|&applied| applied)
            .count();
        let running = conversation.is_running();
        let strip = ui.allocate_ui_with_layout(
            vec2(ui.available_width(), SUBHEADER_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                let chip = Frame::new()
                    .fill(p.field)
                    .corner_radius(14)
                    .inner_margin(Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            icon(ui, Icon::CheckCircle, 15.0, p.success);
                            let label =
                                if applied == 1 { "1 edit".to_string() } else { format!("{applied} edits") };
                            ui.label(RichText::new(label).font(theme::semibold(12.5)).color(p.text));
                        });
                    });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if running {
                        ui.label(RichText::new(&conversation.progress).color(p.text_muted));
                        ui.add(Spinner::new().size(14.0).color(p.accent));
                    } else {
                        ui.label(RichText::new("Ready").color(p.text_faint));
                    }
                });
                chip.response.rect
            },
        );
        ui.add_space(6.0);
        let y = ui.cursor().top();
        let painter = ui.painter();
        painter.hline(ui.max_rect().x_range(), y, Stroke::new(1.0, p.border));
        painter.hline(strip.inner.x_range(), y, Stroke::new(2.0, if running { p.accent } else { p.success }));
        ui.add_space(12.0);
    }

    fn empty_state(&mut self, ui: &mut Ui, conversation: &mut Conversation, editor: &Editor) {
        let p = palette(ui);
        let mut picked = None;
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            let (tile, _) = ui.allocate_exact_size(vec2(46.0, 46.0), Sense::hover());
            ui.painter().rect_filled(tile, 14.0, p.accent_soft);
            paint_icon(ui.painter(), tile.shrink(12.0), Icon::Sparkle, p.accent);
            ui.add_space(10.0);
            ui.label(RichText::new("What should we change?").font(theme::semibold(16.0)).color(p.text));
            ui.add_space(2.0);
            ui.label(
                RichText::new(
                    "Ask for an edit in plain words. Each request gets a card here, \
                     so you can see exactly what changed and undo it.",
                )
                .color(p.text_muted),
            );
        });
        ui.add_space(18.0);
        for example in EXAMPLES {
            if suggestion(ui, p, example).clicked() {
                picked = Some(example.to_string());
            }
        }
        ui.add_space(10.0);
        let shortcut = if cfg!(target_os = "macos") { "⌘K" } else { "Ctrl+K" };
        ui.vertical_centered(|ui| {
            let hint = format!("Press {shortcut} anywhere to ask without the panel.");
            ui.label(RichText::new(hint).small().color(p.text_faint));
        });
        if let Some(example) = picked {
            self.submit(conversation, example, editor, ui.ctx());
        }
    }

    fn cards(&mut self, ui: &mut Ui, conversation: &mut Conversation, editor: &Editor) -> Option<Action> {
        if std::mem::take(&mut conversation.just_finished) {
            conversation.fresh_change = Some(editor.revision());
        }
        let mut action = None;
        let mut hovered = None;
        for index in 0..conversation.entries.len() {
            let (card_action, card_rect) = self.card(ui, conversation, index, editor);
            action = action.or(card_action);
            if ui.rect_contains_pointer(card_rect) {
                hovered = Some(index);
            }
            ui.add_space(10.0);
        }

        // Tint on the page: the hovered card's changes, else an open card's, else the newest change until the next edit.
        let entries = &mut conversation.entries;
        let fresh = conversation.fresh_change == Some(editor.revision());
        let spotlight = hovered.or_else(|| entries.iter().rposition(|e| e.details_open)).or_else(|| {
            fresh.then(|| entries.iter().rposition(|e| matches!(e.outcome, Outcome::Changed(_))))?
        });
        conversation.highlights = match spotlight.map(|i| &mut entries[i].outcome) {
            Some(Outcome::Changed(change)) => match change.state(editor) {
                ChangeState::Applied(ranges) => ranges.clone(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        action
    }

    /// One request's card. Returns what the user asked for, and where the card is.
    fn card(
        &mut self,
        ui: &mut Ui,
        conversation: &mut Conversation,
        index: usize,
        editor: &Editor,
    ) -> (Option<Action>, Rect) {
        let p = palette(ui);
        let running = conversation.is_running() && index + 1 == conversation.entries.len();
        let progress = conversation.progress.clone();
        let entry = &mut conversation.entries[index];
        let mut action = None;
        let mut retry = None;
        let mut cancel = false;
        let stroke = Stroke::new(1.0, if entry.details_open { p.accent } else { p.border });
        let frame =
            Frame::new().fill(p.surface).stroke(stroke).corner_radius(12).inner_margin(Margin::same(14));
        let response = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(&entry.request).font(theme::semibold(12.5)).color(p.text_muted));
            ui.add_space(4.0);
            let reply = if running { "Working on it…".to_string() } else { entry.reply.clone() };
            let color = if matches!(entry.outcome, Outcome::Failed) { p.danger } else { p.text };
            ui.label(RichText::new(reply).size(14.0).color(color));
            ui.add_space(10.0);
            let y = ui.cursor().top();
            ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, p.border));
            ui.add_space(10.0);

            ui.horizontal(|ui| match &mut entry.outcome {
                Outcome::Working => {
                    ui.add(Spinner::new().size(18.0).color(p.accent));
                    ui.label(RichText::new(&progress).color(p.text_muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        cancel = button(ui, ButtonKind::Outline, None, "Cancel").clicked();
                    });
                }
                Outcome::Failed | Outcome::Cancelled => {
                    let failed = matches!(entry.outcome, Outcome::Failed);
                    let (status_icon, color, label) = if failed {
                        (Icon::Warning, p.danger, "Didn't finish")
                    } else {
                        (Icon::Close, p.text_faint, "Cancelled")
                    };
                    icon(ui, status_icon, 18.0, color);
                    ui.label(RichText::new(label).color(p.text_muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if button(ui, ButtonKind::Secondary, None, "Try again").clicked() {
                            retry = Some(entry.request.clone());
                        }
                    });
                }
                Outcome::Answered => {
                    icon(ui, Icon::CheckCircle, 18.0, p.success);
                    ui.label(RichText::new("No changes to the document").color(p.text_muted));
                }
                Outcome::Changed(change) => {
                    let state = change.state(editor).clone();
                    let (added, removed) = change.counts();
                    match &state {
                        ChangeState::Applied(_) => {
                            icon(ui, Icon::CheckCircle, 18.0, p.success);
                            change_counts(ui, p, added, removed);
                        }
                        ChangeState::Undone => {
                            icon(ui, Icon::Undo, 18.0, p.text_muted);
                            ui.label(RichText::new("Undone").color(p.text_muted));
                        }
                        ChangeState::EditedSince => {
                            icon(ui, Icon::Warning, 18.0, p.warning);
                            ui.label(RichText::new("Edited since").color(p.text_muted)).on_hover_text(
                                "These paragraphs were changed again after the assistant edited them, \
                                 so this can't be undone here. Use Undo in the toolbar instead.",
                            );
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let details = if entry.details_open { "Hide details" } else { "See details" };
                        if button(ui, ButtonKind::Secondary, None, details).clicked() {
                            entry.details_open = !entry.details_open;
                            if let (true, ChangeState::Applied(ranges)) = (entry.details_open, &state) {
                                action = ranges.first().map(|r| Action::Reveal(r.start));
                            }
                        }
                        let toggle = match state {
                            ChangeState::Applied(_) => Some((Icon::Undo, "Undo", true)),
                            ChangeState::Undone => Some((Icon::Redo, "Redo", false)),
                            ChangeState::EditedSince => None,
                        };
                        if let Some((toggle_icon, label, undo)) = toggle {
                            let hint =
                                if undo { "Take this change back out" } else { "Put this change back" };
                            let clicked = button(ui, ButtonKind::Outline, Some(toggle_icon), label)
                                .on_hover_text(hint)
                                .clicked();
                            if clicked {
                                action = change.toggled(editor.document(), undo).map(Action::Apply);
                            }
                        }
                    });
                }
            });

            if let (true, Outcome::Changed(change)) = (entry.details_open, &entry.outcome) {
                ui.add_space(12.0);
                diff_view::show(ui, change);
            }
        });
        if cancel {
            conversation.cancel();
        }
        if let Some(request) = retry {
            self.submit(conversation, request, editor, ui.ctx());
        }
        (action, response.response.rect)
    }

    fn composer(&mut self, ui: &mut Ui, conversation: &mut Conversation, editor: &Editor) {
        let p = palette(ui);
        let running = conversation.is_running();
        let border = if self.input_has_focus { p.accent } else { p.border };
        let frame = Frame::new()
            .fill(p.field)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(12)
            .inner_margin(Margin { left: 12, right: 8, top: 10, bottom: 8 });
        frame.show(ui, |ui| {
            let field = TextEdit::multiline(&mut self.input)
                .hint_text("Ask for an edit…")
                .frame(Frame::NONE)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .return_key(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter));
            let response = ui.add_enabled(!running, field);
            self.input_has_focus = response.has_focus();
            let enter = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
            let mut send = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.config.agent.model).small().color(p.text_faint))
                    .on_hover_text("Enter to send · Shift+Enter for a new line");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can_send = !running && !self.input.trim().is_empty();
                    send = send_button(ui, p, can_send).clicked();
                });
            });
            if enter || send {
                let request = std::mem::take(&mut self.input);
                self.submit(conversation, request, editor, ui.ctx());
            }
        });
    }
}

/// "+3 −1": paragraphs added or rewritten, and removed.
fn change_counts(ui: &mut Ui, p: &Palette, added: usize, removed: usize) {
    let mut job = LayoutJob::default();
    let format = |color: Color32| TextFormat { font_id: theme::semibold(13.0), color, ..Default::default() };
    job.append(&format!("+{added}"), 0.0, format(p.success));
    job.append(&format!("−{removed}"), 6.0, format(p.danger));
    ui.label(job).on_hover_text(format!("{added} paragraphs added or rewritten, {removed} removed"));
}

/// A round send button with an arrow.
fn send_button(ui: &mut Ui, p: &Palette, enabled: bool) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), sense);
    let fill = match (enabled, response.hovered()) {
        (false, _) => p.pressed,
        (true, true) => p.accent_hover,
        (true, false) => p.accent,
    };
    ui.painter().circle_filled(rect.center(), 15.0, fill);
    paint_icon(
        ui.painter(),
        rect.shrink(8.0),
        Icon::Send,
        if enabled { Color32::WHITE } else { p.text_faint },
    );
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Send")
    } else {
        response
    }
}

/// An example request, shown as an outlined row.
fn suggestion(ui: &mut Ui, p: &Palette, text: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let fill = if response.hovered() { p.hover } else { p.surface };
    ui.painter().rect(rect, 10.0, fill, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let icon_rect = Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), vec2(14.0, 14.0));
    paint_icon(ui.painter(), icon_rect, Icon::Sparkle, p.accent);
    let font = egui::FontId::proportional(13.5);
    ui.painter().text(
        egui::pos2(rect.left() + 36.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        p.text,
    );
    response.on_hover_cursor(CursorIcon::PointingHand)
}
