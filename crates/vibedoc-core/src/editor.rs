//! The editor: a document plus a selection, with undo/redo.
//!
//! Every edit goes through here so undo history and the caret stay correct.
//! Undo stores whole-document snapshots: simple, and fast enough for
//! documents people actually write by hand.

use crate::document::{Document, Image, Paragraph, ParagraphKind, Position, Selection, StyleFlag, TextStyle};

/// Word's font size ladder, used by "grow" and "shrink" and the size menu.
pub const FONT_SIZES: [f32; 16] =
    [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 48.0, 72.0];

#[derive(Clone)]
struct Snapshot {
    document: Document,
    selection: Selection,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Typing,
    Other,
}

pub struct Editor {
    document: Document,
    selection: Selection,
    /// Style for the next typed character (Ctrl+B with nothing selected changes this).
    typing_style: TextStyle,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_edit: Option<EditKind>,
    /// Goes up on every document change, so callers can cache work (like layout).
    revision: u64,
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new(Document::default())
    }
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Editor {
            document,
            selection: Selection::default(),
            typing_style: TextStyle::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
            revision: 0,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    // ---- Selection and caret movement ----

    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = Selection {
            anchor: self.document.clamp(selection.anchor),
            cursor: self.document.clamp(selection.cursor),
        };
        self.typing_style = self.document.style_at(self.selection.cursor);
        self.last_edit = None;
    }

    /// Moves the caret. With `extend`, the selection grows instead (Shift+arrow).
    pub fn move_caret(&mut self, to: Position, extend: bool) {
        let anchor = if extend { self.selection.anchor } else { to };
        self.set_selection(Selection { anchor, cursor: to });
    }

    pub fn select_all(&mut self) {
        self.set_selection(Selection { anchor: self.document.start(), cursor: self.document.end() });
    }

    pub fn move_left(&mut self, extend: bool) {
        if !extend && !self.selection.is_empty() {
            return self.move_caret(self.selection.start(), false);
        }
        let to = self.position_before(self.selection.cursor);
        self.move_caret(to, extend);
    }

    pub fn move_right(&mut self, extend: bool) {
        if !extend && !self.selection.is_empty() {
            return self.move_caret(self.selection.end(), false);
        }
        let to = self.position_after(self.selection.cursor);
        self.move_caret(to, extend);
    }

    pub fn select_word_at(&mut self, position: Position) {
        let (start, end) = self.document.word_at(self.document.clamp(position));
        self.set_selection(Selection { anchor: start, cursor: end });
    }

    pub fn selected_text(&self) -> String {
        self.document.text_in(self.selection.start(), self.selection.end())
    }

    // ---- Editing ----

    /// Typing replaces the selection. Consecutive keystrokes undo together.
    pub fn type_text(&mut self, text: &str) {
        self.record(EditKind::Typing);
        self.delete_selection();
        let end = self.document.insert_text(self.selection.cursor, text, self.typing_style);
        self.place_caret(end);
    }

    pub fn paste(&mut self, text: &str) {
        self.record(EditKind::Other);
        self.delete_selection();
        let end = self.document.insert_text(self.selection.cursor, text, self.typing_style);
        self.place_caret(end);
    }

    pub fn delete_selected(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.record(EditKind::Other);
        self.delete_selection();
    }

    /// Enter. On an empty list item it leaves the list instead, like Word.
    pub fn new_paragraph(&mut self) {
        self.record(EditKind::Other);
        self.delete_selection();
        let caret = self.selection.cursor;
        let paragraph = &mut self.document.paragraphs[caret.paragraph];
        if paragraph.kind.is_list() && paragraph.is_empty() {
            paragraph.kind = ParagraphKind::Normal;
            return self.mark_changed();
        }
        let start_of_new = self.document.split_paragraph(caret);
        self.place_caret(start_of_new);
    }

    /// Backspace. At the start of a list item it removes the bullet first, like Word.
    pub fn delete_backward(&mut self) {
        if !self.selection.is_empty() {
            return self.delete_selected();
        }
        let caret = self.selection.cursor;
        if caret == self.document.start() && !self.document.paragraphs[0].kind.is_list() {
            return;
        }
        self.record(EditKind::Other);
        let paragraph = &mut self.document.paragraphs[caret.paragraph];
        if caret.offset == 0 && paragraph.kind.is_list() {
            paragraph.kind = ParagraphKind::Normal;
            return self.mark_changed();
        }
        let before = self.position_before(caret);
        self.document.delete(before, caret);
        self.place_caret(before);
    }

    pub fn delete_forward(&mut self) {
        if !self.selection.is_empty() {
            return self.delete_selected();
        }
        let caret = self.selection.cursor;
        if caret == self.document.end() {
            return;
        }
        self.record(EditKind::Other);
        let after = self.position_after(caret);
        self.document.delete(caret, after);
        self.place_caret(caret);
    }

    /// Ctrl+B / Ctrl+I / Ctrl+U. With no selection, it changes what you type next.
    pub fn toggle_style(&mut self, flag: StyleFlag) {
        let turn_on = !self.current_style().get(flag);
        self.apply_style(|style| style.set(flag, turn_on));
    }

    /// Changes the style of the selection (font, size, color, ...). With no
    /// selection, it changes what you type next.
    pub fn apply_style(&mut self, change: impl Fn(&mut TextStyle)) {
        change(&mut self.typing_style);
        if self.selection.is_empty() {
            return;
        }
        self.record(EditKind::Other);
        let (start, end) = (self.selection.start(), self.selection.end());
        self.document.restyle(start, end, change);
        self.mark_changed();
    }

    /// Removes all character formatting from the selection.
    pub fn clear_formatting(&mut self) {
        self.apply_style(|style| *style = TextStyle::default());
    }

    /// Makes text bigger or smaller by `steps` sizes on Word's size ladder.
    pub fn step_font_size(&mut self, steps: i32) {
        let kind = self.current_paragraph_kind();
        let current = self.current_style().points(kind);
        let index = FONT_SIZES.iter().position(|&size| size >= current).unwrap_or(FONT_SIZES.len() - 1);
        let target = (index as i32 + steps).clamp(0, FONT_SIZES.len() as i32 - 1) as usize;
        let half_points = (FONT_SIZES[target] * 2.0) as u16;
        self.apply_style(|style| style.size = Some(half_points));
    }

    /// The style shown as "active" in the toolbar.
    pub fn current_style(&self) -> TextStyle {
        if self.selection.is_empty() {
            return self.typing_style;
        }
        let (start, end) = (self.selection.start(), self.selection.end());
        let all = |flag| self.document.all_chars_in(start, end, |s| s.get(flag));
        // Font, size, and color show what the selection starts with.
        let first = self.document.chars_in(start, end).next().map_or(self.typing_style, |c| c.style);
        TextStyle {
            bold: all(StyleFlag::Bold),
            italic: all(StyleFlag::Italic),
            underline: all(StyleFlag::Underline),
            strikethrough: all(StyleFlag::Strikethrough),
            ..first
        }
    }

    /// Sets the kind (heading, list, ...) of every paragraph touched by the selection.
    pub fn set_paragraph_kind(&mut self, kind: ParagraphKind) {
        self.format_paragraphs(|paragraph| paragraph.kind = kind);
    }

    /// Changes every paragraph touched by the selection, e.g. its alignment.
    pub fn format_paragraphs(&mut self, change: impl Fn(&mut Paragraph)) {
        self.record(EditKind::Other);
        for index in self.selection.start().paragraph..=self.selection.end().paragraph {
            change(&mut self.document.paragraphs[index]);
        }
        self.mark_changed();
    }

    /// Ticks or unticks a checklist item.
    pub fn toggle_checked(&mut self, paragraph: usize) {
        if self.document.paragraphs.get(paragraph).is_none_or(|p| p.kind != ParagraphKind::Checklist) {
            return;
        }
        self.record(EditKind::Other);
        let paragraph = &mut self.document.paragraphs[paragraph];
        paragraph.checked = !paragraph.checked;
        self.mark_changed();
    }

    /// Inserts a picture at the caret, replacing any selection.
    pub fn insert_image(&mut self, image: Image) {
        self.record(EditKind::Other);
        self.delete_selection();
        let after = self.document.insert_image(self.selection.cursor, image);
        self.place_caret(after);
    }

    /// Swaps in an edited copy of the document (the AI's work) as one undo step.
    pub fn replace_document(&mut self, document: Document) {
        self.record(EditKind::Other);
        self.document = document;
        self.set_selection(self.selection);
        self.mark_changed();
    }

    pub fn current_paragraph_kind(&self) -> ParagraphKind {
        self.current_paragraph().kind
    }

    /// The paragraph with the caret, for showing its formatting in the toolbar.
    pub fn current_paragraph(&self) -> &Paragraph {
        &self.document.paragraphs[self.selection.cursor.paragraph]
    }

    // ---- Undo / redo ----

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore(snapshot);
        }
    }

    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore(snapshot);
        }
    }

    // ---- Helpers ----

    fn record(&mut self, kind: EditKind) {
        let continues_typing = kind == EditKind::Typing && self.last_edit == Some(EditKind::Typing);
        if !continues_typing {
            self.undo_stack.push(self.snapshot());
            self.redo_stack.clear();
        }
        self.last_edit = Some(kind);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { document: self.document.clone(), selection: self.selection }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.document = snapshot.document;
        self.set_selection(snapshot.selection);
        self.mark_changed();
    }

    fn delete_selection(&mut self) {
        let (start, end) = (self.selection.start(), self.selection.end());
        self.document.delete(start, end);
        self.selection = Selection::caret(start);
        self.mark_changed();
    }

    /// Moves the caret after an edit without breaking up undo grouping.
    fn place_caret(&mut self, position: Position) {
        self.selection = Selection::caret(position);
        self.mark_changed();
    }

    fn mark_changed(&mut self) {
        self.revision += 1;
    }

    fn position_before(&self, position: Position) -> Position {
        if position.offset > 0 {
            Position::new(position.paragraph, position.offset - 1)
        } else if position.paragraph > 0 {
            let previous = position.paragraph - 1;
            Position::new(previous, self.document.paragraph_len(previous))
        } else {
            position
        }
    }

    fn position_after(&self, position: Position) -> Position {
        if position.offset < self.document.paragraph_len(position.paragraph) {
            Position::new(position.paragraph, position.offset + 1)
        } else if position.paragraph + 1 < self.document.paragraphs.len() {
            Position::new(position.paragraph + 1, 0)
        } else {
            position
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(editor: &Editor) -> Vec<String> {
        editor.document().paragraphs.iter().map(|p| p.text()).collect()
    }

    #[test]
    fn typing_then_undo_removes_the_whole_burst() {
        let mut e = Editor::default();
        e.type_text("h");
        e.type_text("i");
        e.undo();
        assert_eq!(text(&e), [""]);
        e.redo();
        assert_eq!(text(&e), ["hi"]);
    }

    #[test]
    fn moving_the_caret_starts_a_new_undo_step() {
        let mut e = Editor::default();
        e.type_text("ab");
        e.move_left(false);
        e.type_text("X");
        e.undo();
        assert_eq!(text(&e), ["ab"]);
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut e = Editor::new(Document::default());
        e.type_text("hello world");
        e.set_selection(Selection { anchor: Position::new(0, 0), cursor: Position::new(0, 5) });
        e.type_text("bye");
        assert_eq!(text(&e), ["bye world"]);
    }

    #[test]
    fn backspace_at_paragraph_start_merges() {
        let mut e = Editor::default();
        e.type_text("ab");
        e.new_paragraph();
        e.type_text("cd");
        e.move_caret(Position::new(1, 0), false);
        e.delete_backward();
        assert_eq!(text(&e), ["abcd"]);
        assert_eq!(e.selection().cursor, Position::new(0, 2));
    }

    #[test]
    fn enter_on_empty_list_item_leaves_the_list() {
        let mut e = Editor::default();
        e.set_paragraph_kind(ParagraphKind::Bullet);
        e.type_text("item");
        e.new_paragraph();
        assert_eq!(e.current_paragraph_kind(), ParagraphKind::Bullet);
        e.new_paragraph();
        assert_eq!(e.current_paragraph_kind(), ParagraphKind::Normal);
        assert_eq!(e.document().paragraphs.len(), 2);
    }

    #[test]
    fn toggle_bold_on_selection_and_back() {
        let mut e = Editor::default();
        e.type_text("hello");
        e.select_all();
        e.toggle_style(StyleFlag::Bold);
        assert!(e.current_style().bold);
        e.toggle_style(StyleFlag::Bold);
        assert!(!e.current_style().bold);
    }

    #[test]
    fn toggle_bold_without_selection_affects_next_typing() {
        let mut e = Editor::default();
        e.type_text("a");
        e.toggle_style(StyleFlag::Bold);
        e.type_text("b");
        let chars = &e.document().paragraphs[0].chars;
        assert!(!chars[0].style.bold);
        assert!(chars[1].style.bold);
    }

    #[test]
    fn replacing_the_document_is_one_undo_step() {
        let mut e = Editor::default();
        e.type_text("before");
        let mut edited = e.document().clone();
        edited.insert_text(Position::new(0, 0), "x\ny\n", TextStyle::default());
        e.replace_document(edited);
        assert_eq!(text(&e), ["x", "y", "before"]);
        e.undo();
        assert_eq!(text(&e), ["before"]);
    }

    #[test]
    fn styling_a_selection_keeps_other_formatting() {
        let mut e = Editor::default();
        e.type_text("hello");
        e.select_all();
        e.toggle_style(StyleFlag::Bold);
        e.apply_style(|s| s.font = crate::document::Font::Lora);
        let style = e.document().paragraphs[0].chars[0].style;
        assert!(style.bold);
        assert_eq!(style.font, crate::document::Font::Lora);
        e.clear_formatting();
        assert_eq!(e.document().paragraphs[0].chars[0].style, TextStyle::default());
    }

    #[test]
    fn font_size_steps_along_the_ladder() {
        let mut e = Editor::default();
        e.type_text("x");
        e.select_all();
        e.step_font_size(1);
        assert_eq!(e.current_style().size, Some(28));
        e.step_font_size(-2);
        assert_eq!(e.current_style().size, Some(22));
    }

    #[test]
    fn checklist_items_tick_and_continue_unticked() {
        let mut e = Editor::default();
        e.set_paragraph_kind(ParagraphKind::Checklist);
        e.type_text("buy milk");
        e.toggle_checked(0);
        assert!(e.document().paragraphs[0].checked);
        e.new_paragraph();
        assert_eq!(e.current_paragraph_kind(), ParagraphKind::Checklist);
        assert!(!e.current_paragraph().checked);
    }

    #[test]
    fn alignment_carries_over_to_the_next_paragraph() {
        let mut e = Editor::default();
        e.format_paragraphs(|p| p.alignment = crate::document::Alignment::Center);
        e.type_text("title");
        e.new_paragraph();
        assert_eq!(e.current_paragraph().alignment, crate::document::Alignment::Center);
    }

    #[test]
    fn revision_changes_on_edits_only() {
        let mut e = Editor::default();
        let start = e.revision();
        e.move_right(false);
        assert_eq!(e.revision(), start);
        e.type_text("x");
        assert!(e.revision() > start);
    }
}
