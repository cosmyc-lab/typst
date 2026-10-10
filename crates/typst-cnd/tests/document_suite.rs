mod common;

use common::{
    NodeStats, all_example_files, assert_cnd_contract, assert_json_roundtrip,
    assert_labels_globally_unique, assert_pool_refs_resolve, assert_refs_resolve,
    assert_tag_sequence, assert_unique_ids, cnd_for_example, codepoint_slice,
    compile_example, example_path, find_by_label, find_lists, heading_texts,
    incoming_count, label_exists, paragraph_texts_in_order, table_stats,
    tags_under_heading, walk_nodes,
};
use typst_cnd::{CndNode, ListItem, TableKind, cnd_to_json};

/// `(bibliography key, citation form, supplement)` on one node.
type CiteTuple = (String, Option<String>, Option<String>);

/// `(node text, bibliography key, citation form, the marker's text span)`.
type CiteSpanTuple = (String, String, Option<String>, Option<Vec<i64>>);

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
            "{name} produced no nodes"
        );
    }
}

#[test]
fn all_examples_produce_valid_cnds() {
    for path in all_example_files() {
        let name = path.file_name().unwrap().to_string_lossy();
        let cnd = cnd_for_example(&name);
        assert_cnd_contract(&cnd);
        assert_unique_ids(&cnd.nodes);
        assert_labels_globally_unique(&cnd);
        assert_json_roundtrip(&cnd);
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
    let cnd = cnd_for_example("structured.typ");
    assert!(label_exists(&cnd.nodes, "tab-params-nominaux"));

    let figure = find_by_label(&cnd.nodes, "tab-params-nominaux").expect("figure");
    let CndNode::Figure(figure) = figure else {
        panic!("expected figure node");
    };
    assert_eq!(figure.caption.as_deref(), Some("Paramètres nominaux de fonctionnement."));
    assert!(figure.number.as_deref().is_some_and(|n| n.contains('1')));
    let table = figure
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("table child");
    assert!(
        table
            .raw
            .as_ref()
            .map(|r| r.value.as_str())
            .is_some_and(|r| r.contains("table("))
    );
    assert!(
        incoming_count(&cnd.nodes, figure.base.label.as_deref().expect("label")) > 0,
        "the captioned figure is a cross-reference target"
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn comprehensive_document_structure() {
    let cnd = cnd_for_example("comprehensive.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    let root_paragraphs = cnd
        .nodes
        .iter()
        .filter(|n| matches!(n, CndNode::Paragraph(_)))
        .count();
    assert_eq!(root_paragraphs, 1, "expected one pre-heading paragraph at document root");
    assert!(stats.paragraphs >= 8);
    assert!(stats.max_heading_level >= 3);
    assert_eq!(stats.tables, 4);

    let tables = table_stats(&cnd.nodes);
    assert_eq!(tables.with_caption, 4);
    assert_eq!(tables.with_fig_number, 4);
    assert_eq!(tables.with_label, 4);

    let arch =
        find_by_label(&cnd.nodes, "ch-architecture").expect("architecture heading");
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
    assert_eq!(
        para.base.state_metadata.get("revision").and_then(|v| v.as_str()),
        Some("4.2")
    );
    assert_eq!(
        para.base.state_metadata.get("status").and_then(|v| v.as_str()),
        Some("approved")
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_multipage_spans_pages() {
    let cnd = cnd_for_example("complex_multipage.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert!(
        stats.pages.len() >= 2,
        "expected content on multiple pages: {:?}",
        stats.pages
    );
    assert!(stats.headings >= 5);
    assert_eq!(stats.tables, 2);

    let f101 = find_by_label(&cnd.nodes, "sec-f101").expect("f101 heading");
    let CndNode::Heading(f101) = f101 else {
        panic!("expected heading");
    };
    let para = f101
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.base.state_metadata.contains_key("zone") => {
                Some(p)
            }
            _ => None,
        })
        .expect("paragraph with zone metadata");
    assert_eq!(
        para.base.state_metadata.get("criticality").and_then(|v| v.as_str()),
        Some("high")
    );

    let tables = table_stats(&cnd.nodes);
    assert_eq!(tables.with_caption, 2);
    assert_eq!(tables.with_fig_number, 2);

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_refs_dense_graph() {
    let cnd = cnd_for_example("complex_refs.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert_eq!(stats.tables, 3);
    assert!(label_exists(&cnd.nodes, "tab-signaux"));
    assert!(label_exists(&cnd.nodes, "tab-boucles"));
    assert!(label_exists(&cnd.nodes, "tab-recap"));

    let overview = find_by_label(&cnd.nodes, "ch-overview").expect("overview");
    let CndNode::Heading(overview) = overview else {
        panic!("expected heading");
    };
    assert!(
        !overview.base.refs.is_empty(),
        "overview heading should reference labelled targets"
    );

    let signaux = find_by_label(&cnd.nodes, "tab-signaux").expect("signaux");
    let CndNode::Figure(signaux) = signaux else {
        panic!("expected figure");
    };
    assert!(
        incoming_count(&cnd.nodes, signaux.base.label.as_deref().expect("label")) >= 2
    );

    let regulation = find_by_label(&cnd.nodes, "sec-regulation").expect("regulation");
    let CndNode::Heading(regulation) = regulation else {
        panic!("expected heading");
    };
    let regulation_para = regulation
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p)
                if p.base.state_metadata.get("domain").and_then(|v| v.as_str())
                    == Some("regulation") =>
            {
                Some(p)
            }
            _ => None,
        })
        .expect("regulation paragraph with metadata");
    assert!(!regulation_para.base.refs.is_empty());

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_tables_standalone_and_figure_variants() {
    let cnd = cnd_for_example("complex_tables.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert_eq!(stats.tables, 3);
    assert!(label_exists(&cnd.nodes, "tab-config-standalone"));
    assert!(label_exists(&cnd.nodes, "tab-plages-a"));
    assert!(label_exists(&cnd.nodes, "tab-plages-b"));

    let standalone =
        find_by_label(&cnd.nodes, "tab-config-standalone").expect("standalone");
    let CndNode::Table(_standalone) = standalone else {
        panic!("expected bare table (standalone is not figure-wrapped)");
    };

    let tables = table_stats(&cnd.nodes);
    assert_eq!(tables.with_caption, 2, "only figure tables have captions");
    assert_eq!(tables.with_fig_number, 2);
    assert_eq!(tables.with_label, 3);

    let plages_a = find_by_label(&cnd.nodes, "tab-plages-a").expect("plages a");
    let plages_b = find_by_label(&cnd.nodes, "tab-plages-b").expect("plages b");
    let CndNode::Figure(a) = plages_a else { panic!() };
    let CndNode::Figure(b) = plages_b else { panic!() };
    assert_ne!(a.base.id, b.base.id);
    assert_ne!(a.caption, b.caption);
    let a_table = a
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("table child a");
    let b_table = b
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("table child b");
    assert!(a_table.cells.iter().any(|c| c.text.contains("PT-A")));
    assert!(b_table.cells.iter().any(|c| c.text.contains("PT-C")));

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_columns_reading_order_flatten() {
    let cnd = cnd_for_example("complex_columns.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert_tag_sequence(
        &cnd.nodes,
        &[
            "INTRO", "L1", "L2", "M1", "M2", "R1", "OUT-L", "IN-L1", "IN-R1", "OUT-R",
            "PC1", "PC2", "PC3", "PC4", "PC5", "PC6", "PC7", "PC8", "POST",
        ],
    );

    assert_eq!(
        tags_under_heading(&cnd.nodes, "sec-three-cols"),
        vec!["L1", "L2", "M1", "M2", "R1"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        tags_under_heading(&cnd.nodes, "sec-nested-cols"),
        vec!["OUT-L", "IN-L1", "IN-R1", "OUT-R"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        tags_under_heading(&cnd.nodes, "ch-page-cols"),
        (1..=8).map(|i| format!("PC{i}")).collect::<Vec<_>>()
    );

    let three_cols = find_by_label(&cnd.nodes, "sec-three-cols").expect("three cols");
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

    let figure = find_by_label(&cnd.nodes, "tab-bus-col").expect("figure");
    let CndNode::Figure(figure) = figure else {
        panic!("expected figure");
    };
    assert!(figure.caption.as_ref().is_some_and(|c| c.contains("bus")));
    let table = figure
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("table child");
    assert_eq!(table.cells.len(), 6);

    let page_cols = find_by_label(&cnd.nodes, "ch-page-cols").expect("page cols");
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

    let post = find_by_label(&cnd.nodes, "ch-single")
        .and_then(|n| match n {
            CndNode::Heading(h) => h.children.iter().find_map(|c| match c {
                CndNode::Paragraph(p) if p.text.starts_with("[POST]") => Some(p),
                _ => None,
            }),
            _ => None,
        })
        .expect("post paragraph");
    assert!(!post.base.refs.is_empty());

    assert_refs_resolve(&cnd.nodes);
}

/// Real-world newsletter template using `@preview/dashing-dept-news`.
///
/// First compile needs network access to fetch the package from Typst Universe.
#[test]
fn newsletter_dashing_dept_news_template() {
    let cnd = cnd_for_example("newsletter/main.typ");
    assert_eq!(cnd.cnd_version, typst_cnd::CND_VERSION);
    assert!(cnd.source.as_ref().expect("source").hash.starts_with("sha256:"));
    assert_eq!(cnd.doc.title, "Chemistry Department");
    assert_unique_ids(&cnd.nodes);
    assert_json_roundtrip(&cnd);
    assert_refs_resolve(&cnd.nodes);

    assert_eq!(cnd.doc.title, "Chemistry Department");

    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert!(stats.headings >= 5);
    assert!(stats.paragraphs >= 10);

    let headings = heading_texts(&cnd.nodes);
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
        nodes
            .iter()
            .find(|n| matches!(n, CndNode::Heading(h) if h.text.contains(needle)))
    }

    let sixtus = find_heading(&cnd.nodes, "Sixtus Award").expect("sixtus heading");
    let CndNode::Heading(sixtus) = sixtus else { panic!() };
    let sixtus_paragraphs: Vec<&str> = sixtus
        .children
        .iter()
        .filter_map(|c| match c {
            CndNode::Paragraph(p) => Some(p.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        sixtus_paragraphs.len(),
        4,
        "award section has exactly four prose paragraphs, got {sixtus_paragraphs:?}"
    );
    // The quote's attribution and the figure's caption belong to the
    // QuoteNode and FigureNode respectively; they must not also surface as
    // paragraphs of the section.
    for leaked in ["Prof. Herzog", "Our new department rectangle"] {
        assert!(
            !sixtus_paragraphs.iter().any(|p| p.contains(leaked)),
            "container content {leaked:?} leaked into a paragraph: {sixtus_paragraphs:?}"
        );
    }
}

#[test]
fn rich_document_node_types() {
    let cnd = cnd_for_example("rich.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert!(stats.quotes >= 1, "expected at least one quote node");
    assert!(stats.lists >= 2, "expected bullet and numbered lists");
    assert!(stats.code >= 1, "expected a code block node");
    assert!(stats.math >= 1, "expected a block math node");
    // Every captioned float is now a figure wrapper, including the
    // grid-in-figure — so this counts the image figure plus the grid figure.
    assert!(stats.figures >= 1, "expected at least one figure node");
    assert!(stats.tables >= 1, "expected a grid-in-figure table node");

    let grid_fig = find_by_label(&cnd.nodes, "rich-grid").expect("grid figure");
    let CndNode::Figure(grid_fig) = grid_fig else {
        panic!("expected grid figure node");
    };
    assert!(grid_fig.caption.as_deref().is_some_and(|c| c.contains("grid")));
    let grid = grid_fig
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("grid table child");
    assert_eq!(grid.kind, typst_cnd::TableKind::Grid);

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn rich_document_no_duplicate_container_paragraphs() {
    let cnd = cnd_for_example("rich.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);

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
    let cnd = cnd_for_example("complex_semantic.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert_eq!(cnd.doc.title, "Hardcore semantic fixture — CND integration");
    assert!(cnd.doc.authors.len() >= 2);
    assert!(cnd.doc.keywords.iter().any(|k| k.contains("quote")));

    assert!(stats.headings >= 8);
    assert_eq!(stats.quotes, 2);
    assert!(stats.lists >= 2);
    assert_eq!(stats.code, 2);
    assert_eq!(stats.math, 2);
    assert_eq!(stats.tables, 3);
    // Three figure wrappers: the two captioned tables/grids (tab-signals,
    // fig-grid-zones) plus the one true non-table figure (fig-diagram).
    assert_eq!(stats.figures, 3);
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
        assert!(label_exists(&cnd.nodes, label), "missing label {label}");
    }

    let knuth = find_by_label(&cnd.nodes, "quote-knuth").expect("knuth quote");
    let CndNode::Quote(knuth) = knuth else {
        panic!("expected quote node");
    };
    assert_eq!(knuth.attribution.as_deref(), Some("Donald Knuth"));
    assert!(knuth.text.contains("Programs are meant to be read"));

    let grid_fig = find_by_label(&cnd.nodes, "fig-grid-zones").expect("grid");
    let CndNode::Figure(grid_fig) = grid_fig else {
        panic!("expected grid figure node");
    };
    assert!(grid_fig.caption.as_deref().is_some_and(|c| c.contains("Zone layout")));
    let grid = grid_fig
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("grid table child");
    assert_eq!(grid.kind, TableKind::Grid);

    let signals = find_by_label(&cnd.nodes, "tab-signals").expect("signals");
    let CndNode::Figure(signals) = signals else {
        panic!("expected figure");
    };
    assert!(
        incoming_count(&cnd.nodes, signals.base.label.as_deref().expect("label")) >= 2
    );

    let corpus = find_by_label(&cnd.nodes, "ch-corpus").expect("corpus");
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

    let xrefs = find_by_label(&cnd.nodes, "sec-xrefs").expect("xrefs section");
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
    assert!(closing.base.refs.len() >= 5);
    // A label is now the edge itself, not an optional decoration on it —
    // an unlabelled edge cannot be represented (ADR 0017).
    assert!(
        closing.base.refs.iter().all(|reference| !reference.label.is_empty()),
        "refs should carry Typst labels: {:?}",
        closing.base.refs
    );
    assert!(
        closing
            .base
            .refs
            .iter()
            .any(|reference| reference.label == "eq-golden"),
        "expected @eq-golden in refs"
    );
    assert_eq!(
        closing.base.state_metadata.get("section").and_then(|v| v.as_str()),
        Some("xrefs")
    );

    let lists = find_lists(&cnd.nodes);
    let bullet = lists
        .iter()
        .find(|list| {
            !list.ordered && list.items.iter().any(|item| !item.children.is_empty())
        })
        .expect("bullet list with nested children");
    assert_eq!(bullet.items.len(), 3);
    assert_eq!(bullet.items[1].children.len(), 2);
    assert!(bullet.items[1].children[1].text.contains("LI-B2"));

    let annex = find_by_label(&cnd.nodes, "ch-annex").expect("annex");
    let CndNode::Heading(annex) = annex else {
        panic!("expected heading");
    };
    let standalone = annex
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("standalone bare table");
    assert!(standalone.cells.iter().any(|c| c.text.contains("complex_semantic")));

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_semantic_no_duplicate_container_paragraphs() {
    let cnd = cnd_for_example("complex_semantic.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);

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
    let cnd = cnd_for_example("complex_hardcore.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    assert_eq!(cnd.doc.title, "Hardcore mixed layout + semantics");
    assert!(stats.pages.len() >= 2, "expected multipage layout: {:?}", stats.pages);
    assert_eq!(stats.quotes, 1);
    assert_eq!(stats.code, 1);
    assert_eq!(stats.math, 1);
    assert_eq!(stats.lists, 1);
    assert_eq!(stats.tables, 2);

    assert_tag_sequence(
        &cnd.nodes,
        &[
            "HC-0", "HC-L1", "HC-R1", "HC-R2", "HC-NL", "HC-IL", "HC-IR", "TAIL-1",
            "TAIL-3",
        ],
    );

    let cols = find_by_label(&cnd.nodes, "sec-cols-blocks").expect("cols section");
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

    let grid_fig = find_by_label(&cnd.nodes, "fig-hc-grid").expect("grid");
    let CndNode::Figure(grid_fig) = grid_fig else {
        panic!("expected grid figure");
    };
    let grid = grid_fig
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("grid table child");
    assert_eq!(grid.kind, TableKind::Grid);

    let tail = find_by_label(&cnd.nodes, "sec-tail").expect("tail");
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

    let energy = find_by_label(&cnd.nodes, "eq-hc-energy").expect("energy eq");
    let CndNode::Math(energy) = energy else {
        panic!("expected math");
    };
    assert!(
        incoming_count(&cnd.nodes, energy.base.label.as_deref().expect("label")) > 0,
        "equation should be referenced from tail paragraph"
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn inline_code_not_duplicated_in_paragraphs() {
    let cnd = cnd_for_example("inline_code.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);

    for text in &paragraphs {
        for token in &["typst-cnd", "extract_text", "plain_text", "foo"] {
            let count = text.matches(token).count();
            assert!(
                count <= 1,
                "inline code token {token:?} appears {count}× in paragraph (expected ≤1): {text:?}"
            );
        }
    }

    let lists = find_lists(&cnd.nodes);
    assert!(!lists.is_empty(), "inline_code.typ should produce at least one list");
    for list in lists {
        for item in &list.items {
            for token in &["typst-cnd", "cargo test", "inline code"] {
                let count = item.text.matches(token).count();
                assert!(
                    count <= 1,
                    "inline code token {token:?} appears {count}× in list item (expected ≤1): {:?}",
                    item.text
                );
            }
        }
    }

    let mut quote_texts: Vec<String> = Vec::new();
    fn collect_quote_texts(nodes: &[CndNode], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                CndNode::Quote(q) => out.push(q.text.clone()),
                CndNode::Heading(h) => collect_quote_texts(&h.children, out),
                _ => {}
            }
        }
    }
    collect_quote_texts(&cnd.nodes, &mut quote_texts);
    assert!(!quote_texts.is_empty(), "inline_code.typ should produce at least one quote");
    for text in &quote_texts {
        let count = text.matches("extract_text").count();
        assert!(
            count <= 1,
            "inline code token 'extract_text' appears {count}× in quote (expected ≤1): {text:?}"
        );
    }
}

#[test]
fn terms_definition_lists() {
    let cnd = cnd_for_example("terms.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);

    // Two definition lists, and the list nested in a term description must
    // NOT also surface as a standalone list node (proposal 0004 dedup).
    assert_eq!(stats.terms, 2);
    assert_eq!(stats.lists, 0, "nested list must inline into the term, not emit a node");

    let mut terms: Vec<&CndNode> = Vec::new();
    fn collect<'a>(nodes: &'a [CndNode], out: &mut Vec<&'a CndNode>) {
        for node in nodes {
            if matches!(node, CndNode::Terms(_)) {
                out.push(node);
            }
            if let CndNode::Heading(h) = node {
                collect(&h.children, out);
            }
        }
    }
    collect(&cnd.nodes, &mut terms);
    assert_eq!(terms.len(), 2);

    let CndNode::Terms(tight) = terms[0] else { panic!() };
    assert!(tight.tight, "first list is a tight definition list");
    assert_eq!(tight.items.len(), 3);
    assert_eq!(tight.items[0].term, "manifest");
    assert_eq!(tight.items[0].description, "The serialized document tree.");

    let CndNode::Terms(wide) = terms[1] else { panic!() };
    assert!(!wide.tight, "second list is a wide definition list");
    assert_eq!(wide.items.len(), 3);
    let consumer = &wide.items[1];
    assert_eq!(consumer.term, "consumer");
    assert!(
        consumer.description.contains("the search indexer")
            && consumer.description.contains("export tooling"),
        "nested list content is inlined into the term description: {:?}",
        consumer.description
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn image_figure_carries_path_and_alt() {
    let cnd = cnd_for_example("image_figure.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert_eq!(stats.images, 1, "the captioned image is an ImageNode child");

    let figure = find_by_label(&cnd.nodes, "fig-cover").expect("figure");
    let CndNode::Figure(figure) = figure else {
        panic!("expected figure wrapper");
    };
    assert_eq!(figure.kind.as_deref(), Some("image"));
    assert!(figure.caption.as_deref().is_some_and(|c| c.contains("cover art")));

    let image = figure
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Image(img) => Some(img),
            _ => None,
        })
        .expect("image child");
    assert!(
        image
            .path
            .as_deref()
            .is_some_and(|p| p.contains("newsletter-cover.png")),
        "image path preserved: {:?}",
        image.path
    );
    assert_eq!(image.alt.as_deref(), Some("Department cover art"));

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn bare_image_emits_a_standalone_image_node() {
    // "A bare image outside any figure is an ImageNode with no wrapper"
    // (the CND model's own words) — not silence. Regression: the emitter
    // used to extract images only inside `#figure`, so a document whose
    // only visual was a bare `#image(...)` indexed with no image node at
    // all, and nothing downstream could ever find or show it.
    let cnd = cnd_for_example("bare_image.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert_eq!(stats.images, 1, "the bare image is a standalone ImageNode");
    assert_eq!(stats.figures, 0, "no figure wrapper is invented for it");
    assert_eq!(stats.paragraphs, 2, "the two authored paragraphs survive");

    fn find_image(nodes: &[CndNode]) -> Option<&typst_cnd::ImageNode> {
        nodes.iter().find_map(|n| match n {
            CndNode::Image(img) => Some(img),
            CndNode::Heading(h) => find_image(&h.children),
            _ => None,
        })
    }
    let image = find_image(&cnd.nodes).expect("standalone image node under the heading");
    assert!(
        image
            .path
            .as_deref()
            .is_some_and(|p| p.contains("newsletter-cover.png")),
        "image path preserved: {:?}",
        image.path
    );
    assert_eq!(image.alt.as_deref(), Some("Cover art, bare"));
}

#[test]
fn figure_wrapped_image_is_not_also_emitted_bare() {
    // The bare-image collection must not double-emit an image that a
    // figure already carries as its child.
    let cnd = cnd_for_example("image_figure.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert_eq!(stats.images, 1, "exactly the figure's child, no bare twin");
    assert_eq!(stats.figures, 1);
}

#[test]
fn footnotes_pool_and_edges() {
    let cnd = cnd_for_example("footnotes.typ");

    // Three declaration footnotes become pool entries; the labelled note is
    // referenced twice (its declaration plus a `#footnote(<label>)` marker).
    let labels: Vec<&str> = cnd.footnotes.iter().map(|f| f.label.as_str()).collect();
    assert_eq!(labels, vec!["1", "2", "3"]);
    assert!(
        cnd.footnotes.iter().any(|f| f.text.contains("2023 audit")),
        "footnote body text is captured in the pool"
    );

    assert_pool_refs_resolve(&cnd);

    // Collect footnote edges per paragraph in reading order.
    let mut edges: Vec<Vec<String>> = Vec::new();
    fn walk(nodes: &[CndNode], out: &mut Vec<Vec<String>>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => {
                    out.push(p.base.footnotes.iter().map(|r| r.label.clone()).collect());
                }
                CndNode::Heading(h) => walk(&h.children, out),
                _ => {}
            }
        }
    }
    walk(&cnd.nodes, &mut edges);
    // The middle paragraph carries two markers (note 2 and the re-reference
    // to note 3); the last paragraph declares note 3.
    assert!(
        edges.iter().any(|e| e.len() == 2
            && e.contains(&"2".to_string())
            && e.contains(&"3".to_string())),
        "a paragraph should carry both note 2 and the re-referenced note 3: {edges:?}"
    );
    assert!(edges.iter().any(|e| e == &vec!["1".to_string()]));

    // Text spans (ADR 0013): a footnote marker renders its ordinal digit
    // into the node text, so its span is 1 code point wide over that digit.
    fn footnote_spans(
        nodes: &[CndNode],
        out: &mut Vec<(String, String, Option<Vec<i64>>)>,
    ) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => {
                    for reference in &p.base.footnotes {
                        out.push((
                            p.text.clone(),
                            reference.label.clone(),
                            reference.text_span.clone(),
                        ));
                    }
                }
                CndNode::Heading(h) => footnote_spans(&h.children, out),
                _ => {}
            }
        }
    }
    let mut spans = Vec::new();
    footnote_spans(&cnd.nodes, &mut spans);
    for (text, label, span) in &spans {
        let span = span.as_ref().expect("footnote in a paragraph carries a text span");
        assert_eq!(span[1] - span[0], 1, "footnote marker span is 1 code point wide");
        assert_eq!(
            &codepoint_slice(text, span),
            label,
            "the footnote span covers its rendered ordinal digit"
        );
    }
    // The re-referenced note 3 appears with a span in two distinct nodes.
    assert!(
        spans.iter().filter(|(_, label, _)| label == "3").count() >= 2,
        "note 3 is positioned in both the declaring and re-referencing paragraphs"
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn citations_bibliography_pool_and_cite_edges() {
    let cnd = cnd_for_example("citations.typ");

    // Bibliography pool: two cited works with the curated typed subset.
    let smith = cnd
        .bibliography
        .iter()
        .find(|b| b.label == "smith2024")
        .expect("smith2024 bib entry");
    assert_eq!(smith.type_.as_deref(), Some("article"));
    assert_eq!(smith.year, Some(2024));
    assert!(smith.authors.iter().any(|a| a.contains("Smith")));
    assert_eq!(smith.doi.as_deref(), Some("10.1000/jods.2024.12"));
    assert!(
        smith.formatted.as_deref().is_some_and(|f| !f.is_empty()),
        "formatted reference string present"
    );
    assert!(smith.fields.get("title").is_some(), "fields carries the full source entry");

    assert!(cnd.bibliography.iter().any(|b| b.label == "jones2022"));

    assert_pool_refs_resolve(&cnd);

    // Collect cite edges per paragraph.
    let mut cites: Vec<Vec<CiteTuple>> = Vec::new();
    fn walk(nodes: &[CndNode], out: &mut Vec<Vec<CiteTuple>>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => out.push(
                    p.base
                        .cites
                        .iter()
                        .map(|c| (c.label.clone(), c.form.clone(), c.supplement.clone()))
                        .collect(),
                ),
                CndNode::Heading(h) => walk(&h.children, out),
                _ => {}
            }
        }
    }
    walk(&cnd.nodes, &mut cites);
    let all: Vec<_> = cites.iter().flatten().collect();

    // Supplement, prose form, and suppressed (form: none) citation captured.
    assert!(
        all.iter().any(|(k, f, s)| k == "smith2024"
            && f.as_deref() == Some("normal")
            && s.as_deref() == Some("p. 104")),
        "page-specific supplement captured: {all:?}"
    );
    assert!(
        all.iter()
            .any(|(k, f, _)| k == "jones2022" && f.as_deref() == Some("prose"))
    );
    assert!(
        all.iter()
            .any(|(k, f, _)| k == "jones2022" && f.as_deref() == Some("none"))
    );

    // Text spans (ADR 0013): the rendered citation marker slices out of the
    // node text; a suppressed (form: none) citation has no marker, no span.
    let mut spans: Vec<CiteSpanTuple> = Vec::new();
    fn cite_spans(nodes: &[CndNode], out: &mut Vec<CiteSpanTuple>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => {
                    for cite in &p.base.cites {
                        out.push((
                            p.text.clone(),
                            cite.label.clone(),
                            cite.form.clone(),
                            cite.text_span.clone(),
                        ));
                    }
                }
                CndNode::Heading(h) => cite_spans(&h.children, out),
                _ => {}
            }
        }
    }
    cite_spans(&cnd.nodes, &mut spans);
    assert!(
        spans.iter().any(|(text, k, f, sp)| k == "smith2024"
            && f.as_deref() == Some("normal")
            && sp.as_ref().is_some_and(|s| codepoint_slice(text, s) == "[1]")),
        "the normal smith2024 citation's text_span slices to \"[1]\": {spans:?}"
    );
    assert!(
        spans.iter().any(|(_, k, f, sp)| k == "jones2022"
            && f.as_deref() == Some("none")
            && sp.is_none()),
        "a suppressed (form: none) citation has a null text_span"
    );

    // A `@key` citation is a RefElem but must not create a cross-reference
    // edge — citations resolve in the bibliography, not the node tree.
    fn assert_no_bibkey_refs(nodes: &[CndNode]) {
        for node in nodes {
            for reference in &node.base().refs {
                let label = reference.label.as_str();
                assert!(
                    label != "smith2024" && label != "jones2022",
                    "citation key leaked into refs: {label}"
                );
            }
            match node {
                CndNode::Heading(h) => assert_no_bibkey_refs(&h.children),
                CndNode::Figure(f) => assert_no_bibkey_refs(&f.children),
                _ => {}
            }
        }
    }
    assert_no_bibkey_refs(&cnd.nodes);

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn ref_text_spans_are_codepoint_offsets() {
    let cnd = cnd_for_example("complex_refs.typ");

    // Collect (node text, ref label, span) for every positioned ref.
    let mut spans: Vec<(String, String, Vec<i64>)> = Vec::new();
    fn walk(nodes: &[CndNode], out: &mut Vec<(String, String, Vec<i64>)>) {
        for node in nodes {
            if let CndNode::Paragraph(p) = node {
                for reference in &p.base.refs {
                    if let Some(span) = &reference.text_span {
                        out.push((p.text.clone(), reference.label.clone(), span.clone()));
                    }
                }
            }
            if let CndNode::Heading(h) = node {
                walk(&h.children, out);
            }
        }
    }
    walk(&cnd.nodes, &mut spans);
    assert!(!spans.is_empty(), "cross-reference markers should carry text spans");

    // Every span slices to the rendered reference text, and the `\u{a0}`
    // inside "Section 11" / "Tableau 1" is the codepoint-vs-byte canary: a
    // byte offset would land mid-character here.
    //
    // The supplement is whatever `translations/fr.txt` gives for `heading`
    // (upstream #8728 changed it from "Chapitre" to "Section"). If this
    // assertion fails on a word, check that file before suspecting the
    // offsets — the invariant under test is the offset arithmetic, not the
    // wording.
    let mut saw_numbered_heading_ref = false;
    for (text, label, span) in &spans {
        let slice = codepoint_slice(text, span);
        assert!(
            slice.contains('\u{a0}'),
            "ref {label} span {span:?} slices to a numbered reference: {slice:?}"
        );
        if label == "sec-detail" && slice == "Section\u{a0}11" {
            saw_numbered_heading_ref = true;
        }
    }
    assert!(
        saw_numbered_heading_ref,
        "expected a ref rendering \"Section\\u{{a0}}11\" (10 code points, 11 bytes): {spans:?}"
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn markers_in_non_flat_nodes_do_not_leak_or_span() {
    let cnd = cnd_for_example("nonflat_markers.typ");

    // The footnote still enters the pool even though it sits in a list item.
    assert_eq!(cnd.footnotes.len(), 1);
    assert!(cnd.footnotes[0].text.contains("inside a list item"));

    // Leak guard: the footnote body must not bleed into the list item text
    // (the item body is text-extracted unrealized; ADR 0013 leak fix).
    let lists = find_lists(&cnd.nodes);
    let list = lists.first().expect("a bullet list");
    let carrier = list
        .items
        .iter()
        .find(|item| item.text.contains("carrying a footnote"))
        .expect("the footnote-carrying item");
    assert_eq!(
        carrier.text, "An item carrying a footnote.",
        "the footnote body must not leak into the list item text"
    );

    assert_pool_refs_resolve(&cnd);
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
        "terms.typ",
        "image_figure.typ",
        "bare_image.typ",
        "footnotes.typ",
        "citations.typ",
        "nonflat_markers.typ",
        "newsletter/main.typ",
        "paragraph_identity.typ",
        "nested_lists.typ",
        "list_in_float_long.typ",
        "list_in_float_short.typ",
        "list_in_bare_float.typ",
        "list_in_bare_float_block.typ",
        "par_in_float_beside_list.typ",
        "float_in_list_item.typ",
        "list_in_footnote_item.typ",
        "list_in_footnote_par.typ",
        "box_identity.typ",
        "_box_identity_lib.typ",
        "box_float_text.typ",
        "_box_float_text_lib.typ",
        "float_scope_let.typ",
        "float_scope_helper.typ",
        "float_holds_by_range.typ",
        "float_holds_by_range_quote.typ",
        "_float_holds_lib.typ",
        "_float_holds_chapter.typ",
        "float_held_by_text.typ",
        "_float_text_chapter.typ",
        "float_held_by_text_detached.typ",
        "float_held_by_text_whole.typ",
        "float_held_by_text_prefix.typ",
        "float_held_by_text_reused.typ",
        "handbuilt_toc.typ",
        "figure_bodies.typ",
        "terms_and_paragraphs.typ",
        "quote_with_list.typ",
        "figure_raw.typ",
        "figure_helper_drawing.typ",
    ] {
        assert!(example_path(name).is_file(), "missing example file: {name}");
    }
}

#[test]
fn heading_show_rules_do_not_duplicate_headings_as_paragraphs() {
    // A show rule that re-emits `it.body` inside fresh markup realizes an
    // extra paragraph carrying the heading's own text; the emitter must
    // keep exactly the author's paragraphs.
    let cnd = cnd_for_example("heading_show_rules.typ");
    let headings = heading_texts(&cnd.nodes);
    assert_eq!(
        headings,
        vec!["1. Overview", "1.1 Details", "2. Operations"],
        "headings should survive custom show rules"
    );
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    assert_eq!(
        paragraphs.len(),
        3,
        "exactly the three authored paragraphs, got: {paragraphs:?}"
    );
    for heading in &headings {
        assert!(
            !paragraphs.iter().any(|p| p == heading),
            "heading {heading:?} duplicated as a paragraph: {paragraphs:?}"
        );
    }
}

/// Paragraphs are deduplicated by element identity: a paragraph that merely
/// starts like a heading, a list item or a callout title is the author's own
/// and must survive, and inline PDF artifacts never reach a paragraph's text.
#[test]
fn paragraph_identity_dedupe_and_inline_artifacts() {
    let cnd = cnd_for_example("paragraph_identity.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    for expected in [
        "Decision taken by the team, without a spec.",
        "First item of a list that is long enough, followed by more text.",
        "A",
        "Decision We keep option 3.",
        "Before after.",
        "Paragraph after the artifact.",
        "Lead words trailing words.",
    ] {
        let count = paragraphs.iter().filter(|p| p.as_str() == expected).count();
        assert_eq!(
            count, 1,
            "paragraph {expected:?} must appear exactly once, got: {paragraphs:?}"
        );
    }
    assert!(
        !paragraphs.iter().any(|p| p.contains("decoration")),
        "artifact-only text leaked into a paragraph: {paragraphs:?}"
    );
    assert!(
        !paragraphs.iter().any(|p| p.contains('\u{2605}')),
        "inline artifact glyph leaked into a paragraph: {paragraphs:?}"
    );
    assert_eq!(
        paragraphs.len(),
        7,
        "exactly the seven authored paragraphs, got: {paragraphs:?}"
    );
}

/// Text carried by a layout-only container — a `block`, a grid cell, a
/// `place`, a user function's output — must reach the CND.
///
/// Typst does not build a `ParElem` for a fragment body that is entirely
/// inline, so such text is laid out but never located, and the emit pipeline
/// (which reads `Introspector::query`) cannot see it. `Feature::CndSemantics`
/// forces the paragraph to exist; `emit::ancestry` then keeps the extra
/// paragraphs that creates from duplicating text that is already emitted.
#[test]
fn layout_container_content_is_not_lost() {
    let cnd = cnd_for_example("layout_containers.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    let joined = paragraphs.join("\n");

    for expected in [
        // `block(..)[..]` emitted by a user function, both of whose blocks
        // hold nothing but inline content.
        "KICKER LINE",
        "Title inside a block",
        // A card: the title, then one block per item, inside a grid cell.
        "LEFT CARD",
        "Left item one",
        "Left item two",
        "RIGHT CARD",
        "Right item one",
        "Right item two",
        // A `place`d block.
        "Placed punch line at the bottom.",
    ] {
        assert!(
            paragraphs.iter().any(|p| p.contains(expected)),
            "container content {expected:?} missing from the CND, got {paragraphs:?}"
        );
    }

    // An inline `box` is part of its enclosing paragraph, which already
    // renders its text — it must not also become a paragraph of its own.
    for chip in ["FIRST CHIP", "SECOND CHIP"] {
        assert_eq!(
            joined.matches(chip).count(),
            1,
            "inline box content {chip:?} was emitted more than once: {paragraphs:?}"
        );
    }

    // The box is dropped because it is written inside this paragraph, not
    // because its text matches — a word the sentence uses in its own right
    // must survive on both sides of the box.
    assert!(
        paragraphs.iter().any(|p| p
            == "A paragraph where idem repeats a word the sentence itself uses: idem."),
        "a box whose text recurs in its own sentence lost content, got {paragraphs:?}"
    );

    // A running footer is page furniture, repeated on every page.
    assert!(
        !joined.contains("RUNNING FOOTER"),
        "page furniture leaked into the document body: {paragraphs:?}"
    );

    // A hard line break carries no text of its own; without a separator the
    // words on either side of it run together.
    assert!(
        paragraphs.iter().any(|p| p.contains("first line second line")),
        "hard line break did not separate the lines, got {paragraphs:?}"
    );
}

#[test]
fn inline_boxes_are_absorbed_and_floats_are_not() {
    let cnd = cnd_for_example("inline_boxes_and_floats.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    let joined = paragraphs.join("\n");

    // The body of an inline `box` is realized into a paragraph of its own,
    // which the paragraph holding the box already renders.
    assert_eq!(
        paragraphs.iter().filter(|p| p.contains("boxed")).count(),
        1,
        "an inline box became a paragraph of its own: {paragraphs:?}"
    );
    assert!(
        !joined.contains("see Section"),
        "a box holding a cross-reference was emitted twice: {paragraphs:?}"
    );

    // A float written inside a paragraph is deferred to wherever it fits,
    // and its tags can land inside a *later* paragraph's still-open range.
    // It is not part of that paragraph and must survive.
    assert_eq!(
        paragraphs.iter().filter(|p| p.contains("FLOATED BODY TEXT")).count(),
        1,
        "the float body was dropped or duplicated: {paragraphs:?}"
    );

    // Page furniture. Upstream marks the header, the footer and the
    // background as artifacts but not the foreground; without that the stamp
    // is one paragraph per page.
    assert!(
        !joined.contains("DRAFT STAMP"),
        "the page foreground leaked into the document: {paragraphs:?}"
    );

    // A rendered bibliography entry: its text belongs to the pool, and with
    // `title: none` no generated heading's source range hides it.
    assert!(
        !joined.contains("Context-native pipelines"),
        "a bibliography entry leaked as prose: {paragraphs:?}"
    );
    assert!(
        cnd.bibliography.iter().any(|entry| entry.label == "smith2024"),
        "the bibliography pool lost its entry"
    );

    // Dropping the box's paragraph must not drop the edges only it carries:
    // the enclosing paragraph holds the box body *unrealized*, so the
    // footnote and citation tags exist nowhere else.
    let edges = paragraph_edges(&cnd.nodes, "A sentence with");
    assert!(
        edges.footnotes.contains(&"1".to_string()),
        "the footnote written inside a box lost its edge: {edges:?}"
    );
    assert!(
        edges.cites.contains(&"smith2024".to_string()),
        "the citation written inside a box lost its edge: {edges:?}"
    );
    assert!(
        edges.refs.contains(&"sec".to_string()),
        "the reference written inside a box lost its edge: {edges:?}"
    );
    assert_eq!(
        edges.refs.len(),
        1,
        "absorbing the box's reference duplicated an edge the host already had: {edges:?}"
    );

    assert_cnd_contract(&cnd);
    assert_pool_refs_resolve(&cnd);
}

#[derive(Debug, Default)]
struct ParagraphEdges {
    refs: Vec<String>,
    cites: Vec<String>,
    footnotes: Vec<String>,
}

/// The out-of-tree edges of the first paragraph whose text contains `needle`.
fn paragraph_edges(nodes: &[CndNode], needle: &str) -> ParagraphEdges {
    fn walk(nodes: &[CndNode], needle: &str, out: &mut Option<ParagraphEdges>) {
        for node in nodes {
            if out.is_some() {
                return;
            }
            if let CndNode::Paragraph(p) = node
                && p.text.contains(needle)
            {
                *out = Some(ParagraphEdges {
                    refs: p.base.refs.iter().map(|r| r.label.clone()).collect(),
                    cites: p.base.cites.iter().map(|c| c.label.clone()).collect(),
                    footnotes: p.base.footnotes.iter().map(|f| f.label.clone()).collect(),
                });
                return;
            }
            match node {
                CndNode::Heading(h) => walk(&h.children, needle, out),
                CndNode::Figure(f) => walk(&f.children, needle, out),
                _ => {}
            }
        }
    }
    let mut out = None;
    walk(nodes, needle, &mut out);
    out.unwrap_or_default()
}

/// The first paragraph whose text contains `needle`, anywhere in the tree.
fn find_paragraph<'a>(nodes: &'a [CndNode], needle: &str) -> Option<&'a CndNode> {
    for node in nodes {
        if let CndNode::Paragraph(p) = node
            && p.text.contains(needle)
        {
            return Some(node);
        }
        match node {
            CndNode::Heading(h) => {
                if let Some(found) = find_paragraph(&h.children, needle) {
                    return Some(found);
                }
            }
            CndNode::Figure(f) => {
                if let Some(found) = find_paragraph(&f.children, needle) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// `#link` capture (ADR 0024): the three-row mapping (URL -> `links`,
/// label/location -> `refs`, position -> dropped), and the null-span rules
/// for a non-textual body and a non-flat (list item) node.
#[test]
fn link_capture_maps_dest_kinds_and_does_not_disturb_ref_spans() {
    let cnd = cnd_for_example("links.typ");

    // Nested case: a link whose body begins with a ref to the very heading
    // it sits under. `LinkElem` capture goes through `open_frame`, keyed on
    // the `LinkElem`'s own `Location` for both open and close (like
    // cite/footnote) — never through the ref/`LinkMarker` pending-position
    // mechanism, so there is no shared state for the two to collide over.
    // This guards against a future change re-routing link capture through
    // that mechanism: it would assert one `links` entry whose span contains
    // the `refs` entry's span, both non-null, exactly as today.
    let nested = find_paragraph(&cnd.nodes, "and more text")
        .expect("the nested link+ref paragraph");
    let CndNode::Paragraph(nested) = nested else { unreachable!() };

    let link = nested
        .base
        .links
        .iter()
        .find(|l| l.href == "https://example.com")
        .unwrap_or_else(|| panic!("expected a links entry in {:?}", nested.base.links));
    let link_span = link
        .text_span
        .as_ref()
        .unwrap_or_else(|| panic!("expected a non-null span for {link:?}"));

    let reference = nested
        .base
        .refs
        .iter()
        .find(|r| r.label == "sec-overview")
        .unwrap_or_else(|| panic!("expected a refs entry in {:?}", nested.base.refs));
    let ref_span = reference
        .text_span
        .as_ref()
        .unwrap_or_else(|| panic!("expected a non-null span for {reference:?}"));

    assert!(
        link_span[0] <= ref_span[0] && ref_span[1] <= link_span[1],
        "the link's span {link_span:?} must contain the ref's span {ref_span:?}"
    );
    let link_slice = codepoint_slice(&nested.text, link_span);
    let ref_slice = codepoint_slice(&nested.text, ref_span);
    assert!(
        link_slice.contains(&ref_slice),
        "the link's rendered text {link_slice:?} must contain the ref's {ref_slice:?}"
    );
    assert!(
        link_slice.contains("and more text"),
        "the link's span must cover the whole body, got {link_slice:?}"
    );

    // Row 1: a plain URL link — `links` entry, href verbatim, span slicing
    // to the rendered (auto) body text.
    let plain =
        find_paragraph(&cnd.nodes, "in running text").expect("the plain-url paragraph");
    let CndNode::Paragraph(plain) = plain else { unreachable!() };
    let plain_link = plain
        .base
        .links
        .iter()
        .find(|l| l.href == "https://example.com/docs")
        .unwrap_or_else(|| panic!("expected a links entry in {:?}", plain.base.links));
    let plain_span = plain_link.text_span.as_ref().expect("non-null span");
    assert_eq!(codepoint_slice(&plain.text, plain_span), "https://example.com/docs");

    // Row 2: `#link(<label>)[body]` is a cross-reference with a custom
    // body — a `refs` entry, not a `links` entry.
    let labelled = find_paragraph(&cnd.nodes, "see the overview")
        .expect("the link-to-label paragraph");
    let CndNode::Paragraph(labelled) = labelled else { unreachable!() };
    let labelled_ref = labelled
        .base
        .refs
        .iter()
        .find(|r| r.label == "sec-overview")
        .unwrap_or_else(|| panic!("expected a refs entry in {:?}", labelled.base.refs));
    assert_eq!(
        codepoint_slice(&labelled.text, labelled_ref.text_span.as_ref().unwrap()),
        "see the overview"
    );
    assert!(
        labelled.base.links.is_empty(),
        "a link-to-label must not also surface as a links entry: {:?}",
        labelled.base.links
    );

    // Non-textual body (an empty-bodied link): a `links` entry with a null
    // span — the marker opens and closes with no rendered text in between,
    // not a degenerate `[n, n]`.
    let empty_body_paragraph = find_paragraph(&cnd.nodes, "carries no rendered text")
        .expect("the empty-bodied-link paragraph");
    let CndNode::Paragraph(empty_body_paragraph) = empty_body_paragraph else {
        unreachable!()
    };
    let empty_body_link = empty_body_paragraph
        .base
        .links
        .iter()
        .find(|l| l.href == "https://example.com/cover")
        .unwrap_or_else(|| {
            panic!("expected a links entry in {:?}", empty_body_paragraph.base.links)
        });
    assert!(
        empty_body_link.text_span.is_none(),
        "an empty-bodied link renders no text: expected a null span, got {:?}",
        empty_body_link.text_span
    );

    // Row 3: a page/point position has no label and is not durable — no
    // `links` entry, no `refs` entry.
    let position_link_paragraph =
        find_paragraph(&cnd.nodes, "Go to top").expect("the position-link paragraph");
    let CndNode::Paragraph(position_link_paragraph) = position_link_paragraph else {
        unreachable!()
    };
    assert!(
        position_link_paragraph.base.links.is_empty(),
        "a position destination must be dropped, not emitted as a links entry: {:?}",
        position_link_paragraph.base.links
    );
    assert!(
        position_link_paragraph.base.refs.is_empty(),
        "a position destination must be dropped, not emitted as a refs entry: {:?}",
        position_link_paragraph.base.refs
    );

    // Non-flat node (a list item): the span fallback rule ("nodes whose
    // text is not a single flat string emit `text_span: None`") now also
    // applies to `links` — generalised from the same rule already in force
    // for `refs` (refs.rs).
    let lists = find_lists(&cnd.nodes);
    let list = lists.first().expect("the bullet list");
    let list_link = list
        .base
        .links
        .iter()
        .find(|l| l.href == "https://example.com/list")
        .unwrap_or_else(|| panic!("expected a links entry in {:?}", list.base.links));
    assert!(
        list_link.text_span.is_none(),
        "a link inside a non-flat node must carry a null span, got {:?}",
        list_link.text_span
    );

    // A paragraph-initial body that splits into two paragraphs (Critical 1's
    // finding): the link opens the block with no leading text before it, so
    // its `Tag::Start` sits before any paragraph group's trigger range and
    // both tags stay in the outer flow — neither paragraph's own walk ever
    // sees either tag, and the null-span flush in `extract.rs` has no open
    // frame to catch. This currently drops the link entirely rather than
    // null-spanning it on either paragraph; pinning that as today's
    // (uncovered) behavior for this one shape, not as something this test
    // claims is correct. A link with leading text before it in the same
    // paragraph does not have this problem (see the other assertions above).
    let first_half = find_paragraph(&cnd.nodes, "First half of a multi-paragraph")
        .expect("the first half of the split link body");
    let CndNode::Paragraph(first_half) = first_half else { unreachable!() };
    assert!(
        first_half.base.links.is_empty(),
        "known gap: a `LinkElem` whose body splits into two paragraphs is \
         dropped, not null-spanned — got {:?}",
        first_half.base.links
    );
    let second_half = find_paragraph(&cnd.nodes, "Second half of the same link body")
        .expect("the second half of the split link body");
    let CndNode::Paragraph(second_half) = second_half else { unreachable!() };
    assert!(second_half.base.links.is_empty(), "same known gap, other half");
}

/// `(text, number)` of each item, for compact assertions on one level.
fn item_summary(items: &[ListItem]) -> Vec<(&str, Option<i32>)> {
    items.iter().map(|item| (item.text.as_str(), item.number)).collect()
}

/// The emitted list whose first item's text starts with `prefix`.
fn list_starting_with<'a>(
    lists: &[&'a typst_cnd::ListNode],
    prefix: &str,
) -> &'a typst_cnd::ListNode {
    lists
        .iter()
        .copied()
        .find(|list| list.items.first().is_some_and(|item| item.text.starts_with(prefix)))
        .unwrap_or_else(|| panic!("no list starting with {prefix:?}"))
}

#[test]
fn nested_list_items_are_children_of_their_parent() {
    let cnd = cnd_for_example("nested_lists.typ");
    let lists = find_lists(&cnd.nodes);

    // One list node per list written at the top level, plus the one inside
    // the figure (whose text the figure does not carry). The nested lists
    // and the list inside the quote are not lists of their own.
    let mut firsts: Vec<&str> =
        lists.iter().map(|list| list.items[0].text.as_str()).collect();
    firsts.sort_unstable();
    assert_eq!(
        firsts,
        [
            "NA1 top one",
            "NB1 enum one",
            "NC1 bullet with enum children",
            "ND1 bullet with explicit numbers",
            "NE1 explicit list inside",
            "NG2 list item in figure",
        ]
    );

    // Three bullet levels.
    let a = list_starting_with(&lists, "NA1");
    assert_eq!(
        item_summary(&a.items),
        [("NA1 top one", None), ("NA2 top two", None), ("NA6 top three", None)]
    );
    assert_eq!(
        item_summary(&a.items[1].children),
        [("NA3 child one", None), ("NA4 child two", None)]
    );
    assert_eq!(
        item_summary(&a.items[1].children[1].children),
        [("NA5 grandchild", None)]
    );
    assert!(a.items[1].children[0].children.is_empty());

    // Bullets under an enum.
    let b = list_starting_with(&lists, "NB1");
    assert!(b.ordered);
    assert_eq!(
        item_summary(&b.items),
        [("NB1 enum one", Some(1)), ("NB2 enum two", Some(2))]
    );
    assert_eq!(
        item_summary(&b.items[1].children),
        [("NB3 bullet in enum", None), ("NB4 bullet in enum", None)]
    );

    // An enum under a bullet, started at 5 by a set rule.
    let c = list_starting_with(&lists, "NC1");
    assert_eq!(item_summary(&c.items), [("NC1 bullet with enum children", None)]);
    assert_eq!(
        item_summary(&c.items[0].children),
        [("NC2 enum at five", Some(5)), ("NC3 enum at six", Some(6))]
    );

    // An enum under a bullet, started at 5 by explicit markup.
    let d = list_starting_with(&lists, "ND1");
    assert_eq!(item_summary(&d.items), [("ND1 bullet with explicit numbers", None)]);
    assert_eq!(
        item_summary(&d.items[0].children),
        [("ND2 explicit five", Some(5)), ("ND3 follows five", Some(6))]
    );

    // An explicit `#list` call inside an item.
    let e = list_starting_with(&lists, "NE1");
    assert_eq!(item_summary(&e.items), [("NE1 explicit list inside", None)]);
    assert_eq!(
        item_summary(&e.items[0].children),
        [("NE2 explicit child", None), ("NE3 explicit child", None)]
    );

    // Every authored line of the lists, the quote's and the figure's
    // included, is in the document exactly once.
    let json = cnd_to_json(&cnd).expect("serialize");
    for token in [
        "NA1", "NA2", "NA3", "NA4", "NA5", "NA6", "NB1", "NB2", "NB3", "NB4", "NC1",
        "NC2", "NC3", "ND1", "ND2", "ND3", "NE1", "NE2", "NE3", "NF1", "NF2", "NF3",
        "NG2", "NG3", "NH1",
    ] {
        assert_eq!(json.matches(token).count(), 1, "{token} must appear exactly once");
    }

    // The quote carries its own list.
    let quote = cnd
        .nodes
        .iter()
        .find_map(|node| match node {
            CndNode::Quote(quote) => Some(quote),
            _ => None,
        })
        .expect("a quote");
    assert!(quote.text.contains("NF2") && quote.text.contains("NF3"), "{:?}", quote.text);
}

/// How many times each word occurs in the serialized CND.
fn word_counts(cnd: &typst_cnd::Cnd) -> std::collections::HashMap<String, usize> {
    let json = cnd_to_json(cnd).expect("serialize");
    let mut counts = std::collections::HashMap::new();
    for word in json.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        *counts.entry(word.to_string()).or_default() += 1;
    }
    counts
}

/// The text of every list item, nested ones included.
fn all_list_item_texts(cnd: &typst_cnd::Cnd) -> Vec<String> {
    fn walk(items: &[ListItem], out: &mut Vec<String>) {
        for item in items {
            out.push(item.text.clone());
            walk(&item.children, out);
        }
    }
    let mut out = Vec::new();
    for list in find_lists(&cnd.nodes) {
        walk(&list.items, &mut out);
    }
    out
}

#[test]
fn lists_in_floats_beside_open_lists_are_kept_once() {
    // A float is laid out wherever it fits, which can be while a list,
    // written elsewhere, spans the page. The list inside the float's figure
    // must not be taken for that list's nested content.
    for name in ["list_in_float_long.typ", "list_in_float_short.typ"] {
        let cnd = cnd_for_example(name);
        let counts = word_counts(&cnd);
        let last = if name == "list_in_float_long.typ" { 29 } else { 2 };
        let mut tokens = vec!["FLB1".to_string(), "FLB2".into(), "FLT1".into()];
        for i in 0..=last {
            tokens.push(format!("PA{i}"));
            tokens.push(format!("QA{i}"));
        }
        for token in tokens {
            assert_eq!(
                counts.get(&token).copied().unwrap_or(0),
                1,
                "{name}: {token} must appear exactly once"
            );
        }
        let texts = all_list_item_texts(&cnd);
        for token in ["FLB1", "FLB2", "FLT1"] {
            assert!(
                texts.iter().any(|text| text.contains(token)),
                "{name}: {token} must be carried by a list node"
            );
        }
    }
}

#[test]
fn lists_in_bare_floats_beside_open_lists_are_kept_once() {
    // Without a figure around it, nothing in the frames encloses the drawn
    // list: the float's tags close where it is written. The list open
    // around the drawn body is not what the float was written in.
    for (name, tokens) in [
        ("list_in_bare_float.typ", &["FLB1", "FLB2", "FLT1", "FLT2"][..]),
        ("list_in_bare_float_block.typ", &["FLB1"][..]),
    ] {
        let cnd = cnd_for_example(name);
        let counts = word_counts(&cnd);
        let texts = all_list_item_texts(&cnd);
        for token in tokens {
            assert_eq!(
                counts.get(*token).copied().unwrap_or(0),
                1,
                "{name}: {token} must appear exactly once"
            );
            assert!(
                texts.iter().any(|text| text.contains(token)),
                "{name}: {token} must be carried by a list node"
            );
        }
        for i in 0..30 {
            for prefix in ["PA", "QA"] {
                let token = format!("{prefix}{i}");
                if prefix == "QA" && name != "list_in_bare_float.typ" {
                    continue;
                }
                assert_eq!(
                    counts.get(&token).copied().unwrap_or(0),
                    1,
                    "{name}: {token} must appear exactly once"
                );
            }
        }
    }

    // The list nested in the float's own list stays its child.
    let cnd = cnd_for_example("list_in_bare_float.typ");
    let parent = find_lists(&cnd.nodes)
        .into_iter()
        .flat_map(|list| list.items.iter())
        .find(|item| item.text.contains("FLT1"))
        .expect("FLT1 item");
    assert!(
        parent.children.iter().any(|child| child.text.contains("FLT2")),
        "FLT2 must be FLT1's child, not a list of its own"
    );
}

#[test]
fn a_float_paragraph_beside_an_open_list_is_kept() {
    let cnd = cnd_for_example("par_in_float_beside_list.typ");
    let counts = word_counts(&cnd);
    assert_eq!(counts.get("FPB1").copied().unwrap_or(0), 1, "FPB1 must appear once");
    assert!(
        paragraph_texts_in_order(&cnd.nodes)
            .iter()
            .any(|text| text.contains("FPB1")),
        "FPB1 must be a paragraph"
    );
}

#[test]
fn a_float_written_in_a_list_item_is_kept_once() {
    // The item carries the float's text; the float is written in the list,
    // so the list open where its body is drawn is a real ancestor.
    let cnd = cnd_for_example("float_in_list_item.typ");
    let counts = word_counts(&cnd);
    assert_eq!(counts.get("FIL1").copied().unwrap_or(0), 1, "FIL1 must appear once");
}

#[test]
fn lists_in_footnotes_live_in_the_footnote_pool_only() {
    for (name, tokens) in [
        ("list_in_footnote_item.typ", &["FL1", "FL2"][..]),
        ("list_in_footnote_par.typ", &["FL3"][..]),
    ] {
        let cnd = cnd_for_example(name);
        let texts = all_list_item_texts(&cnd);
        let counts = word_counts(&cnd);
        for token in tokens {
            assert!(
                !texts.iter().any(|text| text.contains(token)),
                "{name}: {token} must not be in a body list node: {texts:?}"
            );
            assert!(
                cnd.footnotes.iter().any(|note| note.text.contains(token)),
                "{name}: {token} must be in the footnote pool"
            );
            assert_eq!(counts.get(*token).copied(), Some(1), "{name}: {token} once");
        }
    }
}

#[test]
fn inline_box_bodies_are_emitted_once_whatever_their_origin() {
    // A box body is realized as a paragraph of its own inside the line of the
    // paragraph that holds it. Whether its spans are detached (a string
    // passed through a styling function) or come from another file, it is
    // part of that paragraph — and a floated body is still content.
    let cnd = cnd_for_example("box_identity.typ");
    let counts = word_counts(&cnd);
    let mut tokens: Vec<String> = Vec::new();
    for letter in 'A'..='J' {
        tokens.push(format!("Q{letter}1"));
        tokens.push(format!("Q{letter}2"));
    }
    tokens.extend(["QK1", "QL1", "QM1"].map(String::from));
    for token in ["RA1", "RA2", "RB1", "RB2", "RC1", "RD1", "RE1", "RE2", "RF1"] {
        tokens.push(token.into());
    }
    for token in ["SA1", "SA2", "SA3", "SA4"] {
        tokens.push(token.into());
    }
    let mut wrong = Vec::new();
    for token in &tokens {
        let count = counts.get(token).copied().unwrap_or(0);
        if count != 1 {
            wrong.push(format!("{token}={count}"));
        }
    }
    assert!(wrong.is_empty(), "each token must appear exactly once: {wrong:?}");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    assert!(
        paragraphs.iter().any(|p| p.contains("SA3 FLOATED BODY")),
        "the floated body must be a paragraph: {paragraphs:?}"
    );
    assert!(
        paragraphs
            .iter()
            .any(|p| p.starts_with("RE1 see") && p.contains("RE2")),
        "the imported box holding a reference stays in its paragraph: {paragraphs:?}"
    );
    // Known limit: a box body written in another file, whose reference
    // resolves to text ("Section 1") that the unrealized paragraph around it
    // does not have, is neither inside that paragraph's source nor found in
    // its text. It is kept as a paragraph of its own: a duplicate, never a
    // loss.
    assert!(
        paragraphs
            .iter()
            .any(|p| p.starts_with("RF1 then") && p.contains("RF2")),
        "the paragraph around a box written in another file keeps it: {paragraphs:?}"
    );
    assert!(
        counts.get("RF2").copied().unwrap_or(0) >= 1,
        "a box written in another file holding a reference is never lost"
    );
}

#[test]
fn floats_from_another_file_survive_matching_text() {
    // A float whose spans are in another file is not tested by source
    // containment; it must not be taken for a box body because the
    // paragraph it lands in holds the same sentence, or a longer word.
    let cnd = cnd_for_example("box_float_text.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    for float in [
        ["Yes indeed it is so."; 6].join(" "),
        // A float of two paragraphs: the break between them is no glue.
        ["Kept one QA."; 3].join(" "),
        ["Kept two QB."; 2].join(" "),
        // A reference realizes to text the unrealized float does not have.
        ["See Section 1 for QR details in full."; 5].join(" "),
    ] {
        // A reference's supplement is joined by a non-breaking space.
        let count = paragraphs
            .iter()
            .filter(|p| p.split_whitespace().collect::<Vec<_>>().join(" ") == float)
            .count();
        assert_eq!(count, 1, "{float:?} must be one paragraph: {paragraphs:?}");
    }
    let counts = word_counts(&cnd);
    assert_eq!(
        counts.get("Draft").copied().unwrap_or(0),
        1,
        "the floated word must survive beside a longer word: {paragraphs:?}"
    );
}

#[test]
fn floats_defined_earlier_do_not_claim_the_paragraphs_in_between() {
    // A float body built from an earlier `#let`, or a float helper defined in
    // the preamble, spans in the source from the definition to the call. The
    // paragraphs written in between are not that float's, and another float
    // landing in one of them must survive.
    let disclaimer = ["Yes indeed it is so."; 6].join(" ");
    for (name, own) in [
        ("float_scope_let.typ", "Wide note WZ."),
        ("float_scope_helper.typ", "Aside: later note AZ"),
    ] {
        let cnd = cnd_for_example(name);
        let paragraphs = paragraph_texts_in_order(&cnd.nodes);
        for float in [disclaimer.as_str(), own] {
            let count = paragraphs.iter().filter(|p| p.trim() == float).count();
            assert_eq!(
                count, 1,
                "{name}: {float:?} must be one paragraph: {paragraphs:?}"
            );
        }
    }
}

#[test]
fn a_float_quoting_another_file_does_not_claim_its_paragraph() {
    // The second note quotes, word for word, a paragraph written in an
    // included file. That paragraph has a source range of its own, outside
    // the float's, so it is not the float's body: another float landing in it
    // is kept, and so is the quoting float itself.
    let long = std::fs::read_to_string(example_path("_float_holds_chapter.typ"))
        .expect("chapter file");
    let long = long.trim();
    for (name, first) in [
        ("float_holds_by_range.typ", ["Yes indeed it is so."; 6].join(" ")),
        ("float_holds_by_range_quote.typ", ["Zed note here."; 6].join(" ")),
    ] {
        let first = first.as_str();
        let cnd = cnd_for_example(name);
        let paragraphs = paragraph_texts_in_order(&cnd.nodes);
        let count = |text: &str| paragraphs.iter().filter(|p| p.trim() == text).count();
        assert_eq!(
            count(first),
            1,
            "{name}: {first:?} must be one paragraph: {paragraphs:?}"
        );
        assert_eq!(
            count(long),
            2,
            "{name}: the chapter and the note quoting it must both stay: {paragraphs:?}"
        );
    }
}

#[test]
fn a_float_held_only_by_words_is_never_absorbed() {
    // A preamble helper's float reaches, in the source, over the long
    // paragraph, and its body quotes the note word for word (or the long
    // paragraph in full). The note lands in the long paragraph, which is no
    // float's body: the note must survive, whether it has a source range of
    // its own (in an included chapter) or none (a string through `upper`),
    // and even when the long paragraph is content the float also shows.
    let lower = ["Yes indeed it is so."; 6].join(" ");
    let upper = lower.to_uppercase();
    let long = |note: &str| {
        format!(
            "Long paragraph that spans pages: one two three four five six seven \
             eight nine ten. {note} eleven twelve thirteen fourteen fifteen \
             sixteen seventeen eighteen nineteen twenty twentyone twentytwo \
             twentythree twentyfour twentyfive twentysix twentyseven \
             twentyeight twentynine thirty."
        )
    };
    for (name, note, others) in [
        ("float_held_by_text.typ", &lower, vec![(format!("Quoted: {lower}"), 1)]),
        (
            "float_held_by_text_detached.typ",
            &upper,
            vec![(format!("Quoted: {upper}"), 1), (long(&upper), 1)],
        ),
        ("float_held_by_text_whole.typ", &lower, vec![(long(&lower), 2)]),
        ("float_held_by_text_prefix.typ", &lower, vec![(long(&lower), 1)]),
        ("float_held_by_text_reused.typ", &lower, vec![(long(&lower), 2)]),
    ] {
        let cnd = cnd_for_example(name);
        let paragraphs = paragraph_texts_in_order(&cnd.nodes);
        let count = |text: &str| paragraphs.iter().filter(|p| p.trim() == text).count();
        assert_eq!(
            count(note),
            1,
            "{name}: {note:?} must be one paragraph: {paragraphs:?}"
        );
        for (text, expected) in others {
            assert_eq!(
                count(&text),
                expected,
                "{name}: {text:?} must be {expected} paragraph(s): {paragraphs:?}"
            );
        }
    }
}

/// Every node of the tree, depth first, with heading and figure children.
fn all_nodes(nodes: &[CndNode]) -> Vec<&CndNode> {
    fn walk<'a>(nodes: &'a [CndNode], out: &mut Vec<&'a CndNode>) {
        for node in nodes {
            out.push(node);
            match node {
                CndNode::Heading(h) => walk(&h.children, out),
                CndNode::Figure(f) => walk(&f.children, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut out);
    out
}

#[test]
fn handbuilt_toc_emits_each_boxed_number_once() {
    // The number in a styled box is part of its grid cell's paragraph: it
    // must come out once, not once more as a paragraph of its own. The title
    // cells repeat headings, which stay nodes of their own.
    let cnd = cnd_for_example("handbuilt_toc.typ");
    let counts = word_counts(&cnd);
    for token in ["TP1", "TP2"] {
        assert_eq!(
            counts.get(token).copied(),
            Some(1),
            "{token} must appear exactly once"
        );
    }
    let headings = heading_texts(&cnd.nodes);
    assert_eq!(headings, ["TA1 first part", "TB1 second part"]);
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    for body in ["TA2 body.", "TB2 body."] {
        assert_eq!(
            paragraphs.iter().filter(|p| p.as_str() == body).count(),
            1,
            "{body:?} must be one paragraph: {paragraphs:?}"
        );
    }
}

#[test]
fn figure_bodies_are_not_duplicated() {
    let cnd = cnd_for_example("figure_bodies.typ");
    let nodes = all_nodes(&cnd.nodes);
    let captions: Vec<&str> = nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Figure(f) => f.caption.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(
        captions,
        [
            "FA3 caption one",
            "FB5 caption grid",
            "FC3 caption context grid",
            "FD3 caption block grid",
            "FF3 caption table",
        ]
    );
    let cells: Vec<&str> = nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .flat_map(|t| t.cells.iter().map(|c| c.text.as_str()))
        .collect();
    for cell in ["FB1 key", "FB4 value", "FD1 key", "FD2 value", "FF1 key", "FF2 value"] {
        assert_eq!(
            cells.iter().filter(|c| **c == cell).count(),
            1,
            "{cell:?} must be one table cell: {cells:?}"
        );
    }
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    assert_eq!(paragraphs, ["FE1 bare grid key", "FE2 bare grid value"]);
    // The list inside the framed figure body is kept once, as a list.
    assert_eq!(all_list_item_texts(&cnd), ["FA2 list item in figure"]);
    let counts = word_counts(&cnd);
    for token in ["FA2", "FB1", "FB4", "FD1", "FE1", "FE2"] {
        assert_eq!(
            counts.get(token).copied(),
            Some(1),
            "{token} must appear exactly once"
        );
    }
    // Known gap: a figure keeps only its caption when its body is not an image,
    // a raw block, a table or a grid it can reach: the paragraph of the
    // framed body (FA1) and the cells of a grid built under `context` (FC1,
    // FC2) are lost, and the list (FA2) is emitted beside the figure rather
    // than inside it.
}

#[test]
fn terms_never_swallow_paragraphs_that_start_like_them() {
    let cnd = cnd_for_example("terms_and_paragraphs.typ");
    let nodes = all_nodes(&cnd.nodes);
    let terms: Vec<(&str, &str)> = nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Terms(t) => Some(t),
            _ => None,
        })
        .flat_map(|t| t.items.iter().map(|i| (i.term.as_str(), i.description.as_str())))
        .collect();
    assert_eq!(terms, [("Status", "accepted"), ("Owner", "someone")]);
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    assert_eq!(
        paragraphs,
        [
            "Status of the rollout is still open, and this sentence stays.",
            "Owner wanted. This sentence stays as well.",
            "The owner field is set, and this sentence does not start with a term.",
        ]
    );
}

#[test]
fn a_list_in_a_quote_is_carried_by_the_quote_only() {
    let cnd = cnd_for_example("quote_with_list.typ");
    assert!(find_lists(&cnd.nodes).is_empty(), "the quote carries its list");
    let quote = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|n| match n {
            CndNode::Quote(q) => Some(q),
            _ => None,
        })
        .expect("a quote");
    for token in ["QW1", "QW2", "QW3"] {
        assert!(quote.text.contains(token), "{token} in the quote: {:?}", quote.text);
    }
    let counts = word_counts(&cnd);
    for token in ["QW1", "QW2", "QW3", "QW4"] {
        assert_eq!(
            counts.get(token).copied(),
            Some(1),
            "{token} must appear exactly once"
        );
    }
    assert_eq!(paragraph_texts_in_order(&cnd.nodes), ["QW4 after."]);
}

#[test]
fn a_figure_carries_its_raw_block() {
    let cnd = cnd_for_example("figure_raw.typ");
    let figures: Vec<_> = cnd
        .nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Figure(f) => Some(f),
            _ => None,
        })
        .collect();
    let carried: Vec<(Option<&str>, Vec<&str>)> = figures
        .iter()
        .map(|f| {
            let code = f
                .children
                .iter()
                .filter_map(|c| match c {
                    CndNode::Code(code) => Some(code.text.as_str()),
                    _ => None,
                })
                .collect();
            (f.caption.as_deref(), code)
        })
        .collect();
    assert_eq!(
        carried,
        [
            (Some("RA2 caption"), vec!["ra1: 1"]),
            (Some("RB2 caption styled"), vec!["rb1: 1"]),
            (Some("RC2 caption styled no context"), vec!["rc1: 1"]),
        ]
    );
    // Known gap: each raw block is also emitted as a code node beside its
    // figure, so its text appears twice.
}

#[test]
fn a_figure_around_a_helper_drawing_keeps_its_caption_and_image() {
    let cnd = cnd_for_example("figure_helper_drawing.typ");
    let nodes = all_nodes(&cnd.nodes);
    let captions: Vec<&str> = nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Figure(f) => f.caption.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(captions, ["HA2 caption"]);
    let images: Vec<&str> = nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Image(i) => i.path.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 1, "the image is emitted once: {images:?}");
    assert!(images[0].contains("newsletter-cover.png"), "{images:?}");
    // Known gap: the image escapes the figure and is emitted bare beside it,
    // the figure's child is the decorative grid with empty cells, and the
    // label text (HA1) is lost.
}
