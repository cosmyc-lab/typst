//! Shared helpers for typst-cnd integration tests.

use std::collections::HashSet;
use std::path::PathBuf;

use typst_cnd::{
    CndDocument, Cnd, CndNode, ListNode, cnd_from_document, cnd_to_json,
    world::{CndWorld, built_at_now, source_info},
};

#[derive(Debug, Default)]
pub struct NodeStats {
    pub headings: usize,
    pub paragraphs: usize,
    pub tables: usize,
    pub quotes: usize,
    pub code: usize,
    pub math: usize,
    pub figures: usize,
    pub images: usize,
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
    pub with_raw_typst: usize,
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
    warned
        .output
        .unwrap_or_else(|errors| panic!("compile failed for {}: {errors:?}", path.display()))
}

pub fn cnd_for_example(name: &str) -> Cnd {
    let path = example_path(name);
    let world = CndWorld::new(&path).expect("world");
    let doc = compile_example(name);
    cnd_from_document(&doc, source_info(&world), built_at_now())
}

pub fn heading_texts(nodes: &[CndNode]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(nodes: &[CndNode], out: &mut Vec<String>) {
        for node in nodes {
            if let CndNode::Heading(h) = node {
                out.push(h.text.clone());
                walk(&h.children, out);
            }
        }
    }
    walk(nodes, &mut out);
    out
}

pub fn walk_nodes(nodes: &[CndNode], stats: &mut NodeStats) {
    for node in nodes {
        stats.ids.insert(node.id());
        match node {
            CndNode::Heading(h) => {
                stats.headings += 1;
                stats.max_heading_level = stats.max_heading_level.max(h.level);
                stats.pages.insert(h.base.location.page);
                if let Some(label) = &h.base.label {
                    stats.labels.insert(label.clone());
                }
                walk_nodes(&h.children, stats);
            }
            CndNode::Paragraph(p) => {
                stats.paragraphs += 1;
                stats.pages.insert(p.base.location.page);
                if let Some(label) = &p.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::Table(t) => {
                stats.tables += 1;
                stats.pages.insert(t.base.location.page);
                if let Some(label) = &t.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::Quote(q) => {
                stats.quotes += 1;
                stats.pages.insert(q.base.location.page);
                if let Some(label) = &q.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::Code(c) => {
                stats.code += 1;
                stats.pages.insert(c.base.location.page);
                if let Some(label) = &c.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::Math(m) => {
                stats.math += 1;
                stats.pages.insert(m.base.location.page);
                if let Some(label) = &m.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::Figure(f) => {
                stats.figures += 1;
                stats.pages.insert(f.base.location.page);
                if let Some(label) = &f.base.label {
                    stats.labels.insert(label.clone());
                }
                walk_nodes(&f.children, stats);
            }
            CndNode::Image(img) => {
                stats.images += 1;
                stats.pages.insert(img.base.location.page);
                if let Some(label) = &img.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::List(l) => {
                stats.lists += 1;
                stats.pages.insert(l.base.location.page);
                if let Some(label) = &l.base.label {
                    stats.labels.insert(label.clone());
                }
            }
            CndNode::Terms(t) => {
                stats.terms += 1;
                stats.pages.insert(t.base.location.page);
                if let Some(label) = &t.base.label {
                    stats.labels.insert(label.clone());
                }
            }
        }
    }
}

/// Table stats. A captioned table's caption/number/label now live on the
/// wrapping `FigureNode` (ADR 0010), not on the `TableNode` itself — this
/// walk attributes them accordingly instead of reading removed fields.
pub fn table_stats(nodes: &[CndNode]) -> TableStats {
    let mut stats = TableStats::default();
    fn walk(nodes: &[CndNode], stats: &mut TableStats) {
        for node in nodes {
            match node {
                CndNode::Table(table) => {
                    if table.raw.as_ref().is_some_and(|r| !r.value.is_empty()) {
                        stats.with_raw_typst += 1;
                    }
                    if table.base.label.is_some() {
                        stats.with_label += 1;
                    }
                }
                CndNode::Figure(figure) => {
                    let has_caption = figure.caption.as_ref().is_some_and(|c| !c.is_empty());
                    let has_fig_number =
                        figure.number.as_ref().is_some_and(|n| !n.is_empty());
                    for child in &figure.children {
                        if let CndNode::Table(table) = child {
                            if has_caption {
                                stats.with_caption += 1;
                            }
                            if has_fig_number {
                                stats.with_fig_number += 1;
                            }
                            if table.raw.as_ref().is_some_and(|r| !r.value.is_empty()) {
                                stats.with_raw_typst += 1;
                            }
                            if figure.base.label.is_some() {
                                stats.with_label += 1;
                            }
                        }
                    }
                }
                CndNode::Heading(h) => walk(&h.children, stats),
                CndNode::Paragraph(_)
                | CndNode::Quote(_)
                | CndNode::Code(_)
                | CndNode::Math(_)
                | CndNode::Image(_)
                | CndNode::List(_)
                | CndNode::Terms(_) => {}
            }
        }
    }
    walk(nodes, &mut stats);
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
    let total = stats.headings
        + stats.paragraphs
        + stats.tables
        + stats.quotes
        + stats.code
        + stats.math
        + stats.figures
        + stats.images
        + stats.lists
        + stats.terms;
    assert_eq!(
        stats.ids.len(),
        total,
        "duplicate node ids detected"
    );
}

/// Slice a string by `[start, end)` Unicode code-point offsets — the
/// coordinate space of a link's `text_span` (ADR 0013). Panics if the span
/// is out of range, which is itself a useful assertion.
pub fn codepoint_slice(text: &str, span: &[i64]) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[span[0] as usize..span[1] as usize].iter().collect()
}

/// Number of nodes whose forward `refs` name `target` — the derived
/// incoming-edge count (ADR 0008 removed the materialized `refs_from`; the
/// SDK computes this via `Cnd.incoming`). Keyed by label, since that is
/// what an edge carries (ADR 0017).
pub fn incoming_count(nodes: &[CndNode], target: &str) -> usize {
    fn walk(nodes: &[CndNode], target: &str, count: &mut usize) {
        for node in nodes {
            if node.base().refs.iter().any(|reference| reference.label == target) {
                *count += 1;
            }
            match node {
                CndNode::Heading(h) => walk(&h.children, target, count),
                CndNode::Figure(f) => walk(&f.children, target, count),
                _ => {}
            }
        }
    }
    let mut count = 0;
    walk(nodes, target, &mut count);
    count
}

/// Collect every node label in the tree.
pub fn node_labels(nodes: &[CndNode]) -> HashSet<String> {
    fn walk(nodes: &[CndNode], out: &mut HashSet<String>) {
        for node in nodes {
            if let Some(label) = &node.base().label {
                out.insert(label.clone());
            }
            match node {
                CndNode::Heading(h) => walk(&h.children, out),
                CndNode::Figure(f) => walk(&f.children, out),
                _ => {}
            }
        }
    }
    let mut out = HashSet::new();
    walk(nodes, &mut out);
    out
}

/// Every forward `refs` edge must name a label some node carries (ADR
/// 0017). Existence only — the reverse-consistency invariant is gone with
/// `refs_from` (ADR 0008), and the SDK derives incoming edges on demand.
pub fn assert_refs_resolve(nodes: &[CndNode]) {
    let labels = node_labels(nodes);

    fn check(nodes: &[CndNode], labels: &HashSet<String>) {
        for node in nodes {
            for reference in &node.base().refs {
                assert!(
                    labels.contains(&reference.label),
                    "refs target @{} from {} resolves to no node",
                    reference.label,
                    node.id()
                );
            }
            match node {
                CndNode::Heading(h) => check(&h.children, labels),
                CndNode::Figure(f) => check(&f.children, labels),
                _ => {}
            }
        }
    }
    check(nodes, &labels);
}

/// Labels are globally unique across nodes and both pools (ADR 0017) — the
/// invariant that lets an edge name its target by label alone. Typst itself
/// permits duplicate labels (it only errors on ambiguous *resolution*), so
/// this is a real thing a document can violate, not a formality.
pub fn assert_labels_globally_unique(cnd: &Cnd) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut claim = |label: &str, owner: String| {
        assert!(seen.insert(label.to_string()), "duplicate label @{label} on {owner}");
    };

    fn walk(nodes: &[CndNode], claim: &mut impl FnMut(&str, String)) {
        for node in nodes {
            if let Some(label) = &node.base().label {
                claim(label, format!("node {}", node.id()));
            }
            match node {
                CndNode::Heading(h) => walk(&h.children, claim),
                CndNode::Figure(f) => walk(&f.children, claim),
                _ => {}
            }
        }
    }
    walk(&cnd.nodes, &mut claim);
    for entry in &cnd.bibliography {
        claim(&entry.label, format!("bibliography entry {}", entry.id));
    }
    for note in &cnd.footnotes {
        claim(&note.label, format!("footnote {}", note.id));
    }
}

/// Every `cites`/`footnotes` edge on a node must resolve to a real pool
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
    fn walk(
        nodes: &[CndNode],
        footnote_labels: &HashSet<&str>,
        bib_labels: &HashSet<&str>,
    ) {
        for node in nodes {
            for reference in &node.base().footnotes {
                assert!(
                    footnote_labels.contains(reference.label.as_str()),
                    "footnote edge @{} does not resolve in the footnotes pool",
                    reference.label
                );
            }
            for citation in &node.base().cites {
                assert!(
                    bib_labels.contains(citation.label.as_str()),
                    "cite edge @{} does not resolve in the bibliography pool",
                    citation.label
                );
            }
            match node {
                CndNode::Heading(h) => walk(&h.children, footnote_labels, bib_labels),
                CndNode::Figure(f) => walk(&f.children, footnote_labels, bib_labels),
                _ => {}
            }
        }
    }
    walk(&cnd.nodes, &footnote_labels, &bib_labels);

    // Pool ids are unique and disjoint from each other and from node ids.
    let mut node_ids = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut node_ids);
    assert_eq!(footnote_ids.len(), cnd.footnotes.len(), "duplicate footnote ids");
    assert_eq!(bib_ids.len(), cnd.bibliography.len(), "duplicate bib ids");
    assert!(footnote_ids.is_disjoint(&bib_ids), "footnote/bib ids overlap");
    assert!(
        footnote_ids.is_disjoint(&node_ids.ids) && bib_ids.is_disjoint(&node_ids.ids),
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
    for node in nodes {
        let node_label = node.base().label.as_deref();
        if node_label == Some(label) {
            return Some(node);
        }
        if let CndNode::Heading(h) = node {
            if let Some(found) = find_by_label(&h.children, label) {
                return Some(found);
            }
        }
        if let CndNode::Figure(f) = node {
            if let Some(found) = find_by_label(&f.children, label) {
                return Some(found);
            }
        }
    }
    None
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

/// Collect paragraph texts in depth-first cnd order (reading order).
pub fn paragraph_texts_in_order(nodes: &[CndNode]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(nodes: &[CndNode], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => out.push(p.text.clone()),
                CndNode::Heading(h) => walk(&h.children, out),
                CndNode::Table(_)
                | CndNode::Quote(_)
                | CndNode::Code(_)
                | CndNode::Math(_)
                | CndNode::Figure(_)
                | CndNode::Image(_)
                | CndNode::List(_)
                | CndNode::Terms(_) => {}
            }
        }
    }
    walk(nodes, &mut out);
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

/// Collect list nodes in depth-first cnd order.
pub fn find_lists(nodes: &[CndNode]) -> Vec<&ListNode> {
    let mut out = Vec::new();
    fn walk<'a>(nodes: &'a [CndNode], out: &mut Vec<&'a ListNode>) {
        for node in nodes {
            match node {
                CndNode::List(list) => out.push(list),
                CndNode::Heading(h) => walk(&h.children, out),
                CndNode::Paragraph(_)
                | CndNode::Table(_)
                | CndNode::Quote(_)
                | CndNode::Code(_)
                | CndNode::Math(_)
                | CndNode::Figure(_)
                | CndNode::Image(_)
                | CndNode::Terms(_) => {}
            }
        }
    }
    walk(nodes, &mut out);
    out
}
