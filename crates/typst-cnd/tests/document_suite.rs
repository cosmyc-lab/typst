mod common;

use common::{
    all_example_files, assert_json_roundtrip, assert_manifest_contract, assert_refs_resolve,
    assert_tag_sequence, assert_unique_ids, compile_example, example_path, find_by_label,
    find_lists, heading_texts, label_exists, manifest_for_example, paragraph_texts_in_order,
    table_stats, tags_under_heading, walk_nodes, NodeStats,
};
use typst_cnd::{CndNode, TableKind};

#[test]
fn all_examples_compile() {
    let files = all_example_files();
    assert!(
        files.len() >= 9,
        "expected at least 9 example documents, found {}",
        files.len()
    );

    for path in files {
        let name = path.file_name().unwrap().to_string_lossy();
        let doc = compile_example(&name);
        assert!(
            !doc.nodes().is_empty() || name == "minimal.typ",
            "{} produced no nodes",
            name
        );
    }
}

#[test]
fn all_examples_produce_valid_manifests() {
    for path in all_example_files() {
        let name = path.file_name().unwrap().to_string_lossy();
        let manifest = manifest_for_example(&name);
        assert_manifest_contract(&manifest);
        assert_unique_ids(&manifest.nodes);
        assert_json_roundtrip(&manifest);
    }
}

#[test]
fn minimal_document_structure() {
    let doc = compile_example("minimal.typ");
    let mut stats = NodeStats::default();
    walk_nodes(doc.nodes(), &mut stats);

    assert_eq!(stats.headings, 1);
    assert_eq!(stats.paragraphs, 1);
    assert_eq!(stats.tables, 0);
    assert_eq!(stats.max_heading_level, 1);
}

#[test]
fn structured_cross_references() {
    let manifest = manifest_for_example("structured.typ");
    assert!(label_exists(&manifest.nodes, "tab-params-nominaux"));

    let table = find_by_label(&manifest.nodes, "tab-params-nominaux").expect("table");
    let CndNode::Table(table) = table else {
        panic!("expected table node");
    };
    assert_eq!(
        table.caption.as_deref(),
        Some("Paramètres nominaux de fonctionnement.")
    );
    assert!(table.fig_number.as_deref().is_some_and(|n| n.contains('1')));
    assert!(table.raw_typst.as_ref().is_some_and(|r| r.contains("table(")));
    assert!(!table.base.refs_from.is_empty());

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn comprehensive_document_structure() {
    let manifest = manifest_for_example("comprehensive.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    let root_paragraphs = manifest
        .nodes
        .iter()
        .filter(|n| matches!(n, CndNode::Paragraph(_)))
        .count();
    assert_eq!(
        root_paragraphs, 1,
        "expected one pre-heading paragraph at document root"
    );
    assert!(stats.paragraphs >= 8);
    assert!(stats.max_heading_level >= 3);
    assert_eq!(stats.tables, 4);

    let tables = table_stats(&manifest.nodes);
    assert_eq!(tables.with_caption, 4);
    assert_eq!(tables.with_fig_number, 4);
    assert_eq!(tables.with_label, 4);

    let arch = find_by_label(&manifest.nodes, "ch-architecture").expect("architecture heading");
    let CndNode::Heading(arch) = arch else {
        panic!("expected heading");
    };
    let para = arch
        .children
        .iter()
        .find(|n| matches!(n, CndNode::Paragraph(_)))
        .expect("paragraph under architecture");
    let CndNode::Paragraph(para) = para else {
        panic!("expected paragraph");
    };
    assert_eq!(para.base.state_metadata.get("revision").and_then(|v| v.as_str()), Some("4.2"));
    assert_eq!(para.base.state_metadata.get("status").and_then(|v| v.as_str()), Some("approved"));

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn complex_multipage_spans_pages() {
    let manifest = manifest_for_example("complex_multipage.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert!(stats.pages.len() >= 2, "expected content on multiple pages: {:?}", stats.pages);
    assert!(stats.headings >= 5);
    assert_eq!(stats.tables, 2);

    let f101 = find_by_label(&manifest.nodes, "sec-f101").expect("f101 heading");
    let CndNode::Heading(f101) = f101 else {
        panic!("expected heading");
    };
    let para = f101
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.base.state_metadata.contains_key("zone") => Some(p),
            _ => None,
        })
        .expect("paragraph with zone metadata");
    assert_eq!(para.base.state_metadata.get("criticality").and_then(|v| v.as_str()), Some("high"));

    let tables = table_stats(&manifest.nodes);
    assert_eq!(tables.with_caption, 2);
    assert_eq!(tables.with_fig_number, 2);

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn complex_refs_dense_graph() {
    let manifest = manifest_for_example("complex_refs.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert_eq!(stats.tables, 3);
    assert!(label_exists(&manifest.nodes, "tab-signaux"));
    assert!(label_exists(&manifest.nodes, "tab-boucles"));
    assert!(label_exists(&manifest.nodes, "tab-recap"));

    let overview = find_by_label(&manifest.nodes, "ch-overview").expect("overview");
    let CndNode::Heading(overview) = overview else {
        panic!("expected heading");
    };
    assert!(
        !overview.base.refs_to.is_empty(),
        "overview heading should reference labelled targets"
    );

    let signaux = find_by_label(&manifest.nodes, "tab-signaux").expect("signaux");
    let CndNode::Table(signaux) = signaux else {
        panic!("expected table");
    };
    assert!(signaux.base.refs_from.len() >= 2);

    let regulation = find_by_label(&manifest.nodes, "sec-regulation").expect("regulation");
    let CndNode::Heading(regulation) = regulation else {
        panic!("expected heading");
    };
    let regulation_para = regulation
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p)
                if p.base.state_metadata.get("domain").and_then(|v| v.as_str()) == Some("regulation") =>
            {
                Some(p)
            }
            _ => None,
        })
        .expect("regulation paragraph with metadata");
    assert!(!regulation_para.base.refs_to.is_empty());

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn complex_tables_standalone_and_figure_variants() {
    let manifest = manifest_for_example("complex_tables.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert_eq!(stats.tables, 3);
    assert!(label_exists(&manifest.nodes, "tab-config-standalone"));
    assert!(label_exists(&manifest.nodes, "tab-plages-a"));
    assert!(label_exists(&manifest.nodes, "tab-plages-b"));

    let standalone = find_by_label(&manifest.nodes, "tab-config-standalone").expect("standalone");
    let CndNode::Table(standalone) = standalone else {
        panic!("expected table");
    };
    assert!(standalone.caption.is_none(), "standalone table has no figure caption");
    assert!(standalone.fig_number.is_none());

    let tables = table_stats(&manifest.nodes);
    assert_eq!(tables.with_caption, 2, "only figure tables have captions");
    assert_eq!(tables.with_fig_number, 2);
    assert_eq!(tables.with_label, 3);

    let plages_a = find_by_label(&manifest.nodes, "tab-plages-a").expect("plages a");
    let plages_b = find_by_label(&manifest.nodes, "tab-plages-b").expect("plages b");
    let CndNode::Table(a) = plages_a else { panic!() };
    let CndNode::Table(b) = plages_b else { panic!() };
    assert_ne!(a.base.id, b.base.id);
    assert_ne!(a.caption, b.caption);
    assert!(a.cells.iter().any(|c| c.text.contains("PT-A")));
    assert!(b.cells.iter().any(|c| c.text.contains("PT-C")));

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn complex_columns_reading_order_flatten() {
    let manifest = manifest_for_example("complex_columns.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert_tag_sequence(
        &manifest.nodes,
        &[
            "INTRO", "L1", "L2", "M1", "M2", "R1", "OUT-L", "IN-L1", "IN-R1", "OUT-R", "PC1",
            "PC2", "PC3", "PC4", "PC5", "PC6", "PC7", "PC8", "POST",
        ],
    );

    assert_eq!(
        tags_under_heading(&manifest.nodes, "sec-three-cols"),
        vec!["L1", "L2", "M1", "M2", "R1"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        tags_under_heading(&manifest.nodes, "sec-nested-cols"),
        vec!["OUT-L", "IN-L1", "IN-R1", "OUT-R"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        tags_under_heading(&manifest.nodes, "ch-page-cols"),
        (1..=8)
            .map(|i| format!("PC{i}"))
            .collect::<Vec<_>>()
    );

    let three_cols = find_by_label(&manifest.nodes, "sec-three-cols").expect("three cols");
    let CndNode::Heading(three_cols) = three_cols else {
        panic!("expected heading");
    };
    let l1 = three_cols
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.text.starts_with("[L1]") => Some(p),
            _ => None,
        })
        .expect("L1 paragraph");
    assert_eq!(
        l1.base.state_metadata.get("track").and_then(|v| v.as_str()),
        Some("left")
    );

    let table = find_by_label(&manifest.nodes, "tab-bus-col").expect("table");
    let CndNode::Table(table) = table else {
        panic!("expected table");
    };
    assert!(table.caption.as_ref().is_some_and(|c| c.contains("bus")));
    assert_eq!(table.cells.len(), 6);

    let page_cols = find_by_label(&manifest.nodes, "ch-page-cols").expect("page cols");
    let CndNode::Heading(page_cols) = page_cols else {
        panic!("expected heading");
    };
    assert!(
        page_cols
            .children
            .iter()
            .filter_map(|n| match n {
                CndNode::Paragraph(p) => Some(p.base.location.page),
                _ => None,
            })
            .all(|page| page >= 2),
        "page-column paragraphs should start after the pagebreak"
    );

    let post = find_by_label(&manifest.nodes, "ch-single")
        .and_then(|n| match n {
            CndNode::Heading(h) => h.children.iter().find_map(|c| match c {
                CndNode::Paragraph(p) if p.text.starts_with("[POST]") => Some(p),
                _ => None,
            }),
            _ => None,
        })
        .expect("post paragraph");
    assert!(!post.base.refs_to.is_empty());

    assert_refs_resolve(&manifest.nodes);
}

/// Real-world newsletter template using `@preview/dashing-dept-news`.
///
/// First compile needs network access to fetch the package from Typst Universe.
#[test]
fn newsletter_dashing_dept_news_template() {
    let manifest = manifest_for_example("newsletter/main.typ");
    assert_eq!(manifest.cnd_version, typst_cnd::CND_VERSION);
    assert!(manifest.doc_hash.starts_with("sha256:"));
    assert_eq!(manifest.doc.title, "Chemistry Department");
    assert_unique_ids(&manifest.nodes);
    assert_json_roundtrip(&manifest);
    assert_refs_resolve(&manifest.nodes);

    assert_eq!(manifest.doc.title, "Chemistry Department");

    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);
    assert!(stats.headings >= 5);
    assert!(stats.paragraphs >= 10);

    let headings = heading_texts(&manifest.nodes);
    for expected in [
        "The Sixtus Award goes to Purview",
        "Guest lecture from Dr. Elizabeth Lee",
        "Safety first",
        "Tigers win big",
        "Another Success",
    ] {
        assert!(
            headings.iter().any(|h| h.contains(expected)),
            "missing heading containing {expected:?}, got {headings:?}"
        );
    }

    fn find_heading<'a>(nodes: &'a [CndNode], needle: &str) -> Option<&'a CndNode> {
        nodes.iter().find_map(|n| match n {
            CndNode::Heading(h) if h.text.contains(needle) => Some(n),
            _ => None,
        })
    }

    let sixtus = find_heading(&manifest.nodes, "Sixtus Award").expect("sixtus heading");
    let CndNode::Heading(sixtus) = sixtus else { panic!() };
    assert!(
        sixtus
            .children
            .iter()
            .filter(|c| matches!(c, CndNode::Paragraph(_)))
            .count()
            >= 5,
        "award section should contain multiple paragraphs"
    );
}

#[test]
fn rich_document_node_types() {
    let manifest = manifest_for_example("rich.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert!(stats.quotes >= 1, "expected at least one quote node");
    assert!(stats.lists >= 2, "expected bullet and numbered lists");
    assert!(stats.code >= 1, "expected a code block node");
    assert!(stats.math >= 1, "expected a block math node");
    assert!(stats.figures >= 1, "expected a non-table figure node");
    assert!(stats.tables >= 1, "expected a grid-in-figure table node");

    let grid = find_by_label(&manifest.nodes, "rich-grid").expect("grid figure");
    let CndNode::Table(grid) = grid else {
        panic!("expected grid table node");
    };
    assert_eq!(grid.kind, typst_cnd::TableKind::Grid);
    assert!(grid.caption.as_deref().is_some_and(|c| c.contains("grid")));

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn rich_document_no_duplicate_container_paragraphs() {
    let manifest = manifest_for_example("rich.typ");
    let paragraphs = paragraph_texts_in_order(&manifest.nodes);

    assert_eq!(
        paragraphs,
        vec![
            "A paragraph before structured blocks.".to_string(),
            "See Equation\u{a0}1, Figure\u{a0}1, and Figure\u{a0}2.".to_string(),
        ],
        "quote/list/code/math content must not also appear as paragraph nodes"
    );
}

#[test]
fn complex_semantic_all_node_types_and_xrefs() {
    let manifest = manifest_for_example("complex_semantic.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert_eq!(manifest.doc.title, "Hardcore semantic fixture — CND integration");
    assert!(manifest.doc.authors.len() >= 2);
    assert!(manifest.doc.keywords.iter().any(|k| k.contains("quote")));

    assert!(stats.headings >= 8);
    assert_eq!(stats.quotes, 2);
    assert!(stats.lists >= 2);
    assert_eq!(stats.code, 2);
    assert_eq!(stats.math, 2);
    assert_eq!(stats.tables, 3);
    assert_eq!(stats.figures, 1);
    assert_eq!(stats.paragraphs, 8);

    for label in [
        "ch-corpus",
        "quote-knuth",
        "eq-golden",
        "eq-sum",
        "tab-signals",
        "fig-grid-zones",
        "fig-diagram",
        "sec-xrefs",
        "ch-annex",
    ] {
        assert!(label_exists(&manifest.nodes, label), "missing label {label}");
    }

    let knuth = find_by_label(&manifest.nodes, "quote-knuth").expect("knuth quote");
    let CndNode::Quote(knuth) = knuth else {
        panic!("expected quote node");
    };
    assert_eq!(knuth.attribution.as_deref(), Some("Donald Knuth"));
    assert!(knuth.text.contains("Programs are meant to be read"));

    let grid = find_by_label(&manifest.nodes, "fig-grid-zones").expect("grid");
    let CndNode::Table(grid) = grid else {
        panic!("expected grid table node");
    };
    assert_eq!(grid.kind, TableKind::Grid);
    assert!(grid.caption.as_deref().is_some_and(|c| c.contains("Zone layout")));

    let signals = find_by_label(&manifest.nodes, "tab-signals").expect("signals");
    let CndNode::Table(signals) = signals else {
        panic!("expected table");
    };
    assert!(signals.base.refs_from.len() >= 2);

    let corpus = find_by_label(&manifest.nodes, "ch-corpus").expect("corpus");
    let CndNode::Heading(corpus) = corpus else {
        panic!("expected heading");
    };
    let overview = corpus
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.text.contains("Overview paragraph") => Some(p),
            _ => None,
        })
        .expect("overview paragraph");
    assert_eq!(
        overview.base.state_metadata.get("domain").and_then(|v| v.as_str()),
        Some("semantic")
    );
    assert_eq!(
        overview.base.state_metadata.get("revision").and_then(|v| v.as_str()),
        Some("hard-1")
    );

    let xrefs = find_by_label(&manifest.nodes, "sec-xrefs").expect("xrefs section");
    let CndNode::Heading(xrefs) = xrefs else {
        panic!("expected heading");
    };
    let closing = xrefs
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.text.contains("Closing paragraph") => Some(p),
            _ => None,
        })
        .expect("closing xref paragraph");
    assert!(closing.base.refs_to.len() >= 5);
    assert!(
        closing
            .base
            .refs_to
            .iter()
            .all(|reference| reference.label.is_some()),
        "refs_to should carry Typst labels: {:?}",
        closing.base.refs_to
    );
    assert!(
        closing
            .base
            .refs_to
            .iter()
            .any(|reference| reference.label.as_deref() == Some("eq-golden")),
        "expected @eq-golden in refs_to"
    );
    assert_eq!(
        closing.base.state_metadata.get("section").and_then(|v| v.as_str()),
        Some("xrefs")
    );

    let lists = find_lists(&manifest.nodes);
    let bullet = lists
        .iter()
        .find(|list| !list.ordered && list.items.iter().any(|item| !item.children.is_empty()))
        .expect("bullet list with nested children");
    assert_eq!(bullet.items.len(), 3);
    assert_eq!(bullet.items[1].children.len(), 2);
    assert!(bullet.items[1].children[1].text.contains("LI-B2"));

    let annex = find_by_label(&manifest.nodes, "ch-annex").expect("annex");
    let CndNode::Heading(annex) = annex else {
        panic!("expected heading");
    };
    let standalone = annex
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Table(t) if t.caption.is_none() => Some(t),
            _ => None,
        })
        .expect("standalone table without caption");
    assert!(standalone.cells.iter().any(|c| c.text.contains("complex_semantic")));

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn complex_semantic_no_duplicate_container_paragraphs() {
    let manifest = manifest_for_example("complex_semantic.typ");
    let paragraphs = paragraph_texts_in_order(&manifest.nodes);

    for leaked in [
        "Programs are meant to be read",
        "Root item Alpha",
        "Nested Beta-1",
        "Numbered one with tag",
        "fn checksum",
        "phi.alt",
    ] {
        assert!(
            paragraphs.iter().all(|p| !p.contains(leaked)),
            "paragraph leaked container content {leaked:?}: {paragraphs:?}"
        );
    }
}

#[test]
fn complex_hardcore_columns_semantics_and_reading_order() {
    let manifest = manifest_for_example("complex_hardcore.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&manifest.nodes, &mut stats);

    assert_eq!(manifest.doc.title, "Hardcore mixed layout + semantics");
    assert!(stats.pages.len() >= 2, "expected multipage layout: {:?}", stats.pages);
    assert_eq!(stats.quotes, 1);
    assert_eq!(stats.code, 1);
    assert_eq!(stats.math, 1);
    assert_eq!(stats.lists, 1);
    assert_eq!(stats.tables, 2);

    assert_tag_sequence(
        &manifest.nodes,
        &[
            "HC-0", "HC-L1", "HC-R1", "HC-R2", "HC-NL", "HC-IL", "HC-IR", "TAIL-1", "TAIL-3",
        ],
    );

    let cols = find_by_label(&manifest.nodes, "sec-cols-blocks").expect("cols section");
    let CndNode::Heading(cols) = cols else {
        panic!("expected heading");
    };
    let quote = cols
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Quote(q) if q.text.contains("Quote trapped") => Some(q),
            _ => None,
        })
        .expect("quote in column block");
    assert_eq!(
        quote.base.state_metadata.get("track").and_then(|v| v.as_str()),
        Some("left")
    );
    assert_eq!(quote.attribution.as_deref(), Some("Source L"));
    assert!(
        cols.children.iter().all(|n| match n {
            CndNode::Paragraph(p) => !p.text.contains("Quote trapped"),
            _ => true,
        }),
        "quote body must not duplicate as paragraph"
    );

    let grid = find_by_label(&manifest.nodes, "fig-hc-grid").expect("grid");
    let CndNode::Table(grid) = grid else {
        panic!("expected grid table");
    };
    assert_eq!(grid.kind, TableKind::Grid);

    let tail = find_by_label(&manifest.nodes, "sec-tail").expect("tail");
    let CndNode::Heading(tail) = tail else {
        panic!("expected heading");
    };
    let tail_para = tail
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.text.starts_with("[TAIL-1]") => Some(p),
            _ => None,
        })
        .expect("tail paragraph");
    assert!(
        tail_para.text.contains("Équation") && tail_para.text.contains("Tableau"),
        "tail paragraph should contain resolved reference text"
    );

    let energy = find_by_label(&manifest.nodes, "eq-hc-energy").expect("energy eq");
    let CndNode::Math(energy) = energy else {
        panic!("expected math");
    };
    assert!(
        !energy.base.refs_from.is_empty(),
        "equation should be referenced from tail paragraph"
    );

    assert_refs_resolve(&manifest.nodes);
}

#[test]
fn example_files_exist() {
    for name in [
        "minimal.typ",
        "structured.typ",
        "comprehensive.typ",
        "complex_multipage.typ",
        "complex_refs.typ",
        "complex_tables.typ",
        "complex_columns.typ",
        "complex_semantic.typ",
        "complex_hardcore.typ",
        "rich.typ",
        "newsletter/main.typ",
    ] {
        assert!(
            example_path(name).is_file(),
            "missing example file: {name}"
        );
    }
}
