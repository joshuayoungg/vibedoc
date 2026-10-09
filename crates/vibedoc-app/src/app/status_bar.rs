//! The status bar under the pages: page, word count, and zoom.

use eframe::egui::{Align, CursorIcon, Label, Layout, RichText, Sense, Ui};

use super::App;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, palette};
use crate::ui::widgets::icon_button;

const ZOOM_STEP: f32 = 0.1;
const ZOOM_RANGE: (f32, f32) = (0.5, 3.0);

impl App {
    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        ui.horizontal(|ui| {
            let tab = &mut self.tabs[self.active];
            let layout = tab.view.layout(ui.ctx(), &tab.editor);
            let page = layout.caret(tab.editor.selection().cursor).page + 1;
            let words = tab.editor.document().word_count();
            ui.add_space(4.0);
            ui.label(RichText::new(format!("Page {page} of {}", layout.page_count)).color(p.text_muted));
            ui.label(RichText::new("·").color(p.text_faint));
            let unit = if words == 1 { "word" } else { "words" };
            ui.label(RichText::new(format!("{} {unit}", with_thousands(words))).color(p.text_muted));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let zoom = &mut tab.view.zoom;
                if icon_button(ui, Icon::Plus, false).on_hover_text("Zoom in").clicked() {
                    *zoom = (*zoom + ZOOM_STEP).min(ZOOM_RANGE.1);
                }
                let percent =
                    RichText::new(format!("{:.0}%", *zoom * 100.0)).font(theme::semibold(12.5)).color(p.text);
                let reset = ui.add(Label::new(percent).sense(Sense::click()));
                if reset.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Reset to 100%").clicked() {
                    *zoom = 1.0;
                }
                if icon_button(ui, Icon::Minus, false).on_hover_text("Zoom out").clicked() {
                    *zoom = (*zoom - ZOOM_STEP).max(ZOOM_RANGE.0);
                }
            });
        });
    }
}

/// 1234567 -> "1,234,567"
fn with_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn thousands_are_separated() {
        assert_eq!(super::with_thousands(7), "7");
        assert_eq!(super::with_thousands(1098), "1,098");
        assert_eq!(super::with_thousands(1234567), "1,234,567");
    }
}
