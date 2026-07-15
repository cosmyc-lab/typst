//! Serde types matching the cnd-engine manifest contract.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CND_VERSION: &str = "0.1.0";

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
    pub page: i32,
    pub span: i32,
    pub page_span: i32,
    pub parent_span: i32,
    pub span_count: i32,
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
    List(ListNode),
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
            Self::List(n) => &n.base,
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
            Self::List(n) => &mut n.base,
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

    pub fn children_mut(&mut self) -> Option<&mut Vec<CndNode>> {
        match self {
            Self::Heading(n) => Some(&mut n.children),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fig_number: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_typst: Option<String>,
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
            caption: None,
            fig_number: None,
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
            alt: None,
            path: None,
            raw_typst: None,
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
