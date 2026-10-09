//! The Ctrl+K command bar: ask the assistant from anywhere, without the panel.

use eframe::egui::{self, Frame, Key, Margin, RichText, TextEdit};
use vibedoc_core::Editor;

use super::Conversation;

use super::Assistant;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, palette_for};
use crate::ui::widgets::icon;

impl Assistant {
    pub fn open_command_bar(&mut self) {
        if self.command_bar.is_none() {
            self.command_bar = Some(String::new());
        }
    }

    pub fn command_bar(&mut self, ctx: &egui::Context, conversation: &mut Conversation, editor: &Editor) {
        let Some(text) = self.command_bar.as_mut() else { return };
        let p = palette_for(ctx.global_style().visuals.dark_mode);
        let mut submit = false;
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let frame = Frame::window(&ctx.global_style()).inner_margin(Margin::symmetric(16, 14));
        egui::Window::new("Ask the assistant")
            .title_bar(false)
            .resizable(false)
            .frame(frame)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 90.0])
            .fixed_size([560.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (tile, _) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                    ui.painter().rect_filled(tile, 8.0, p.accent_soft);
                    crate::ui::icons::paint_icon(ui.painter(), tile.shrink(7.0), Icon::Sparkle, p.accent);
                    let field = TextEdit::singleline(text)
                        .hint_text("What should I change? e.g. “make this a numbered list”")
                        .font(egui::FontId::proportional(16.0))
                        .frame(Frame::NONE)
                        .desired_width(f32::INFINITY);
                    let response = ui.add(field);
                    submit = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    if !submit {
                        response.request_focus();
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    icon(ui, Icon::Send, 12.0, p.text_muted);
                    ui.label(RichText::new("Enter to send").small().color(p.text_muted));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Esc to close").small().color(p.text_muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let model = RichText::new(&self.config.agent.model)
                            .font(theme::semibold(11.0))
                            .color(p.text_muted);
                        ui.label(model);
                    });
                });
            });
        if submit {
            let request = std::mem::take(text);
            self.submit(conversation, request, editor, ctx);
            close = true;
        }
        if close {
            self.command_bar = None;
        }
    }
}
