//! "See details": a change shown as removed (red) and added (green)
//! paragraphs, with their formatting, like a code review diff.

use eframe::egui::text::LayoutJob;
use eframe::egui::{FontId, Frame, Label, Margin, RichText, Stroke, TextFormat, Ui};
use vibedoc_core::{is_image_char, Document, Paragraph, ParagraphKind};

use super::Change;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, palette, Palette};
use crate::ui::widgets::icon;

/// Very long paragraphs are cut short here; the page shows them in full.
const MAX_CHARS: usize = 600;

#[derive(Clone, Copy)]
enum Side {
    Removed,
    Added,
}

pub fn show(ui: &mut Ui, change: &Change) {
    let p = palette(ui);
    Frame::new().fill(p.field).stroke(Stroke::new(1.0, p.border)).corner_radius(10).show(ui, |ui| {
        ui.set_width(ui.available_width());
        Frame::new().inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, Icon::Document, 14.0, p.text_muted);
                ui.label(RichText::new("Changes").font(theme::semibold(12.5)).color(p.text));
            });
        });
        for (index, hunk) in change.hunks.iter().enumerate() {
            if index > 0 {
                Frame::new().inner_margin(Margin::symmetric(10, 2)).show(ui, |ui| {
                    ui.label(RichText::new("· · ·").color(p.text_faint));
                });
            }
            for paragraph in &change.before.paragraphs[hunk.before.clone()] {
                line(ui, p, &change.before, paragraph, Side::Removed);
            }
            for paragraph in &change.after.paragraphs[hunk.after.clone()] {
                line(ui, p, &change.after, paragraph, Side::Added);
            }
        }
        ui.add_space(6.0);
    });
}

fn line(ui: &mut Ui, p: &Palette, document: &Document, paragraph: &Paragraph, side: Side) {
    let (fill, accent, sign) = match side {
        Side::Removed => (p.danger_soft, p.danger, "−"),
        Side::Added => (p.success_soft, p.success, "+"),
    };
    Frame::new().fill(fill).inner_margin(Margin::symmetric(10, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.add_sized(
                [10.0, 18.0],
                Label::new(RichText::new(sign).font(theme::semibold(13.0)).color(accent)),
            );
            ui.add(Label::new(paragraph_job(p, document, paragraph)).wrap());
        });
    });
}

/// The paragraph's text with its bold, italic, and underline, plus a marker for its kind.
fn paragraph_job(p: &Palette, document: &Document, paragraph: &Paragraph) -> LayoutJob {
    let mut job = LayoutJob::default();
    let size = 13.0;
    let muted = TextFormat::simple(theme::semibold(11.0), p.text_muted);
    let plain = TextFormat::simple(FontId::proportional(size), p.text);
    let tag = match paragraph.kind {
        ParagraphKind::Normal => None,
        ParagraphKind::Heading1 => Some(("H1  ", &muted)),
        ParagraphKind::Heading2 => Some(("H2  ", &muted)),
        ParagraphKind::Heading3 => Some(("H3  ", &muted)),
        ParagraphKind::Quote => Some(("Quote  ", &muted)),
        ParagraphKind::Code => Some(("Code  ", &muted)),
        ParagraphKind::Bullet => Some(("•  ", &plain)),
        ParagraphKind::Numbered => Some(("1.  ", &plain)),
        ParagraphKind::Checklist if paragraph.checked => Some(("☑  ", &plain)),
        ParagraphKind::Checklist => Some(("☐  ", &plain)),
    };
    if let Some((text, format)) = tag {
        job.append(text, 0.0, format.clone());
    }
    if paragraph.is_empty() {
        let format =
            TextFormat { italics: true, ..TextFormat::simple(FontId::proportional(size), p.text_faint) };
        job.append("Empty line", 0.0, format);
        return job;
    }

    let mut remaining = MAX_CHARS;
    for (style, text) in paragraph.runs() {
        if remaining == 0 {
            break;
        }
        let bold = style.bold || paragraph.kind.is_heading();
        let font = if bold { theme::semibold(size) } else { FontId::proportional(size) };
        let format = TextFormat {
            font_id: font,
            color: p.text,
            italics: style.italic,
            underline: if style.underline { Stroke::new(1.0, p.text) } else { Stroke::NONE },
            strikethrough: if style.strikethrough { Stroke::new(1.0, p.text) } else { Stroke::NONE },
            ..Default::default()
        };
        // Pictures are single placeholder characters; name them instead.
        let mut plain = String::new();
        for ch in text.chars().take(remaining) {
            remaining -= 1;
            if is_image_char(ch) && document.image(ch).is_some() {
                job.append(&std::mem::take(&mut plain), 0.0, format.clone());
                let picture =
                    TextFormat { italics: true, ..TextFormat::simple(theme::semibold(12.0), p.accent) };
                job.append("[picture]", 0.0, picture);
            } else {
                plain.push(ch);
            }
        }
        job.append(&plain, 0.0, format);
    }
    if remaining == 0 && paragraph.len() > MAX_CHARS {
        job.append(" …", 0.0, TextFormat::simple(FontId::proportional(size), p.text_muted));
    }
    job
}
