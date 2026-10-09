//! VibeDoc's core: the document model, editing, find, and page layout.
//! No GUI code lives here, so everything is unit-testable.

pub mod changes;
pub mod document;
pub mod editor;
pub mod find;
pub mod layout;

pub use document::{
    is_image_char, Alignment, Color, Document, Font, Image, LineSpacing, Paragraph, ParagraphKind, Position,
    Script, Selection, StyleFlag, StyledChar, TextStyle,
};
pub use editor::Editor;
pub use layout::{Layout, Measure, PageSetup};
