//! Typst's CND exporter.

mod cnd;
mod document;
mod emit;
mod location;
mod metadata;
mod model;
mod rich_text;
pub mod world;

pub use self::document::{CndDocument, cnd_document, cnd_from_document, cnd_to_json};
pub use self::model::{
    BibEntry, CND_VERSION, CiteRef, Cnd, CndNode, DocMetadata, FigureNode, Footnote,
    FootnoteRef, ImageNode, ListItem, ListNode, NodeRef, RawSource, SourceInfo,
    TableKind, TableNode, TermItem, TermsNode,
};
pub use self::rich_text::{
    CODE, EMPH, HIGHLIGHT, LinkRef, MATH, Mark, Numbering, RichText, STRIKE, STRONG, SUB,
    SUP, UNDERLINE, canonical_marks, trim_rebase,
};
