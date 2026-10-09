//! Inkwell's core: the document model, editing, find, and page layout.
//! No GUI code lives here, so everything is unit-testable.

pub mod document;
pub mod editor;
pub mod find;
pub mod layout;

pub use document::{
    Document, Paragraph, ParagraphKind, Position, Selection, StyleFlag, StyledChar, TextStyle,
};
pub use editor::Editor;
pub use layout::{Layout, Measure, PageSetup};
