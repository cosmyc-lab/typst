//! Serde types matching the cnd-engine manifest contract.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CND_VERSION: &str = "0.2.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CndManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub cnd_version: String,
    pub doc_hash: String,
    pub compiled_at: String,
    pub doc: DocMetadata,
    pub nodes: Vec<CndNode>,
    /// Bibliography pool — target of `cites` edges. Always present (never
    /// null); empty when the document cites nothing (proposal 0004).
    #[serde(default)]
    pub bibliography: Vec<BibEntry>,
    /// Footnote pool — target of `footnotes` edges. Always present.
    #[serde(default)]
    pub footnotes: Vec<Footnote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DocDate {
    pub year: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DocMetadata {
    pub title: String,
    pub authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<DocDate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeLocation {
    /// The page on which the node begins in the compiled document — the
    /// one layout fact a consumer cannot derive from the tree. Reading
    /// order, per-page order, and within-parent order are all derived by
    /// consumers from the normative reading order of `nodes` (spec §2).
    pub page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CndNode {
    Heading(HeadingNode),
    Paragraph(ParagraphNode),
    Table(TableNode),
    Quote(QuoteNode),
    Code(CodeNode),
    Math(MathNode),
    Figure(FigureNode),
    Image(ImageNode),
    List(ListNode),
    Terms(TermsNode),
}

impl CndNode {
    pub fn id(&self) -> Uuid {
        self.base().id
    }

    pub fn base(&self) -> &NodeBase {
        match self {
            Self::Heading(n) => &n.base,
            Self::Paragraph(n) => &n.base,
            Self::Table(n) => &n.base,
            Self::Quote(n) => &n.base,
            Self::Code(n) => &n.base,
            Self::Math(n) => &n.base,
            Self::Figure(n) => &n.base,
            Self::Image(n) => &n.base,
            Self::List(n) => &n.base,
            Self::Terms(n) => &n.base,
        }
    }

    pub fn base_mut(&mut self) -> &mut NodeBase {
        match self {
            Self::Heading(n) => &mut n.base,
            Self::Paragraph(n) => &mut n.base,
            Self::Table(n) => &mut n.base,
            Self::Quote(n) => &mut n.base,
            Self::Code(n) => &mut n.base,
            Self::Math(n) => &mut n.base,
            Self::Figure(n) => &mut n.base,
            Self::Image(n) => &mut n.base,
            Self::List(n) => &mut n.base,
            Self::Terms(n) => &mut n.base,
        }
    }

    pub fn location_mut(&mut self) -> &mut NodeLocation {
        &mut self.base_mut().location
    }

    pub fn refs_to_mut(&mut self) -> &mut Vec<NodeRef> {
        &mut self.base_mut().refs_to
    }

    pub fn refs_from_mut(&mut self) -> &mut Vec<NodeRef> {
        &mut self.base_mut().refs_from
    }

    pub fn cites_mut(&mut self) -> &mut Vec<CiteRef> {
        &mut self.base_mut().cites
    }

    pub fn footnotes_mut(&mut self) -> &mut Vec<FootnoteRef> {
        &mut self.base_mut().footnotes
    }

    /// Nodes with their own reading-flow children: headings and figure
    /// wrappers. A figure's children keep their own `location` — nothing is
    /// inherited from the wrapper (ADR 0010) — but they still need to be
    /// reachable for metadata application and location assignment.
    pub fn children_mut(&mut self) -> Option<&mut Vec<CndNode>> {
        match self {
            Self::Heading(n) => Some(&mut n.children),
            Self::Figure(n) => Some(&mut n.children),
            _ => None,
        }
    }
}

/// Resolved cross-reference edge: stable node id plus optional Typst label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct NodeRef {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl NodeRef {
    pub fn new(id: Uuid, label: Option<String>) -> Self {
        Self { id, label }
    }
}

/// Forward citation edge; `id` resolves in the manifest `bibliography`
/// pool (proposal 0004). `form` mirrors Typst's citation form; `span` is
/// an optional `[start, end)` codepoint offset into the node's rendered
/// text (currently always `None` — spans are a deferred conformance
/// level, see the pools module).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CiteRef {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplement: Option<String>,
}

/// Forward footnote edge; `id` resolves in the manifest `footnotes` pool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct FootnoteRef {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Vec<i64>>,
}

/// Footnote pool entry — flat supporting text keyed by its rendered
/// ordinal `label` (proposal 0004).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Footnote {
    pub id: Uuid,
    pub label: String,
    pub text: String,
}

/// Bibliography pool entry — target of `cites` edges. `rendered` is the
/// reference string as displayed in the compiled document; a curated
/// typed subset is lifted alongside it, and the full source entry is
/// carried losslessly as structured JSON in `raw` (proposal 0004).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BibEntry {
    pub id: Uuid,
    pub label: String,
    pub rendered: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Full source entry (Hayagriva) as structured JSON — always present.
    #[serde(default = "default_json_object")]
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeBase {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs_to: Vec<NodeRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs_from: Vec<NodeRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<CiteRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footnotes: Vec<FootnoteRef>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub state_metadata: HashMap<String, serde_json::Value>,
    pub location: NodeLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HeadingNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub level: i32,
    pub numbering: String,
    pub text: String,
    pub heading_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ParagraphNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TableCell {
    pub row: i32,
    pub col: i32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub rowspan: i32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub colspan: i32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_header: bool,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    #[default]
    Table,
    Grid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TableNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "is_table_kind")]
    pub kind: TableKind,
    /// "data" | "content" hint for text rendering (cnd-sdk's
    /// `cnd.core.node_text` "inline"/"auto" modes) — set from a
    /// `content_kind:` argument on the Typst-side table wrapper, threaded
    /// through the generic `cnd.metadata` state (see `emit/table.rs`'s
    /// `content_kind_from_metadata`). `None` when the author never tagged it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<TableCell>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_typst: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QuoteNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub block: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CodeNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub block: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MathNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_typst: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbering: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub block: bool,
}

/// Captioned/numbered float wrapper — never a content carrier (ADR 0010).
///
/// The wrapped content (image, table, code, …) lives in `children` and
/// keeps its own node type and `location`. `kind` is the counter/label
/// selector of the figure ("image", "table", "raw", or an author-custom
/// kind like "atom") — an open string, never a content discriminator.
/// An unconvertible body yields `children: []` with `raw_typst` filled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FigureNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fig_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_typst: Option<String>,
}

/// Leaf image content. A bare image outside any figure is an `ImageNode`
/// with no wrapper; a captioned image is an `ImageNode` inside a
/// `FigureNode`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImageNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListItem {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ordered: bool,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub tight: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ListItem>,
}

/// Single term/description pair of a definition list. Flat text, no id,
/// not ref-targetable — same shape as `ListItem`/`TableCell`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TermItem {
    pub term: String,
    pub description: String,
}

/// Definition list (Typst `/ term: description` items) — proposal 0004.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TermsNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub tight: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<TermItem>,
}

impl TermsNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            tight: true,
            items: Vec::new(),
        }
    }
}

fn default_json_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn default_one() -> i32 {
    1
}

fn is_one(v: &i32) -> bool {
    *v == 1
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_table_kind(kind: &TableKind) -> bool {
    *kind == TableKind::Table
}

impl NodeBase {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            id,
            label: None,
            refs_to: Vec::new(),
            refs_from: Vec::new(),
            cites: Vec::new(),
            footnotes: Vec::new(),
            state_metadata: HashMap::new(),
            location,
        }
    }
}

impl HeadingNode {
    pub fn new(
        id: Uuid,
        level: i32,
        numbering: String,
        text: String,
        heading_path: Vec<String>,
        location: NodeLocation,
    ) -> Self {
        Self {
            base: NodeBase::new(id, location),
            level,
            numbering,
            text,
            heading_path,
            children: Vec::new(),
        }
    }
}

impl ParagraphNode {
    pub fn new(id: Uuid, text: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            text,
            lang: None,
        }
    }
}

impl TableNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            kind: TableKind::Table,
            content_kind: None,
            cells: Vec::new(),
            raw_typst: None,
        }
    }
}

impl QuoteNode {
    pub fn new(id: Uuid, text: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            text,
            attribution: None,
            block: true,
            lang: None,
        }
    }
}

impl CodeNode {
    pub fn new(id: Uuid, text: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            text,
            lang: None,
            block: true,
        }
    }
}

impl MathNode {
    pub fn new(id: Uuid, text: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            text,
            raw_typst: None,
            numbering: None,
            block: true,
        }
    }
}

impl FigureNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            caption: None,
            fig_number: None,
            kind: None,
            children: Vec::new(),
            raw_typst: None,
        }
    }
}

impl ImageNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            path: None,
            alt: None,
        }
    }
}

impl ListNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            ordered: false,
            tight: true,
            items: Vec::new(),
        }
    }
}
