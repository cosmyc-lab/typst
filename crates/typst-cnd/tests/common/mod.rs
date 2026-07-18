//! Shared helpers for typst-cnd integration tests.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use typst_cnd::{
    CndDocument, CndManifest, CndNode, ListNode, manifest_from_document, manifest_to_json,
    world::{CndWorld, compiled_at_now, doc_hash},
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

pub fn manifest_for_example(name: &str) -> CndManifest {
    let path = example_path(name);
    let world = CndWorld::new(&path).expect("world");
    let doc = compile_example(name);
    manifest_from_document(&doc, doc_hash(&world), compiled_at_now())
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
                    if table.raw_typst.as_ref().is_some_and(|r| !r.is_empty()) {
                        stats.with_raw_typst += 1;
                    }
                    if table.base.label.is_some() {
                        stats.with_label += 1;
                    }
                }
                CndNode::Figure(figure) => {
                    let has_caption = figure.caption.as_ref().is_some_and(|c| !c.is_empty());
                    let has_fig_number =
                        figure.fig_number.as_ref().is_some_and(|n| !n.is_empty());
                    for child in &figure.children {
                        if let CndNode::Table(table) = child {
                            if has_caption {
                                stats.with_caption += 1;
                            }
                            if has_fig_number {
                                stats.with_fig_number += 1;
                            }
                            if table.raw_typst.as_ref().is_some_and(|r| !r.is_empty()) {
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

pub fn assert_manifest_contract(manifest: &CndManifest) {
    assert_eq!(manifest.cnd_version, typst_cnd::CND_VERSION);
    assert!(manifest.doc_hash.starts_with("sha256:"));
    assert!(!manifest.compiled_at.is_empty());
    assert!(!manifest.doc.title.is_empty());
    assert!(!manifest.doc.authors.is_empty());
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

pub fn assert_refs_resolve(nodes: &[CndNode]) {
    let mut by_id: HashMap<
        uuid::Uuid,
        (&CndNode, Vec<typst_cnd::NodeRef>, Vec<typst_cnd::NodeRef>),
    > = HashMap::new();

    fn walk<'a>(
        nodes: &'a [CndNode],
        out: &mut HashMap<
            uuid::Uuid,
            (&'a CndNode, Vec<typst_cnd::NodeRef>, Vec<typst_cnd::NodeRef>),
        >,
    ) {
        for node in nodes {
            let (refs_to, refs_from) = match node {
                CndNode::Heading(h) => (&h.base.refs_to, &h.base.refs_from),
                CndNode::Paragraph(p) => (&p.base.refs_to, &p.base.refs_from),
                CndNode::Table(t) => (&t.base.refs_to, &t.base.refs_from),
                CndNode::Quote(q) => (&q.base.refs_to, &q.base.refs_from),
                CndNode::Code(c) => (&c.base.refs_to, &c.base.refs_from),
                CndNode::Math(m) => (&m.base.refs_to, &m.base.refs_from),
                CndNode::Figure(f) => (&f.base.refs_to, &f.base.refs_from),
                CndNode::Image(img) => (&img.base.refs_to, &img.base.refs_from),
                CndNode::List(l) => (&l.base.refs_to, &l.base.refs_from),
                CndNode::Terms(t) => (&t.base.refs_to, &t.base.refs_from),
            };
            out.insert(
                node.id(),
                (node, refs_to.clone(), refs_from.clone()),
            );
            match node {
                CndNode::Heading(h) => walk(&h.children, out),
                CndNode::Figure(f) => walk(&f.children, out),
                _ => {}
            }
        }
    }

    walk(nodes, &mut by_id);

    for (id, (_, refs_to, refs_from)) in &by_id {
        for target in refs_to {
            assert!(
                by_id.contains_key(&target.id),
                "refs_to target {} from {id} does not exist",
                target.id
            );
        }
        for source in refs_from {
            assert!(
                by_id.contains_key(&source.id),
                "refs_from source {} on {id} does not exist",
                source.id
            );
        }
    }

    for (source_id, (_, refs_to, _)) in &by_id {
        for target in refs_to {
            let (_, _, refs_from) = by_id.get(&target.id).expect("target");
            assert!(
                refs_from.iter().any(|reference| reference.id == *source_id),
                "missing reverse refs_from on {} for refs_to from {source_id}",
                target.id
            );
        }
    }
}

/// Every `cites`/`footnotes` edge on a node must resolve to a real pool
/// entry, and pool-entry ids must be globally unique and disjoint from
/// node ids (proposal 0004 invariants).
pub fn assert_pool_refs_resolve(manifest: &CndManifest) {
    let footnote_ids: HashSet<uuid::Uuid> = manifest.footnotes.iter().map(|f| f.id).collect();
    let bib_ids: HashSet<uuid::Uuid> = manifest.bibliography.iter().map(|b| b.id).collect();

    fn walk(
        nodes: &[CndNode],
        footnote_ids: &HashSet<uuid::Uuid>,
        bib_ids: &HashSet<uuid::Uuid>,
    ) {
        for node in nodes {
            for reference in &node.base().footnotes {
                assert!(
                    footnote_ids.contains(&reference.id),
                    "footnote edge {} does not resolve to a pool entry",
                    reference.id
                );
            }
            for citation in &node.base().cites {
                assert!(
                    bib_ids.contains(&citation.id),
                    "cite edge {} does not resolve to a bibliography entry",
                    citation.id
                );
            }
            match node {
                CndNode::Heading(h) => walk(&h.children, footnote_ids, bib_ids),
                CndNode::Figure(f) => walk(&f.children, footnote_ids, bib_ids),
                _ => {}
            }
        }
    }
    walk(&manifest.nodes, &footnote_ids, &bib_ids);

    // Pool ids are unique and disjoint from each other and from node ids.
    let mut node_ids = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut node_ids);
    assert_eq!(footnote_ids.len(), manifest.footnotes.len(), "duplicate footnote ids");
    assert_eq!(bib_ids.len(), manifest.bibliography.len(), "duplicate bib ids");
    assert!(footnote_ids.is_disjoint(&bib_ids), "footnote/bib ids overlap");
    assert!(
        footnote_ids.is_disjoint(&node_ids.ids) && bib_ids.is_disjoint(&node_ids.ids),
        "pool ids overlap node ids"
    );
}

pub fn assert_json_roundtrip(manifest: &CndManifest) {
    let json = manifest_to_json(manifest).expect("serialize");
    let parsed: CndManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.cnd_version, manifest.cnd_version);
    assert_eq!(parsed.doc.title, manifest.doc.title);
    assert_eq!(parsed.nodes.len(), manifest.nodes.len());
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

/// Collect paragraph texts in depth-first manifest order (reading order).
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

/// Collect list nodes in depth-first manifest order.
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
