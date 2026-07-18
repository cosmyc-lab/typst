//! Typst's CND manifest exporter.

mod cnd;
mod document;
mod emit;
mod location;
mod manifest;
mod metadata;
pub mod world;

pub use self::document::{CndDocument, cnd_document, manifest_from_document, manifest_to_json};
pub use self::manifest::{
    BibEntry, CND_VERSION, CiteRef, CndManifest, CndNode, DocMetadata, FigureNode, Footnote,
    FootnoteRef, ImageNode, ListNode, NodeRef, TableKind, TableNode, TermItem, TermsNode,
};
