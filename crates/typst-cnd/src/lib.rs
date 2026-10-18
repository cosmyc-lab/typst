//! Typst's CND exporter.

mod document;
mod emit;
mod location;
mod metadata;
mod model;
mod rich_text;
pub mod world;

pub use self::document::{CndDocument, cnd_document, cnd_from_document, cnd_to_json};
pub use self::model::{
    AdmonitionNode, BibEntry, CND_VERSION, Cell, CiteRef, Cnd, CndNode, CodeNode,
    DocMetadata, FigureNode, Footnote, FootnoteRef, HeadingNode, ImageNode, LayoutNode,
    ListItem, ListNode, MathNode, NodeRef, ParagraphNode, QuoteNode, RawSource,
    SourceInfo, TableNode, TermItem, TermsNode, TextSlot, walk, walk_mut,
};
pub use self::rich_text::{
    CODE, EMPH, HIGHLIGHT, LinkRef, MATH, Mark, Numbering, RichText, STRIKE, STRONG, SUB,
    SUP, UNDERLINE, canonical_marks, trim_rebase,
};
