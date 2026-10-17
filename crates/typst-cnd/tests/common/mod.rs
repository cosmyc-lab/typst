//! Shared helpers for typst-cnd integration tests.
//!
//! Every integration test binary that does `mod common;` compiles this whole
//! file, so a helper only one binary uses is dead code in the others. CI runs
//! with `RUSTFLAGS: -Dwarnings`, which turns that into a build failure — so the
//! allow is what lets a test file use a subset without breaking the rest.
#![allow(dead_code)]

use std::collections::HashSet;
use std::path::PathBuf;

use typst_cnd::{
    Cnd, CndDocument, CndNode, ListItem, ListNode, RichText, cnd_from_document,
    cnd_to_json, walk,
    world::{CndWorld, built_at_now, source_info},
};

/// Node counts over the whole tree: every slot of every container (a list
/// item's paragraph, a nested list, a cell's paragraph) is counted.
#[derive(Debug, Default)]
pub struct NodeStats {
    pub nodes: usize,
    pub headings: usize,
    pub paragraphs: usize,
    pub tables: usize,
    pub layouts: usize,
    pub quotes: usize,
    pub code: usize,
    pub math: usize,
    pub figures: usize,
    pub images: usize,
    pub admonitions: usize,
    pub lists: usize,
    pub terms: usize,
    pub max_heading_level: i32,
    pub pages: HashSet<i32>,
    pub labels: HashSet<String>,
    pub ids: HashSet<uuid::Uuid>,
}

#[derive(Debug, Default)]
pub struct TableStats {
    pub with_caption: usize,
    pub with_fig_number: usize,
    pub with_label: usize,
}

pub fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples")
}

pub fn example_path(name: &str) -> PathBuf {
    examples_dir().join(name)
}

pub fn compile_example(name: &str) -> CndDocument {
    let path = example_path(name);
    let world = CndWorld::new(&path).unwrap_or_else(|err| {
        panic!("failed to initialize world for {}: {err:?}", path.display())
    });
    let warned = typst::compile::<CndDocument>(&world);
    for warning in &warned.warnings {
        eprintln!("warning: {warning:?}");
    }
    warned.output.unwrap_or_else(|errors| {
        panic!("compile failed for {}: {errors:?}", path.display())
    })
}

pub fn cnd_for_example(name: &str) -> Cnd {
    let path = example_path(name);
    let world = CndWorld::new(&path).expect("world");
    let doc = compile_example(name);
    cnd_from_document(&doc, source_info(&world), built_at_now())
}

/// Every node of the tree in reading order, every slot entered.
pub fn all_nodes(nodes: &[CndNode]) -> Vec<&CndNode> {
    let mut out = Vec::new();
    walk(nodes, &mut |node| out.push(node));
    out
}

/// Every node of the CND: the body, then the content of every footnote.
pub fn all_nodes_full(cnd: &Cnd) -> Vec<&CndNode> {
    let mut out = all_nodes(&cnd.nodes);
    for note in &cnd.footnotes {
        out.extend(all_nodes(&note.children));
    }
    out
}

/// Every text of every node in `nodes`, in reading order.
pub fn all_texts(nodes: &[CndNode]) -> Vec<&RichText> {
    all_nodes(nodes).into_iter().flat_map(|node| node.texts()).collect()
}

/// The value of the first paragraph among `nodes` (a slot's content), or
/// `""` when there is none.
pub fn first_paragraph(nodes: &[CndNode]) -> &str {
    nodes
        .iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) => Some(p.text.value.as_str()),
            _ => None,
        })
        .unwrap_or("")
}

/// A list item's own text: the value of its paragraph.
pub fn item_text(item: &ListItem) -> &str {
    first_paragraph(&item.children)
}

/// The lists nested directly in a list item.
pub fn nested_lists(item: &ListItem) -> Vec<&ListNode> {
    item.children
        .iter()
        .filter_map(|node| match node {
            CndNode::List(list) => Some(list),
            _ => None,
        })
        .collect()
}

/// The items of the lists nested in a list item, in order.
pub fn nested_items(item: &ListItem) -> Vec<&ListItem> {
    nested_lists(item)
        .into_iter()
        .flat_map(|list| list.items.iter())
        .collect()
}

/// The paragraph values of a footnote's content, joined by a space.
pub fn footnote_text(note: &typst_cnd::Footnote) -> String {
    all_nodes(&note.children)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Paragraph(p) => Some(p.text.value.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn heading_texts(nodes: &[CndNode]) -> Vec<String> {
    all_nodes(nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Heading(h) => Some(h.text.value.clone()),
            _ => None,
        })
        .collect()
}

pub fn walk_nodes(nodes: &[CndNode], stats: &mut NodeStats) {
    walk(nodes, &mut |node| {
        stats.nodes += 1;
        stats.ids.insert(node.id());
        stats.pages.insert(node.base().location.page);
        if let Some(label) = &node.base().label {
            stats.labels.insert(label.clone());
        }
        match node {
            CndNode::Heading(h) => {
                stats.headings += 1;
                stats.max_heading_level = stats.max_heading_level.max(h.level);
            }
            CndNode::Paragraph(_) => stats.paragraphs += 1,
            CndNode::Table(_) => stats.tables += 1,
            CndNode::Layout(_) => stats.layouts += 1,
            CndNode::Quote(_) => stats.quotes += 1,
            CndNode::Code(_) => stats.code += 1,
            CndNode::Math(_) => stats.math += 1,
            CndNode::Figure(_) => stats.figures += 1,
            CndNode::Image(_) => stats.images += 1,
            CndNode::Admonition(_) => stats.admonitions += 1,
            CndNode::List(_) => stats.lists += 1,
            CndNode::Terms(_) => stats.terms += 1,
        }
    });
}

/// Stats of the tabular nodes. A captioned table's caption/number/label
/// live on the wrapping `FigureNode` (ADR 0010); a grid in a figure is a
/// `layout`, the 0.5 form of the former grid-kind table, and counts here as
/// it did then.
pub fn table_stats(nodes: &[CndNode]) -> TableStats {
    let mut stats = TableStats::default();
    walk(nodes, &mut |node| match node {
        CndNode::Table(table) if table.base.label.is_some() => stats.with_label += 1,
        CndNode::Figure(figure) => {
            let has_caption =
                figure.caption.as_ref().is_some_and(|c| !c.value.is_empty());
            let has_fig_number =
                figure.numbering.as_ref().is_some_and(|n| !n.value.is_empty());
            for child in &figure.children {
                if matches!(child, CndNode::Table(_) | CndNode::Layout(_)) {
                    if has_caption {
                        stats.with_caption += 1;
                    }
                    if has_fig_number {
                        stats.with_fig_number += 1;
                    }
                    if figure.base.label.is_some() {
                        stats.with_label += 1;
                    }
                }
            }
        }
        _ => {}
    });
    stats
}

pub fn assert_cnd_contract(cnd: &Cnd) {
    assert_eq!(cnd.cnd_version, typst_cnd::CND_VERSION);
    let source = cnd.source.as_ref().expect("source block");
    assert_eq!(source.type_, "typst");
    assert!(source.hash.starts_with("sha256:"));
    // `uri` is a producer-local identifier, never promised resolvable — and
    // never absolute, which would leak the filesystem tree (spec §2.1).
    let uri = source.uri.as_deref().expect("source uri");
    assert!(!uri.starts_with('/'), "source.uri must stay relative, got {uri}");
    assert!(!cnd.built_at.is_empty());
    assert!(!cnd.doc.title.is_empty());
    assert!(!cnd.doc.authors.is_empty());
}

pub fn assert_unique_ids(nodes: &[CndNode]) {
    let mut stats = NodeStats::default();
    walk_nodes(nodes, &mut stats);
    assert_eq!(stats.ids.len(), stats.nodes, "duplicate node ids detected");
}

/// Slice a string by `[start, end)` Unicode code-point offsets — the
/// coordinate space of a link's `text_span` (ADR 0013). Panics if the span
/// is out of range, which is itself a useful assertion.
pub fn codepoint_slice(text: &str, span: &[i64]) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[span[0] as usize..span[1] as usize].iter().collect()
}

/// Number of nodes whose texts' forward `refs` name `target` — the derived
/// incoming-edge count (ADR 0008 removed the materialized `refs_from`; the
/// SDK computes this via `Cnd.incoming`). Keyed by label, since that is
/// what an edge carries (ADR 0017).
pub fn incoming_count(nodes: &[CndNode], target: &str) -> usize {
    all_nodes(nodes)
        .into_iter()
        .filter(|node| {
            node.texts()
                .iter()
                .any(|text| text.refs.iter().any(|reference| reference.label == target))
        })
        .count()
}

/// Collect every node label in the tree.
pub fn node_labels(nodes: &[CndNode]) -> HashSet<String> {
    all_nodes(nodes)
        .into_iter()
        .filter_map(|node| node.base().label.clone())
        .collect()
}

/// Every forward `refs` edge must name a label some node carries (ADR
/// 0017). Existence only — the reverse-consistency invariant is gone with
/// `refs_from` (ADR 0008), and the SDK derives incoming edges on demand.
pub fn assert_refs_resolve(nodes: &[CndNode]) {
    let labels = node_labels(nodes);
    for node in all_nodes(nodes) {
        for text in node.texts() {
            for reference in &text.refs {
                assert!(
                    labels.contains(&reference.label),
                    "refs target @{} from {} resolves to no node",
                    reference.label,
                    node.id()
                );
            }
        }
    }
}

/// Labels are globally unique across nodes, footnote content and both pools
/// (ADR 0017) — the invariant that lets an edge name its target by label
/// alone. Typst itself permits duplicate labels (it only errors on ambiguous
/// *resolution*), so this is a real thing a document can violate, not a
/// formality.
pub fn assert_labels_globally_unique(cnd: &Cnd) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut claim = |label: &str, owner: String| {
        assert!(seen.insert(label.to_string()), "duplicate label @{label} on {owner}");
    };
    for node in all_nodes_full(cnd) {
        if let Some(label) = &node.base().label {
            claim(label, format!("node {}", node.id()));
        }
    }
    for entry in &cnd.bibliography {
        claim(&entry.label, format!("bibliography entry {}", entry.id));
    }
    for note in &cnd.footnotes {
        claim(&note.label, format!("footnote {}", note.id));
    }
}

/// Every `cites`/`footnotes` edge on a text must resolve to a real pool
/// entry, and pool-entry ids must be globally unique and disjoint from
/// node ids (proposal 0004 invariants).
pub fn assert_pool_refs_resolve(cnd: &Cnd) {
    let footnote_ids: HashSet<uuid::Uuid> = cnd.footnotes.iter().map(|f| f.id).collect();
    let bib_ids: HashSet<uuid::Uuid> = cnd.bibliography.iter().map(|b| b.id).collect();
    let footnote_labels: HashSet<&str> =
        cnd.footnotes.iter().map(|f| f.label.as_str()).collect();
    let bib_labels: HashSet<&str> =
        cnd.bibliography.iter().map(|b| b.label.as_str()).collect();

    // Each family resolves in its OWN domain — existence is not enough,
    // because labels are unique document-wide, so a `cites` edge naming a
    // footnote would still resolve to something (spec §5).
    let nodes = all_nodes_full(cnd);
    for node in &nodes {
        for text in node.texts() {
            for reference in &text.footnotes {
                assert!(
                    footnote_labels.contains(reference.label.as_str()),
                    "footnote edge @{} does not resolve in the footnotes pool",
                    reference.label
                );
            }
            for citation in &text.cites {
                assert!(
                    bib_labels.contains(citation.label.as_str()),
                    "cite edge @{} does not resolve in the bibliography pool",
                    citation.label
                );
            }
        }
    }

    // Pool ids are unique and disjoint from each other and from node ids.
    let node_ids: HashSet<uuid::Uuid> = nodes.iter().map(|node| node.id()).collect();
    assert_eq!(footnote_ids.len(), cnd.footnotes.len(), "duplicate footnote ids");
    assert_eq!(bib_ids.len(), cnd.bibliography.len(), "duplicate bib ids");
    assert!(footnote_ids.is_disjoint(&bib_ids), "footnote/bib ids overlap");
    assert!(
        footnote_ids.is_disjoint(&node_ids) && bib_ids.is_disjoint(&node_ids),
        "pool ids overlap node ids"
    );
}

pub fn assert_json_roundtrip(cnd: &Cnd) {
    let json = cnd_to_json(cnd).expect("serialize");
    let parsed: Cnd = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.cnd_version, cnd.cnd_version);
    assert_eq!(parsed.doc.title, cnd.doc.title);
    assert_eq!(parsed.nodes.len(), cnd.nodes.len());
}

pub fn label_exists(nodes: &[CndNode], label: &str) -> bool {
    let mut stats = NodeStats::default();
    walk_nodes(nodes, &mut stats);
    stats.labels.contains(label)
}

pub fn find_by_label<'a>(nodes: &'a [CndNode], label: &str) -> Option<&'a CndNode> {
    all_nodes(nodes)
        .into_iter()
        .find(|node| node.base().label.as_deref() == Some(label))
}

pub fn all_example_files() -> Vec<PathBuf> {
    std::fs::read_dir(examples_dir())
        .expect("examples dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "typ")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.starts_with('_'))
        })
        .collect()
}

/// The paragraphs of the reading flow, in order: those of the body and of
/// its sections, read through layouts. A grid used for arrangement is a
/// `layout` whose cells hold its blocks, in reading order, so its
/// paragraphs are read where the grid sits — as they were when grids were
/// flattened. A table's, a list's or a quote's paragraphs are not part of
/// the flow: what these tests pin is the reading flow, and that the text a
/// container carries is not *also* emitted as a paragraph of the flow.
pub fn paragraph_texts_in_order(nodes: &[CndNode]) -> Vec<String> {
    let mut out = Vec::new();
    fn flow(nodes: &[CndNode], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => out.push(p.text.value.clone()),
                CndNode::Heading(h) => flow(&h.children, out),
                CndNode::Layout(l) => {
                    for cell in &l.cells {
                        flow(&cell.children, out);
                    }
                }
                _ => {}
            }
        }
    }
    flow(nodes, &mut out);
    out
}

/// Extract `[TAG]` markers from paragraph texts for order assertions.
pub fn paragraph_tags_in_order(nodes: &[CndNode]) -> Vec<String> {
    paragraph_texts_in_order(nodes)
        .iter()
        .filter_map(|text| {
            text.trim_start()
                .strip_prefix('[')
                .and_then(|rest| rest.split(']').next())
                .map(str::to_string)
        })
        .collect()
}

pub fn assert_tag_sequence(nodes: &[CndNode], expected: &[&str]) {
    let tags = paragraph_tags_in_order(nodes);
    assert_eq!(
        tags,
        expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "unexpected paragraph reading order"
    );
}

pub fn tags_under_heading(nodes: &[CndNode], label: &str) -> Vec<String> {
    let Some(CndNode::Heading(heading)) = find_by_label(nodes, label) else {
        panic!("heading {label} not found");
    };
    paragraph_tags_in_order(&heading.children)
}

/// Every list node in reading order, nested lists included.
pub fn find_lists(nodes: &[CndNode]) -> Vec<&ListNode> {
    all_nodes(nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::List(list) => Some(list),
            _ => None,
        })
        .collect()
}

/// The lists not nested in another list's item, in reading order.
pub fn outer_lists(nodes: &[CndNode]) -> Vec<&ListNode> {
    let lists = find_lists(nodes);
    let nested: HashSet<uuid::Uuid> = lists
        .iter()
        .flat_map(|list| list.items.iter())
        .flat_map(nested_lists)
        .map(|list| list.base.id)
        .collect();
    lists
        .into_iter()
        .filter(|list| !nested.contains(&list.base.id))
        .collect()
}

/// A node's `type`, as the format names it.
pub fn node_type(node: &CndNode) -> &'static str {
    match node {
        CndNode::Heading(_) => "heading",
        CndNode::Paragraph(_) => "paragraph",
        CndNode::Quote(_) => "quote",
        CndNode::Code(_) => "code",
        CndNode::Math(_) => "math",
        CndNode::Image(_) => "image",
        CndNode::Figure(_) => "figure",
        CndNode::Admonition(_) => "admonition",
        CndNode::List(_) => "list",
        CndNode::Terms(_) => "terms",
        CndNode::Table(_) => "table",
        CndNode::Layout(_) => "layout",
    }
}

/// The types of `nodes`, in order.
pub fn node_types(nodes: &[CndNode]) -> Vec<&'static str> {
    nodes.iter().map(node_type).collect()
}

/// The values of the paragraphs among `nodes` (not nested ones), in order.
pub fn paragraph_values(nodes: &[CndNode]) -> Vec<&str> {
    nodes
        .iter()
        .filter_map(|node| match node {
            CndNode::Paragraph(p) => Some(p.text.value.as_str()),
            _ => None,
        })
        .collect()
}
