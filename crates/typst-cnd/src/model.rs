//! Serde types matching the CND format contract (cnd-sdk).
//!
//! These duplicate the reference models rather than binding them: nothing
//! is shared as code across languages (ADR 0019). Agreement is proved by
//! emitting a CND and running it through the SDK's `validate()`, not by
//! construction.
//!
//! Every default-valued field is omitted on output, as the reference writer
//! (`Cnd.to_json`) does: `[]`, `{}` and `null`, `false` for `ordered` and
//! `is_header`, `1` for `rowspan` and `colspan` (spec §6, *Defaults*).
//! Required fields are always written, even when empty (`heading_path`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::rich_text::{
    CiteRef, FootnoteRef, LinkRef, NodeRef, Numbering, RawSource, RichText,
};

/// The format version this producer emits. 0.5.0 moves the link families
/// from nodes onto texts and gives every container block-node children
/// (cnd-sdk ADR 0027); there is no 0.4 output.
pub const CND_VERSION: &str = "0.5.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Cnd {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub cnd_version: String,
    pub built_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceInfo>,
    pub doc: DocMetadata,
    pub nodes: Vec<CndNode>,
    /// Bibliography pool — target of `cites` edges (proposal 0004).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bibliography: Vec<BibEntry>,
    /// Footnote pool — target of `footnotes` edges.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footnotes: Vec<Footnote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DocDate {
    pub year: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<i32>,
}

/// Bibliographic metadata for the work. `status` and `version` are what the
/// author declares on the document; `extra` holds any other title-page
/// metadata, its keys never repeating a field name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DocMetadata {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<DocDate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

/// The input artifact this CND was built from — deliberately separate from
/// `doc`, which identifies the work (spec §2.1).
///
/// `hash` is comparable only between CNDs from *this* producer over the
/// same source. `uri` is a producer-local identifier and is never promised
/// resolvable, so it must stay relative — an absolute workstation path
/// would leak the filesystem tree to every downstream reader.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceInfo {
    #[serde(rename = "type")]
    pub type_: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
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

// The variants differ in size because the node types do; boxing the larger
// ones would put a `Box` in every pattern over the model for a saving that
// does not matter at document scale.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CndNode {
    Heading(HeadingNode),
    Paragraph(ParagraphNode),
    Quote(QuoteNode),
    Code(CodeNode),
    Math(MathNode),
    Image(ImageNode),
    Figure(FigureNode),
    Admonition(AdmonitionNode),
    List(ListNode),
    Terms(TermsNode),
    Table(TableNode),
    Layout(LayoutNode),
}

/// One text of a node, as `CndNode::text_mut` addresses it. `Term(i)` is
/// the term of the `i`-th item of a term list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextSlot {
    Text,
    Caption,
    Description,
    Attribution,
    Title,
    Term(usize),
}

impl CndNode {
    pub fn id(&self) -> Uuid {
        self.base().id
    }

    pub fn base(&self) -> &NodeBase {
        match self {
            Self::Heading(n) => &n.base,
            Self::Paragraph(n) => &n.base,
            Self::Quote(n) => &n.base,
            Self::Code(n) => &n.base,
            Self::Math(n) => &n.base,
            Self::Image(n) => &n.base,
            Self::Figure(n) => &n.base,
            Self::Admonition(n) => &n.base,
            Self::List(n) => &n.base,
            Self::Terms(n) => &n.base,
            Self::Table(n) => &n.base,
            Self::Layout(n) => &n.base,
        }
    }

    pub fn base_mut(&mut self) -> &mut NodeBase {
        match self {
            Self::Heading(n) => &mut n.base,
            Self::Paragraph(n) => &mut n.base,
            Self::Quote(n) => &mut n.base,
            Self::Code(n) => &mut n.base,
            Self::Math(n) => &mut n.base,
            Self::Image(n) => &mut n.base,
            Self::Figure(n) => &mut n.base,
            Self::Admonition(n) => &mut n.base,
            Self::List(n) => &mut n.base,
            Self::Terms(n) => &mut n.base,
            Self::Table(n) => &mut n.base,
            Self::Layout(n) => &mut n.base,
        }
    }

    pub fn location_mut(&mut self) -> &mut NodeLocation {
        &mut self.base_mut().location
    }

    /// The node's text in `slot`, if the node has that slot and it is set.
    pub fn text_mut(&mut self, slot: TextSlot) -> Option<&mut RichText> {
        match (self, slot) {
            (Self::Heading(n), TextSlot::Text) => Some(&mut n.text),
            (Self::Paragraph(n), TextSlot::Text) => Some(&mut n.text),
            (Self::Quote(n), TextSlot::Attribution) => n.attribution.as_mut(),
            (Self::Figure(n), TextSlot::Caption) => n.caption.as_mut(),
            (Self::Figure(n), TextSlot::Description) => n.description.as_mut(),
            (Self::Admonition(n), TextSlot::Title) => n.title.as_mut(),
            (Self::Terms(n), TextSlot::Term(i)) => {
                n.items.get_mut(i).map(|item| &mut item.term)
            }
            _ => None,
        }
    }

    /// The node's own texts in spec order (cnd-sdk `texts_of`); never a
    /// nested node's.
    pub fn texts(&self) -> Vec<&RichText> {
        match self {
            Self::Heading(n) => vec![&n.text],
            Self::Paragraph(n) => vec![&n.text],
            Self::Quote(n) => n.attribution.iter().collect(),
            Self::Figure(n) => n.caption.iter().chain(n.description.iter()).collect(),
            Self::Admonition(n) => n.title.iter().collect(),
            Self::Terms(n) => n.items.iter().map(|item| &item.term).collect(),
            Self::Code(_)
            | Self::Math(_)
            | Self::Image(_)
            | Self::List(_)
            | Self::Table(_)
            | Self::Layout(_) => Vec::new(),
        }
    }

    /// Every list of nested nodes, in document order: `children`, each
    /// `items[i].children` or each `cells[i].children` (cnd-sdk
    /// `child_slots`).
    pub fn child_lists(&self) -> Vec<&[CndNode]> {
        match self {
            Self::Heading(n) => vec![&n.children],
            Self::Quote(n) => vec![&n.children],
            Self::Figure(n) => vec![&n.children],
            Self::Admonition(n) => vec![&n.children],
            Self::List(n) => {
                n.items.iter().map(|item| item.children.as_slice()).collect()
            }
            Self::Terms(n) => {
                n.items.iter().map(|item| item.children.as_slice()).collect()
            }
            Self::Table(n) => {
                n.cells.iter().map(|cell| cell.children.as_slice()).collect()
            }
            Self::Layout(n) => {
                n.cells.iter().map(|cell| cell.children.as_slice()).collect()
            }
            Self::Paragraph(_) | Self::Code(_) | Self::Math(_) | Self::Image(_) => {
                Vec::new()
            }
        }
    }

    /// Mutable twin of [`CndNode::child_lists`].
    pub fn child_lists_mut(&mut self) -> Vec<&mut Vec<CndNode>> {
        match self {
            Self::Heading(n) => vec![&mut n.children],
            Self::Quote(n) => vec![&mut n.children],
            Self::Figure(n) => vec![&mut n.children],
            Self::Admonition(n) => vec![&mut n.children],
            Self::List(n) => n.items.iter_mut().map(|item| &mut item.children).collect(),
            Self::Terms(n) => n.items.iter_mut().map(|item| &mut item.children).collect(),
            Self::Table(n) => n.cells.iter_mut().map(|cell| &mut cell.children).collect(),
            Self::Layout(n) => {
                n.cells.iter_mut().map(|cell| &mut cell.children).collect()
            }
            Self::Paragraph(_) | Self::Code(_) | Self::Math(_) | Self::Image(_) => {
                Vec::new()
            }
        }
    }
}

/// Visit every node of `nodes` in pre-order (reading order), entering every
/// child slot of every container.
pub fn walk<'a>(nodes: &'a [CndNode], f: &mut impl FnMut(&'a CndNode)) {
    for node in nodes {
        f(node);
        for list in node.child_lists() {
            walk(list, f);
        }
    }
}

/// Mutable twin of [`walk`]: `f` sees a node before its children.
pub fn walk_mut(nodes: &mut [CndNode], f: &mut impl FnMut(&mut CndNode)) {
    for node in nodes {
        f(node);
        for list in node.child_lists_mut() {
            walk_mut(list, f);
        }
    }
}

/// Footnote pool entry — target of `footnotes` edges, keyed by its rendered
/// ordinal `label`. Its content is block nodes: real nodes, kept out of the
/// body walk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Footnote {
    pub id: Uuid,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// Bibliography pool entry — target of `cites` edges. `formatted` is the
/// reference as displayed in the built document; a curated typed subset is
/// lifted alongside it, and the full source entry is carried losslessly as
/// structured JSON in `fields` — named `fields` rather than `raw` because
/// `raw` means `{format, value}` on nodes (spec §5.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BibEntry {
    pub id: Uuid,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatted: Option<RichText>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Full source entry (Hayagriva) as structured JSON.
    #[serde(default = "default_json_object", skip_serializing_if = "is_empty_object")]
    pub fields: serde_json::Value,
}

/// Shared fields for every node. The link families live on the node's
/// texts (`RichText`), not here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeBase {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// A plain `serde_json::Map`, not a `HashMap`: without `serde_json`'s
    /// `preserve_order` feature (never enabled in this workspace — it would
    /// change map ordering for every crate in the build), `Map` is backed
    /// by a `BTreeMap`, so keys always serialize in the same alphabetical
    /// order. A `HashMap` here would serialize its keys in a random,
    /// per-process order, which is exactly what CND's `built_at`/UUID
    /// normalization in `typst-cli`'s parity test cannot paper over: the
    /// same document would produce a textually different CND on every run.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub state_metadata: serde_json::Map<String, serde_json::Value>,
    /// Always set: this producer is paginated, so every node has a page.
    pub location: NodeLocation,
}

/// Section heading; the section's content is in `children`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HeadingNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub level: i32,
    /// The counter as displayed ("2.1.1"); `None` for an unnumbered heading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbering: Option<Numbering>,
    pub text: RichText,
    pub heading_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// A paragraph: the leaf that carries prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ParagraphNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: RichText,
}

/// Block quotation; its body is block nodes in `children`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QuoteNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<RichText>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// Code block, verbatim. `title` is the label an author gives a listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CodeNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Block equation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MathNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbering: Option<Numbering>,
}

/// Leaf image content. A bare image outside any figure is an `ImageNode`
/// with no wrapper; a captioned image is an `ImageNode` inside a
/// `FigureNode`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImageNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// Captioned/numbered float wrapper — never a content carrier (ADR 0010).
///
/// The wrapped content (image, table, code, …) lives in `children` and
/// keeps its own node type and `location`. `kind` is the counter/label
/// selector of the figure ("image", "table", "raw", or an author-custom
/// kind like "atom") — an open string, never a content discriminator.
/// An unconvertible body yields `children: []` with `raw` filled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FigureNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbering: Option<Numbering>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawSource>,
}

/// A callout ("Note", "Warning"); `kind` is an open vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AdmonitionNode {
    #[serde(flatten)]
    pub base: NodeBase,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<RichText>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// One list entry. Its content is block nodes; a nested list is a `list`
/// node with its own `ordered`. `checked` is `None` for an item that is
/// not a task-list checkbox, and is never inferred.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// Bullet (`ordered: false`) or numbered (`ordered: true`) list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ListNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ordered: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ListItem>,
}

/// One term and its definition (block nodes in `children`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TermItem {
    pub term: RichText,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// Definition list (Typst `/ term: description` items).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TermsNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<TermItem>,
}

/// One cell of a `table` or a `layout`; its content is block nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Cell {
    pub row: i32,
    pub col: i32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub rowspan: i32,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub colspan: i32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_header: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CndNode>,
}

/// A table whose rows and columns carry meaning. `kind` ("data",
/// "content") comes from the author only; `None` means unstated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TableNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Cell>,
}

/// A grid used for arrangement only; position carries no relation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LayoutNode {
    #[serde(flatten)]
    pub base: NodeBase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Cell>,
}

fn default_json_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn is_empty_object(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(serde_json::Map::is_empty)
}

fn default_one() -> i32 {
    1
}

// `skip_serializing_if` hands the predicate a reference — taking these by
// value, as clippy suggests for small Copy types, does not typecheck there.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(v: &i32) -> bool {
    *v == 1
}

impl NodeBase {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            id,
            label: None,
            state_metadata: serde_json::Map::new(),
            location,
        }
    }
}

impl HeadingNode {
    pub fn new(
        id: Uuid,
        level: i32,
        numbering: Option<Numbering>,
        text: RichText,
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
    pub fn new(id: Uuid, text: RichText, location: NodeLocation) -> Self {
        Self { base: NodeBase::new(id, location), text }
    }
}

impl QuoteNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            attribution: None,
            children: Vec::new(),
        }
    }
}

impl AdmonitionNode {
    pub fn new(id: Uuid, kind: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            kind,
            title: None,
            children: Vec::new(),
        }
    }
}

impl CodeNode {
    pub fn new(id: Uuid, text: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            text,
            lang: None,
            title: None,
        }
    }
}

impl MathNode {
    pub fn new(id: Uuid, text: String, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            text,
            raw: None,
            numbering: None,
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

impl FigureNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            kind: None,
            caption: None,
            description: None,
            numbering: None,
            children: Vec::new(),
            raw: None,
        }
    }
}

impl ListNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            ordered: false,
            items: Vec::new(),
        }
    }
}

impl TermsNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            items: Vec::new(),
        }
    }
}

impl TableNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            kind: None,
            cells: Vec::new(),
        }
    }
}

impl LayoutNode {
    pub fn new(id: Uuid, location: NodeLocation) -> Self {
        Self {
            base: NodeBase::new(id, location),
            cells: Vec::new(),
        }
    }
}
