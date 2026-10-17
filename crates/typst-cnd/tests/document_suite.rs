mod common;

use common::{
    NodeStats, all_example_files, all_nodes, all_nodes_full, all_texts,
    assert_cnd_contract, assert_json_roundtrip, assert_labels_globally_unique,
    assert_pool_refs_resolve, assert_refs_resolve, assert_tag_sequence,
    assert_unique_ids, cnd_for_example, codepoint_slice, compile_example, example_path,
    find_by_label, find_lists, first_paragraph, footnote_text, heading_texts,
    incoming_count, item_text, label_exists, nested_items, nested_lists, node_type,
    node_types, outer_lists, paragraph_texts_in_order, paragraph_values, table_stats,
    tags_under_heading, walk_nodes,
};
use typst_cnd::{CndNode, ListItem, RichText, cnd_to_json};

/// The value of an optional text, for compact assertions.
fn value(text: &Option<RichText>) -> Option<&str> {
    text.as_ref().map(|text| text.value.as_str())
}

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
    assert_eq!(value(&figure.caption), Some("Paramètres nominaux de fonctionnement."));
    assert!(figure.numbering.as_ref().is_some_and(|n| n.value.contains('1')));
    let table = figure
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Table(t) => Some(t),
            _ => None,
        })
        .expect("table child");
    // `table.raw` is gone in 0.5: the table's content is its cells, each
    // one's text a paragraph child.
    assert!(
        table
            .cells
            .iter()
            .any(|cell| !first_paragraph(&cell.children).is_empty()),
        "the table carries its cell text: {:?}",
        table.cells
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
    // The flow paragraphs; cells hold their own.
    assert_eq!(paragraph_texts_in_order(&cnd.nodes).len(), 8);
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
    // The references are written in the chapter's first paragraph, which
    // holds them with their spans; the heading's own text holds none.
    assert!(overview.text.refs.is_empty(), "{:?}", overview.text.refs);
    let intro = paragraph_at(&overview.children, 0);
    assert_eq!(
        intro
            .text
            .refs
            .iter()
            .map(|r| {
                let span = r.text_span.as_ref().expect("a span");
                (r.label.as_str(), codepoint_slice(&intro.text.value, span))
            })
            .collect::<Vec<_>>(),
        [
            ("sec-detail", "Section\u{a0}11".to_string()),
            ("tab-signaux", "Tableau\u{a0}1".to_string()),
            ("sec-annexe", "Section\u{a0}2".to_string()),
        ]
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
    assert!(!regulation_para.text.refs.is_empty());

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
    let CndNode::Table(standalone) = standalone else {
        panic!("expected bare table (standalone is not figure-wrapped)");
    };
    // Its cells hold the paragraphs laid out in them, in reading order, and
    // the header row is marked.
    assert_eq!(
        standalone
            .cells
            .iter()
            .map(|cell| (first_paragraph(&cell.children), cell.is_header))
            .collect::<Vec<_>>(),
        [
            ("Clé", true),
            ("Valeur", true),
            ("mode", false),
            ("production", false),
            ("revision", false),
            ("3.1", false),
        ]
    );
    for cell in &standalone.cells {
        assert_eq!(node_types(&cell.children), ["paragraph"]);
    }

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
    // Each figure holds its table only, and no `raw`: only a drawing does.
    for figure in [a, b] {
        assert_eq!(node_types(&figure.children), ["table"]);
        assert!(figure.raw.is_none());
    }
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
    assert!(
        a_table
            .cells
            .iter()
            .any(|c| first_paragraph(&c.children).contains("PT-A"))
    );
    assert!(
        b_table
            .cells
            .iter()
            .any(|c| first_paragraph(&c.children).contains("PT-C"))
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_columns_reading_order_flatten() {
    let cnd = cnd_for_example("complex_columns.typ");
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    // Columns stay flattened: only a grid is a `layout`.
    assert_eq!(stats.layouts, 0);

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
            CndNode::Paragraph(p) if p.text.value.starts_with("[L1]") => Some(p),
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
    assert!(value(&figure.caption).is_some_and(|c| c.contains("bus")));
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
                CndNode::Paragraph(p) if p.text.value.starts_with("[POST]") => Some(p),
                _ => None,
            }),
            _ => None,
        })
        .expect("post paragraph");
    assert!(!post.text.refs.is_empty());

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
    // The flow paragraphs; the quote holds its own.
    assert!(paragraph_texts_in_order(&cnd.nodes).len() >= 10);

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

    // The template lays the page out in a grid, which is a `layout`: the
    // articles sit in its cells, each heading once, opening a section in
    // its cell only.
    fn find_heading<'a>(nodes: &'a [CndNode], needle: &str) -> Option<&'a CndNode> {
        all_nodes(nodes)
            .into_iter()
            .find(|n| matches!(n, CndNode::Heading(h) if h.text.value.contains(needle)))
    }
    let CndNode::Layout(page) = &cnd.nodes[0] else {
        panic!("the template's page grid comes first: {:?}", node_type(&cnd.nodes[0]));
    };
    let in_cell = |cell: &typst_cnd::Cell| -> Vec<String> {
        cell.children
            .iter()
            .filter_map(|n| match n {
                CndNode::Heading(h) => Some(h.text.value.clone()),
                _ => None,
            })
            .collect()
    };
    let cell_headings: Vec<Vec<String>> = page.cells.iter().map(in_cell).collect();
    assert!(
        cell_headings.contains(&vec!["The Sixtus Award goes to Purview".to_string()]),
        "the main article is a cell of the page grid: {cell_headings:?}"
    );
    assert!(
        cell_headings.contains(&vec![
            "Guest lecture from Dr. Elizabeth Lee".to_string(),
            "Safety first".to_string(),
            "Tigers win big".to_string(),
        ]),
        "the side articles are a cell of the page grid: {cell_headings:?}"
    );
    assert_eq!(headings.len(), 5, "each heading once: {headings:?}");

    let sixtus = find_heading(&cnd.nodes, "Sixtus Award").expect("sixtus heading");
    let CndNode::Heading(sixtus) = sixtus else { panic!() };
    let sixtus_paragraphs: Vec<&str> = sixtus
        .children
        .iter()
        .filter_map(|c| match c {
            CndNode::Paragraph(p) => Some(p.text.value.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        sixtus_paragraphs.len(),
        4,
        "award section has exactly four prose paragraphs, got {sixtus_paragraphs:?}"
    );
    // The template boxes the quote's body and attribution in a grid: the
    // quote still holds its body once, and its attribution is its own.
    let quotes: Vec<&typst_cnd::QuoteNode> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|n| match n {
            CndNode::Quote(q) => Some(q),
            _ => None,
        })
        .collect();
    assert_eq!(quotes.len(), 1);
    assert_eq!(value(&quotes[0].attribution), Some("Prof. Herzog"));
    assert_eq!(node_types(&quotes[0].children), ["paragraph"]);
    assert_eq!(
        first_paragraph(&quotes[0].children).trim(),
        "Our Lab has synthesized the most elements of them all."
    );
    assert_eq!(occurrences(&cnd, "Our Lab has synthesized"), 1);
    assert_eq!(occurrences(&cnd, "Herzog"), 1);
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

    // The only quote is inline: text of its paragraph, not a node (cnd-sdk
    // spec §6.4). An inline quote does not show its attribution.
    assert_eq!(stats.quotes, 0, "an inline quote is not a node");
    assert!(paragraph_texts_in_order(&cnd.nodes).contains(
        &"\u{201c}Programs must be written for people to read.\u{201d}".into()
    ));
    assert!(!cnd_to_json(&cnd).expect("serialize").contains("Ada Lovelace"));
    assert!(outer_lists(&cnd.nodes).len() >= 2, "expected bullet and numbered lists");
    assert!(stats.code >= 1, "expected a code block node");
    assert!(stats.math >= 1, "expected a block math node");
    // Every captioned float is now a figure wrapper, including the
    // grid-in-figure — so this counts the image figure plus the grid figure.
    assert!(stats.figures >= 1, "expected at least one figure node");
    assert!(stats.layouts >= 1, "expected a grid-in-figure layout node");

    let grid_fig = find_by_label(&cnd.nodes, "rich-grid").expect("grid figure");
    let CndNode::Figure(grid_fig) = grid_fig else {
        panic!("expected grid figure node");
    };
    assert!(value(&grid_fig.caption).is_some_and(|c| c.contains("grid")));
    // A grid is a `layout` in 0.5, the former grid-kind table.
    assert!(
        grid_fig.children.iter().any(|c| matches!(c, CndNode::Layout(_))),
        "grid layout child"
    );

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
            // An inline quote is text of its own paragraph.
            "\u{201c}Programs must be written for people to read.\u{201d}".to_string(),
            "See Equation\u{a0}1, Figure\u{a0}1, and Figure\u{a0}2.".to_string(),
        ],
        "list/code/math content must not also appear as paragraph nodes"
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
    // The block quote only: the inline one is text of its paragraph.
    assert_eq!(stats.quotes, 1);
    // Lists written in the flow; a nested list is a node inside its item.
    assert!(outer_lists(&cnd.nodes).len() >= 2);
    assert_eq!(stats.code, 2);
    assert_eq!(stats.math, 2);
    // Two tables and a grid, which is a `layout` in 0.5.
    assert_eq!(stats.tables, 2);
    assert_eq!(stats.layouts, 1);
    // Three figure wrappers: the two captioned tables/grids (tab-signals,
    // fig-grid-zones) plus the one true non-table figure (fig-diagram).
    assert_eq!(stats.figures, 3);
    // The flow paragraphs, the inline quote's included; list items, cells
    // and the block quote hold their own.
    assert_eq!(paragraph_texts_in_order(&cnd.nodes).len(), 9);

    for label in [
        "ch-corpus",
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

    // An inline quote is not a node (cnd-sdk spec §6.4): its words stay in
    // their paragraph, quotation marks included, and its label names no
    // node, since none carries it exactly. Typst does not show an inline
    // quote's attribution, so neither does the CND.
    assert!(!label_exists(&cnd.nodes, "quote-knuth"), "an inline quote's label");
    assert_eq!(incoming_count(&cnd.nodes, "quote-knuth"), 0);
    let knuth: Vec<String> = paragraph_texts_in_order(&cnd.nodes)
        .into_iter()
        .filter(|p| p.contains("Programs are meant to be read"))
        .collect();
    assert_eq!(
        knuth,
        [
            "\u{201c}Programs are meant to be read by humans and only incidentally for machines to execute.\u{201d}"
        ]
    );
    assert!(!cnd_to_json(&cnd).expect("serialize").contains("Donald Knuth"));
    let hopper = block_quotes(&cnd.nodes);
    assert_eq!(hopper.len(), 1);
    assert_eq!(value(&hopper[0].attribution), Some("Grace Hopper"));
    assert_eq!(
        paragraph_values(&hopper[0].children),
        ["The most dangerous phrase in the language is: we've always done it this way."]
    );

    let grid_fig = find_by_label(&cnd.nodes, "fig-grid-zones").expect("grid");
    let CndNode::Figure(grid_fig) = grid_fig else {
        panic!("expected grid figure node");
    };
    assert!(value(&grid_fig.caption).is_some_and(|c| c.contains("Zone layout")));
    assert!(
        grid_fig.children.iter().any(|c| matches!(c, CndNode::Layout(_))),
        "grid layout child"
    );

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
            CndNode::Paragraph(p) if p.text.value.contains("Overview paragraph") => {
                Some(p)
            }
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
            CndNode::Paragraph(p) if p.text.value.contains("Closing paragraph") => {
                Some(p)
            }
            _ => None,
        })
        .expect("closing xref paragraph");
    assert!(closing.text.refs.len() >= 5);
    // A label is now the edge itself, not an optional decoration on it —
    // an unlabelled edge cannot be represented (ADR 0017).
    assert!(
        closing.text.refs.iter().all(|reference| !reference.label.is_empty()),
        "refs should carry Typst labels: {:?}",
        closing.text.refs
    );
    assert!(
        closing
            .text
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
            !list.ordered && list.items.iter().any(|item| !nested_lists(item).is_empty())
        })
        .expect("bullet list with nested children");
    assert_eq!(bullet.items.len(), 3);
    assert_eq!(nested_items(&bullet.items[1]).len(), 2);
    assert!(item_text(nested_items(&bullet.items[1])[1]).contains("LI-B2"));

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
    assert!(
        standalone
            .cells
            .iter()
            .any(|c| first_paragraph(&c.children).contains("complex_semantic"))
    );

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn complex_semantic_no_duplicate_container_paragraphs() {
    let cnd = cnd_for_example("complex_semantic.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);

    // The inline quote is a paragraph of the flow, once; the block quote
    // holds its own paragraph.
    assert_eq!(
        paragraphs
            .iter()
            .filter(|p| p.contains("Programs are meant to be read"))
            .count(),
        1
    );
    for leaked in [
        "The most dangerous phrase",
        "Grace Hopper",
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
    // Its only quote is inline: text of its paragraph (cnd-sdk spec §6.4).
    assert_eq!(stats.quotes, 0);
    assert_eq!(stats.code, 1);
    assert_eq!(stats.math, 1);
    assert_eq!(stats.lists, 1);
    // A table and a grid, which is a `layout` in 0.5.
    assert_eq!(stats.tables, 1);
    assert_eq!(stats.layouts, 1);

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
    // The inline quote is a paragraph of the column, once, with the
    // column's state; Typst does not show an inline quote's attribution.
    let quoted: Vec<&typst_cnd::ParagraphNode> = cols
        .children
        .iter()
        .filter_map(|n| match n {
            CndNode::Paragraph(p) if p.text.value.contains("Quote trapped") => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(quoted.len(), 1, "the quote's words are in one paragraph");
    assert_eq!(
        quoted[0].base.state_metadata.get("track").and_then(|v| v.as_str()),
        Some("left")
    );
    assert_eq!(occurrences(&cnd, "Quote trapped"), 1);
    assert!(!cnd_to_json(&cnd).expect("serialize").contains("Source L"));

    let grid_fig = find_by_label(&cnd.nodes, "fig-hc-grid").expect("grid");
    let CndNode::Figure(grid_fig) = grid_fig else {
        panic!("expected grid figure");
    };
    assert!(
        grid_fig.children.iter().any(|c| matches!(c, CndNode::Layout(_))),
        "grid layout child"
    );
    let layout = grid_fig
        .children
        .iter()
        .find_map(|c| match c {
            CndNode::Layout(layout) => Some(layout),
            _ => None,
        })
        .expect("layout");
    assert_eq!(
        layout
            .cells
            .iter()
            .map(|cell| first_paragraph(&cell.children))
            .collect::<Vec<_>>(),
        ["X", "Y", "1", "2"]
    );

    let tail = find_by_label(&cnd.nodes, "sec-tail").expect("tail");
    let CndNode::Heading(tail) = tail else {
        panic!("expected heading");
    };
    let tail_para = tail
        .children
        .iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.text.value.starts_with("[TAIL-1]") => Some(p),
            _ => None,
        })
        .expect("tail paragraph");
    assert!(
        tail_para.text.value.contains("Équation")
            && tail_para.text.value.contains("Tableau"),
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

    // Each inline code run is a `code` mark over its one occurrence.
    let text = paragraph_text(&cnd, "Call ");
    assert_eq!(mark_slices(text, typst_cnd::CODE), ["extract_text", "plain_text"]);

    let lists = find_lists(&cnd.nodes);
    assert!(!lists.is_empty(), "inline_code.typ should produce at least one list");
    for list in lists {
        for item in &list.items {
            for token in &["typst-cnd", "cargo test", "inline code"] {
                let count = item_text(item).matches(token).count();
                assert!(
                    count <= 1,
                    "inline code token {token:?} appears {count}× in list item (expected ≤1): {:?}",
                    item_text(item)
                );
            }
        }
    }

    let quote_texts: Vec<String> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Quote(q) => Some(first_paragraph(&q.children).to_string()),
            _ => None,
        })
        .collect();
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

    // Two definition lists. The list nested in a term description is a node
    // of that description, never also one of the flow.
    assert_eq!(stats.terms, 2);
    assert_eq!(stats.lists, 1, "the nested list is one node, in the term");
    let flow_lists = |nodes: &[CndNode]| -> usize {
        fn count(nodes: &[CndNode]) -> usize {
            nodes
                .iter()
                .map(|node| match node {
                    CndNode::List(_) => 1,
                    CndNode::Heading(h) => count(&h.children),
                    _ => 0,
                })
                .sum()
        }
        count(nodes)
    };
    assert_eq!(flow_lists(&cnd.nodes), 0, "the nested list is not a flow node");

    let terms: Vec<&CndNode> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter(|node| matches!(node, CndNode::Terms(_)))
        .collect();
    assert_eq!(terms.len(), 2);

    // `tight` is gone in 0.5 (spacing is presentation): the tight and the
    // wide list are two distinct nodes, each with its own items.
    let CndNode::Terms(tight) = terms[0] else { panic!() };
    assert_eq!(tight.items.len(), 3);
    assert_eq!(tight.items[0].term.value, "manifest");
    assert_eq!(
        first_paragraph(&tight.items[0].children),
        "The serialized document tree."
    );

    let CndNode::Terms(wide) = terms[1] else { panic!() };
    assert_ne!(tight.base.id, wide.base.id);
    assert_eq!(wide.items.len(), 3);
    let consumer = &wide.items[1];
    assert_eq!(consumer.term.value, "consumer");
    assert_eq!(node_types(&consumer.children), ["paragraph", "list"]);
    assert_eq!(
        first_paragraph(&consumer.children),
        "A reader of the manifest. Consumers include:"
    );
    let CndNode::List(nested) = &consumer.children[1] else { unreachable!() };
    assert!(!nested.ordered);
    assert_eq!(
        nested.items.iter().map(item_text).collect::<Vec<_>>(),
        ["the search indexer", "the agent orchestrator", "export tooling"]
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
    assert!(value(&figure.caption).is_some_and(|c| c.contains("cover art")));

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
    assert_eq!(node_types(&figure.children), ["image"]);
    assert!(figure.raw.is_none(), "an image figure is no drawing");
    // The figure owns its label and location; the image its own.
    assert!(image.base.label.is_none());
    let see = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|n| match n {
            CndNode::Paragraph(p) if p.text.value.starts_with("See ") => Some(p),
            _ => None,
        })
        .expect("the referring paragraph");
    assert_eq!(
        see.text.refs.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        ["fig-cover"]
    );

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
    let CndNode::Figure(figure) = find_by_label(&cnd.nodes, "fig-cover").expect("figure")
    else {
        panic!("expected figure wrapper");
    };
    assert_eq!(node_types(&figure.children), ["image"]);
}

#[test]
fn footnotes_pool_and_edges() {
    let cnd = cnd_for_example("footnotes.typ");

    // Three declaration footnotes become pool entries; the labelled note is
    // referenced twice (its declaration plus a `#footnote(<label>)` marker).
    let labels: Vec<&str> = cnd.footnotes.iter().map(|f| f.label.as_str()).collect();
    assert_eq!(labels, vec!["1", "2", "3"]);
    // Each note holds its paragraph, without the number its entry starts
    // with.
    let contents: Vec<Vec<&str>> =
        cnd.footnotes.iter().map(|f| paragraph_values(&f.children)).collect();
    assert_eq!(
        contents,
        [
            ["First noted in the 2023 audit of the ingestion path."],
            ["See the follow-up report for details."],
            ["The canonical definition lives in the glossary."],
        ]
    );
    assert!(
        cnd.footnotes.iter().any(|f| footnote_text(f).contains("2023 audit")),
        "footnote body text is captured in the pool"
    );

    assert_pool_refs_resolve(&cnd);

    // Collect footnote edges per paragraph in reading order.
    let mut edges: Vec<Vec<String>> = Vec::new();
    fn walk(nodes: &[CndNode], out: &mut Vec<Vec<String>>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(p) => {
                    out.push(p.text.footnotes.iter().map(|r| r.label.clone()).collect());
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
                    for reference in &p.text.footnotes {
                        out.push((
                            p.text.value.clone(),
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
        value(&smith.formatted).is_some_and(|f| !f.is_empty()),
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
                    p.text
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
                    for cite in &p.text.cites {
                        out.push((
                            p.text.value.clone(),
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
    for text in all_texts(&cnd.nodes) {
        for reference in &text.refs {
            let label = reference.label.as_str();
            assert!(
                label != "smith2024" && label != "jones2022",
                "citation key leaked into refs: {label}"
            );
        }
    }

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
                for reference in &p.text.refs {
                    if let Some(span) = &reference.text_span {
                        out.push((
                            p.text.value.clone(),
                            reference.label.clone(),
                            span.clone(),
                        ));
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
    // Rendering a reference adds no mark of its own.
    for text in all_texts(&cnd.nodes) {
        for reference in &text.refs {
            let Some(span) = &reference.text_span else { continue };
            assert!(
                !text.marks.iter().any(|m| m.start < span[1] && m.end > span[0]),
                "a mark covers the reference {span:?} in {:?}",
                text.value
            );
        }
    }

    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn markers_in_non_flat_nodes_do_not_leak_or_span() {
    let cnd = cnd_for_example("nonflat_markers.typ");

    // The footnote still enters the pool even though it sits in a list item.
    assert_eq!(cnd.footnotes.len(), 1);
    assert!(footnote_text(&cnd.footnotes[0]).contains("inside a list item"));

    // Leak guard: the footnote body must not bleed into the list item text.
    // The item's paragraph is realized, so it holds the rendered marker, and
    // the edge sits on it with a span over that marker.
    let lists = find_lists(&cnd.nodes);
    let list = lists.first().expect("a bullet list");
    let carrier = list
        .items
        .iter()
        .find(|item| item_text(item).contains("carrying a footnote"))
        .expect("the footnote-carrying item");
    assert_eq!(
        item_text(carrier),
        "An item carrying a footnote.1",
        "the footnote body must not leak into the list item text"
    );
    let CndNode::Paragraph(paragraph) = &carrier.children[0] else { unreachable!() };
    assert_eq!(paragraph.text.footnotes.len(), 1);
    let span = paragraph.text.footnotes[0].text_span.as_ref().expect("a span");
    assert_eq!(codepoint_slice(&paragraph.text.value, span), "1");

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
        "containers_lists.typ",
        "containers_grids.typ",
        "tables_across_pages.typ",
        "figure_kinds.typ",
        "figure_drawings.typ",
        "containers_quotes_notes.typ",
        "footnote_across_pages.typ",
        "footnote_list_across_pages.typ",
        "footnote_reused_content.typ",
        "quote_attribution_reuse.typ",
        "marks.typ",
        "bib_rich.typ",
        "lang.typ",
        "table_kinds.typ",
        "figure_description.typ",
        "ref_exact_targets.typ",
        "math_refs.typ",
        "doc_metadata.typ",
        "doc_metadata_markers.typ",
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
    // Each heading holds its own paragraph, and nothing else of text.
    let shapes: Vec<(String, Vec<&str>)> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Heading(h) => Some((h.text.value.clone(), node_types(&h.children))),
            _ => None,
        })
        .collect();
    assert_eq!(
        shapes,
        [
            ("1. Overview".to_string(), vec!["paragraph", "heading"]),
            ("1.1 Details".to_string(), vec!["paragraph"]),
            ("2. Operations".to_string(), vec!["paragraph"]),
        ]
    );
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
    // The author's `#strong` is a mark; the inline artifact leaves none.
    assert_eq!(
        mark_slices(paragraph_text(&cnd, "option 3"), typst_cnd::STRONG),
        ["Decision"]
    );
    assert!(paragraph_text(&cnd, "Before after.").marks.is_empty());
    // The quote a show rule boxes into the paragraph is left out of the
    // paragraph's text, so it is a node of its own, holding its words once.
    let quotes = block_quotes(&cnd.nodes);
    assert_eq!(quotes.len(), 1);
    assert_eq!(paragraph_values(&quotes[0].children), ["Quoted aside"]);
    assert_eq!(occurrences(&cnd, "Quoted aside"), 1);
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

    // The cards' grid is a `layout`: each card's blocks are its cell's
    // paragraphs, in reading order.
    let layouts: Vec<&typst_cnd::LayoutNode> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Layout(layout) => Some(layout),
            _ => None,
        })
        .collect();
    assert_eq!(layouts.len(), 1);
    assert_eq!(
        layouts[0]
            .cells
            .iter()
            .map(|cell| paragraph_values(&cell.children))
            .collect::<Vec<_>>(),
        [
            ["LEFT CARD", "Left item one", "Left item two"],
            ["RIGHT CARD", "Right item one", "Right item two"],
        ]
    );

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
    let Some(CndNode::Paragraph(p)) = find_paragraph(nodes, needle) else {
        return ParagraphEdges::default();
    };
    ParagraphEdges {
        refs: p.text.refs.iter().map(|r| r.label.clone()).collect(),
        cites: p.text.cites.iter().map(|c| c.label.clone()).collect(),
        footnotes: p.text.footnotes.iter().map(|f| f.label.clone()).collect(),
    }
}

/// The first paragraph whose text contains `needle`, anywhere in the tree.
fn find_paragraph<'a>(nodes: &'a [CndNode], needle: &str) -> Option<&'a CndNode> {
    all_nodes(nodes).into_iter().find(
        |node| matches!(node, CndNode::Paragraph(p) if p.text.value.contains(needle)),
    )
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
        .text
        .links
        .iter()
        .find(|l| l.href == "https://example.com")
        .unwrap_or_else(|| panic!("expected a links entry in {:?}", nested.text.links));
    let link_span = link
        .text_span
        .as_ref()
        .unwrap_or_else(|| panic!("expected a non-null span for {link:?}"));

    let reference = nested
        .text
        .refs
        .iter()
        .find(|r| r.label == "sec-overview")
        .unwrap_or_else(|| panic!("expected a refs entry in {:?}", nested.text.refs));
    let ref_span = reference
        .text_span
        .as_ref()
        .unwrap_or_else(|| panic!("expected a non-null span for {reference:?}"));

    assert!(
        link_span[0] <= ref_span[0] && ref_span[1] <= link_span[1],
        "the link's span {link_span:?} must contain the ref's span {ref_span:?}"
    );
    let link_slice = codepoint_slice(&nested.text.value, link_span);
    let ref_slice = codepoint_slice(&nested.text.value, ref_span);
    assert!(
        link_slice.contains(&ref_slice),
        "the link's rendered text {link_slice:?} must contain the ref's {ref_slice:?}"
    );
    assert!(
        link_slice.contains("and more text"),
        "the link's span must cover the whole body, got {link_slice:?}"
    );
    // The paragraph's inline code is marked, and leaves both spans whole.
    assert!(!mark_slices(&nested.text, typst_cnd::CODE).is_empty());

    // Row 1: a plain URL link — `links` entry, href verbatim, span slicing
    // to the rendered (auto) body text.
    let plain =
        find_paragraph(&cnd.nodes, "in running text").expect("the plain-url paragraph");
    let CndNode::Paragraph(plain) = plain else { unreachable!() };
    let plain_link = plain
        .text
        .links
        .iter()
        .find(|l| l.href == "https://example.com/docs")
        .unwrap_or_else(|| panic!("expected a links entry in {:?}", plain.text.links));
    let plain_span = plain_link.text_span.as_ref().expect("non-null span");
    assert_eq!(
        codepoint_slice(&plain.text.value, plain_span),
        "https://example.com/docs"
    );

    // Row 2: `#link(<label>)[body]` is a cross-reference with a custom
    // body — a `refs` entry, not a `links` entry.
    let labelled = find_paragraph(&cnd.nodes, "see the overview")
        .expect("the link-to-label paragraph");
    let CndNode::Paragraph(labelled) = labelled else { unreachable!() };
    let labelled_ref = labelled
        .text
        .refs
        .iter()
        .find(|r| r.label == "sec-overview")
        .unwrap_or_else(|| panic!("expected a refs entry in {:?}", labelled.text.refs));
    assert_eq!(
        codepoint_slice(&labelled.text.value, labelled_ref.text_span.as_ref().unwrap()),
        "see the overview"
    );
    assert!(
        labelled.text.links.is_empty(),
        "a link-to-label must not also surface as a links entry: {:?}",
        labelled.text.links
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
        .text
        .links
        .iter()
        .find(|l| l.href == "https://example.com/cover")
        .unwrap_or_else(|| {
            panic!("expected a links entry in {:?}", empty_body_paragraph.text.links)
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
        position_link_paragraph.text.links.is_empty(),
        "a position destination must be dropped, not emitted as a links entry: {:?}",
        position_link_paragraph.text.links
    );
    assert!(
        position_link_paragraph.text.refs.is_empty(),
        "a position destination must be dropped, not emitted as a refs entry: {:?}",
        position_link_paragraph.text.refs
    );

    // A link inside a list item: the item's body is a realized paragraph of
    // its own (CND 0.5), so the edge sits on that paragraph with a span, like
    // any other paragraph's.
    let lists = find_lists(&cnd.nodes);
    let list = lists.first().expect("the bullet list");
    let list_paragraphs: Vec<&RichText> =
        list.items.iter().flat_map(|item| all_texts(&item.children)).collect();
    let (list_text, list_link) = list_paragraphs
        .iter()
        .find_map(|text| {
            text.links
                .iter()
                .find(|l| l.href == "https://example.com/list")
                .map(|link| (text, link))
        })
        .unwrap_or_else(|| panic!("expected a links entry in {list_paragraphs:?}"));
    let span = list_link.text_span.as_ref().expect("a link in an item is spanned");
    assert_eq!(codepoint_slice(&list_text.value, span), "here");

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
        first_half.text.links.is_empty(),
        "known gap: a `LinkElem` whose body splits into two paragraphs is \
         dropped, not null-spanned — got {:?}",
        first_half.text.links
    );
    let second_half = find_paragraph(&cnd.nodes, "Second half of the same link body")
        .expect("the second half of the split link body");
    let CndNode::Paragraph(second_half) = second_half else { unreachable!() };
    assert!(second_half.text.links.is_empty(), "same known gap, other half");
}

/// `(text, number)` of each item, for compact assertions on one level.
fn item_summary<'a>(
    items: impl IntoIterator<Item = &'a ListItem>,
) -> Vec<(&'a str, Option<i32>)> {
    items.into_iter().map(|item| (item_text(item), item.number)).collect()
}

/// The emitted list whose first item's text starts with `prefix`.
fn list_starting_with<'a>(
    lists: &[&'a typst_cnd::ListNode],
    prefix: &str,
) -> &'a typst_cnd::ListNode {
    lists
        .iter()
        .copied()
        .find(|list| {
            list.items
                .first()
                .is_some_and(|item| item_text(item).starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("no list starting with {prefix:?}"))
}

#[test]
fn nested_list_items_are_children_of_their_parent() {
    let cnd = cnd_for_example("nested_lists.typ");
    let lists = find_lists(&cnd.nodes);

    // One outer list node per list written at the top level, plus the ones
    // inside the figure and the quote, which are their blocks. Nested lists
    // are list nodes inside their parent item.
    let nested: std::collections::HashSet<uuid::Uuid> = lists
        .iter()
        .flat_map(|list| list.items.iter())
        .flat_map(nested_lists)
        .map(|list| list.base.id)
        .collect();
    let mut firsts: Vec<&str> = lists
        .iter()
        .filter(|list| !nested.contains(&list.base.id))
        .map(|list| item_text(&list.items[0]))
        .collect();
    firsts.sort_unstable();
    assert_eq!(
        firsts,
        [
            "NA1 top one",
            "NB1 enum one",
            "NC1 bullet with enum children",
            "ND1 bullet with explicit numbers",
            "NE1 explicit list inside",
            "NF2 quote list item",
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
        item_summary(nested_items(&a.items[1])),
        [("NA3 child one", None), ("NA4 child two", None)]
    );
    assert_eq!(
        item_summary(nested_items(nested_items(&a.items[1])[1])),
        [("NA5 grandchild", None)]
    );
    assert!(nested_lists(nested_items(&a.items[1])[0]).is_empty());
    assert_eq!(node_types(&a.items[0].children), ["paragraph"]);
    assert_eq!(node_types(&a.items[1].children), ["paragraph", "list"]);
    assert_eq!(node_types(&nested_items(&a.items[1])[1].children), ["paragraph", "list"]);

    // Bullets under an enum.
    let b = list_starting_with(&lists, "NB1");
    assert!(b.ordered);
    assert_eq!(
        item_summary(&b.items),
        [("NB1 enum one", Some(1)), ("NB2 enum two", Some(2))]
    );
    assert_eq!(
        item_summary(nested_items(&b.items[1])),
        [("NB3 bullet in enum", None), ("NB4 bullet in enum", None)]
    );
    assert!(nested_lists(&b.items[1]).iter().all(|list| !list.ordered));

    // An enum under a bullet, started at 5 by a set rule.
    let c = list_starting_with(&lists, "NC1");
    assert_eq!(item_summary(&c.items), [("NC1 bullet with enum children", None)]);
    assert_eq!(
        item_summary(nested_items(&c.items[0])),
        [("NC2 enum at five", Some(5)), ("NC3 enum at six", Some(6))]
    );
    assert!(nested_lists(&c.items[0]).iter().all(|list| list.ordered));
    assert_eq!(node_types(&c.items[0].children), ["paragraph", "list"]);

    // An enum under a bullet, started at 5 by explicit markup.
    let d = list_starting_with(&lists, "ND1");
    assert_eq!(item_summary(&d.items), [("ND1 bullet with explicit numbers", None)]);
    assert_eq!(
        item_summary(nested_items(&d.items[0])),
        [("ND2 explicit five", Some(5)), ("ND3 follows five", Some(6))]
    );

    // An explicit `#list` call inside an item.
    let e = list_starting_with(&lists, "NE1");
    assert_eq!(item_summary(&e.items), [("NE1 explicit list inside", None)]);
    assert_eq!(node_types(&e.items[0].children), ["paragraph", "list"]);
    assert_eq!(
        item_summary(nested_items(&e.items[0])),
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

    // The quote holds its paragraph and its list.
    let quote = cnd
        .nodes
        .iter()
        .find_map(|node| match node {
            CndNode::Quote(quote) => Some(quote),
            _ => None,
        })
        .expect("a quote");
    assert_eq!(node_types(&quote.children), ["paragraph", "list"]);
    let f = list_starting_with(&lists, "NF2");
    assert_eq!(
        item_summary(&f.items),
        [("NF2 quote list item", None), ("NF3 quote list item", None)]
    );
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
    find_lists(&cnd.nodes)
        .into_iter()
        .flat_map(|list| list.items.iter())
        .map(|item| item_text(item).to_string())
        .collect()
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
        // The open lists' items hold their own paragraph only: the float's
        // list is not routed into whichever item is open where it is drawn.
        let open_items: Vec<&ListItem> = find_lists(&cnd.nodes)
            .into_iter()
            .flat_map(|list| list.items.iter())
            .filter(|item| {
                item_text(item).starts_with("PA") || item_text(item).starts_with("QA")
            })
            .collect();
        assert_eq!(open_items.len(), 2 * (last + 1), "{name}");
        for item in open_items {
            assert_eq!(
                node_types(&item.children),
                ["paragraph"],
                "{name}: {}",
                item_text(item)
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
        .find(|item| item_text(item).contains("FLT1"))
        .expect("FLT1 item");
    assert!(
        nested_items(parent)
            .iter()
            .any(|child| item_text(child).contains("FLT2")),
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
    // Its paragraph is a child of the item it is written in, drawn on an
    // earlier page than that item's body.
    let lists = outer_lists(&cnd.nodes);
    let list = list_starting_with(&lists, "First item");
    assert_eq!(list.items.len(), 3);
    let second = paragraph_values(&list.items[1].children);
    assert_eq!(second.len(), 2, "{second:?}");
    assert_eq!(second[0], "FIL1 aside written in an item.");
    assert!(second[1].starts_with("Second item"), "{second:?}");
}

#[test]
fn lists_in_footnotes_live_in_the_footnote_pool_only() {
    for (name, tokens) in [
        ("list_in_footnote_item.typ", &["FL1", "FL2"][..]),
        ("list_in_footnote_par.typ", &["FL3"][..]),
    ] {
        let cnd = cnd_for_example(name);
        // The note's paragraph and list are its children, and each list
        // item holds its own paragraph.
        assert_eq!(cnd.footnotes.len(), 1, "{name}");
        let note = &cnd.footnotes[0];
        assert_eq!(node_types(&note.children), ["paragraph", "list"], "{name}");
        let CndNode::List(list) = &note.children[1] else { unreachable!() };
        let items: Vec<&str> = list.items.iter().map(item_text).collect();
        assert_eq!(items.len(), tokens.len(), "{name}: {items:?}");
        for (item, token) in items.iter().zip(tokens) {
            assert!(item.starts_with(token), "{name}: {item:?}");
        }
        let texts = all_list_item_texts(&cnd);
        let counts = word_counts(&cnd);
        for token in tokens {
            assert!(
                !texts.iter().any(|text| text.contains(token)),
                "{name}: {token} must not be in a body list node: {texts:?}"
            );
            assert!(
                cnd.footnotes.iter().any(|note| footnote_text(note).contains(token)),
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
            CndNode::Figure(f) => value(&f.caption),
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
    // A grid in a figure is a `layout` in 0.5, the former grid-kind table.
    let cells: Vec<&str> = nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Table(t) => Some(&t.cells),
            CndNode::Layout(l) => Some(&l.cells),
            _ => None,
        })
        .flat_map(|cells| cells.iter().map(|c| first_paragraph(&c.children)))
        .collect();
    for cell in ["FB1 key", "FB4 value", "FD1 key", "FD2 value", "FF1 key", "FF2 value"] {
        assert_eq!(
            cells.iter().filter(|c| **c == cell).count(),
            1,
            "{cell:?} must be one table cell: {cells:?}"
        );
    }
    for cell in ["FC1 key", "FC2 value"] {
        assert_eq!(
            cells.iter().filter(|c| **c == cell).count(),
            1,
            "{cell:?} must be one layout cell: {cells:?}"
        );
    }
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    assert_eq!(paragraphs, ["FE1 bare grid key", "FE2 bare grid value"]);
    // The framed body's paragraph and list are the figure's children, in
    // reading order, each once.
    let framed = cnd
        .nodes
        .iter()
        .find_map(|n| match n {
            CndNode::Figure(f) if value(&f.caption) == Some("FA3 caption one") => Some(f),
            _ => None,
        })
        .expect("the framed figure");
    assert_eq!(node_types(&framed.children), ["paragraph", "list"]);
    assert_eq!(paragraph_values(&framed.children), ["FA1 paragraph in figure."]);
    assert_eq!(all_list_item_texts(&cnd), ["FA2 list item in figure"]);
    assert!(framed.raw.is_none());
    // Each grid or table figure holds its grid or table, and nothing else.
    let bodies: Vec<(&str, Vec<&str>)> = cnd
        .nodes
        .iter()
        .filter_map(|n| match n {
            CndNode::Figure(f) => Some((value(&f.caption)?, node_types(&f.children))),
            _ => None,
        })
        .collect();
    assert_eq!(
        bodies,
        [
            ("FA3 caption one", vec!["paragraph", "list"]),
            ("FB5 caption grid", vec!["layout"]),
            ("FC3 caption context grid", vec!["layout"]),
            ("FD3 caption block grid", vec!["layout"]),
            ("FF3 caption table", vec!["table"]),
        ]
    );
    let counts = word_counts(&cnd);
    for token in [
        "FA1", "FA2", "FA3", "FB1", "FB4", "FB5", "FC1", "FC2", "FC3", "FD1", "FE1",
        "FE2", "FF1", "FF3",
    ] {
        assert_eq!(
            counts.get(token).copied(),
            Some(1),
            "{token} must appear exactly once"
        );
    }
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
        .flat_map(|t| {
            t.items
                .iter()
                .map(|i| (i.term.value.as_str(), first_paragraph(&i.children)))
        })
        .collect();
    assert_eq!(terms, [("Status", "accepted"), ("Owner", "someone")]);
    for item in find_terms(&cnd.nodes).iter().flat_map(|terms| terms.items.iter()) {
        assert_eq!(node_types(&item.children), ["paragraph"], "{:?}", item.term.value);
    }
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
    // The quote holds its paragraph and its list; the list is nowhere else.
    assert_eq!(node_types(&cnd.nodes), ["quote", "paragraph"]);
    let CndNode::Quote(quote) = &cnd.nodes[0] else { unreachable!() };
    assert_eq!(node_types(&quote.children), ["paragraph", "list"]);
    assert_eq!(paragraph_values(&quote.children), ["QW1 quote paragraph."]);
    let CndNode::List(list) = &quote.children[1] else { unreachable!() };
    assert_eq!(
        list.items.iter().map(item_text).collect::<Vec<_>>(),
        ["QW2 quote list item", "QW3 quote list item"]
    );
    assert_eq!(find_lists(&cnd.nodes).len(), 1, "the quote's list only");
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
            (value(&f.caption), code)
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
    // Each raw block is its figure's only child, and is not also emitted
    // beside it.
    for figure in &figures {
        assert_eq!(node_types(&figure.children), ["code"]);
        assert!(figure.raw.is_none());
    }
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert_eq!(stats.code, 3, "one code node per raw block");
    assert_eq!(node_types(&cnd.nodes), ["figure", "figure", "figure"]);
}

#[test]
fn a_helper_drawing_is_raw() {
    // A figure around a drawing built by a helper (an image, a grid of
    // shapes and a label, each positioned by `place`) is a drawing (spec
    // §6.7): its caption is kept, it has no children, its `raw` holds the
    // helper's source, and nothing laid out in it escapes beside it.
    let cnd = cnd_for_example("figure_helper_drawing.typ");
    let [CndNode::Figure(figure)] = cnd.nodes.as_slice() else {
        panic!("one figure, alone: {:?}", node_types(&cnd.nodes));
    };
    assert_eq!(value(&figure.caption), Some("HA2 caption"));
    assert!(figure.children.is_empty(), "{:?}", node_types(&figure.children));
    let raw = figure.raw.as_ref().expect("raw");
    assert_eq!(raw.format, "typst");
    // The body is the helper's value: its source is where the helper builds
    // it, in the `#let`.
    for part in
        ["place(", "image(\"newsletter/newsletter-cover.png\"", "HA1 callout label"]
    {
        assert!(raw.value.contains(part), "{part:?} in {:?}", raw.value);
    }
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert_eq!(stats.images, 0, "no bare image escapes the drawing");
    assert_eq!(stats.nodes, 1);
}

/// The `figure_kinds.typ` figure whose caption is `caption`.
fn figure_with_caption<'a>(
    cnd: &'a typst_cnd::Cnd,
    caption: &str,
) -> &'a typst_cnd::FigureNode {
    all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|n| match n {
            CndNode::Figure(f) if value(&f.caption) == Some(caption) => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a figure captioned {caption:?}"))
}

#[test]
fn theorem_figure_holds_its_paragraphs() {
    let cnd = cnd_for_example("figure_kinds.typ");
    let theorem = figure_with_caption(&cnd, "KA1 completeness");
    assert_eq!(theorem.kind.as_deref(), Some("theorem"));
    let numbering = theorem.numbering.as_ref().expect("numbering");
    assert_eq!(numbering.name.as_deref(), Some("Theorem"));
    assert_eq!(numbering.value, "1");
    assert_eq!(node_types(&theorem.children), ["paragraph", "paragraph"]);
    assert_eq!(
        paragraph_values(&theorem.children),
        [
            "Every bounded set of real numbers has a least upper bound.",
            "A second paragraph, beside Figure\u{a0}1.",
        ]
    );
    assert!(theorem.raw.is_none());
    // The body's edges sit on its own texts, spanned, and are not repeated
    // on the caption.
    let CndNode::Paragraph(second) = &theorem.children[1] else { unreachable!() };
    assert_eq!(second.text.refs.len(), 1);
    assert_eq!(second.text.refs[0].label, "fig-pair");
    let span = second.text.refs[0].text_span.as_ref().expect("a span");
    assert_eq!(codepoint_slice(&second.text.value, span), "Figure\u{a0}1");
    let caption = theorem.caption.as_ref().expect("caption");
    assert!(caption.refs.is_empty(), "{:?}", caption.refs);
    // Every node keeps its own location: the label resolves to the figure.
    assert!(matches!(
        find_by_label(&cnd.nodes, "thm-complete"),
        Some(CndNode::Figure(f)) if f.kind.as_deref() == Some("theorem")
    ));
    assert_refs_resolve(&cnd.nodes);
}

#[test]
fn figure_grid_is_a_layout_child() {
    let cnd = cnd_for_example("figure_kinds.typ");
    let pair = figure_with_caption(&cnd, "KB1 two covers");
    assert_eq!(node_types(&pair.children), ["layout"]);
    assert!(pair.raw.is_none());
    let CndNode::Layout(layout) = &pair.children[0] else { unreachable!() };
    let cells: Vec<(i32, i32, Vec<Option<&str>>)> = layout
        .cells
        .iter()
        .map(|cell| {
            let alts = cell
                .children
                .iter()
                .map(|child| match child {
                    CndNode::Image(image) => image.alt.as_deref(),
                    other => panic!("an image per cell, got {}", node_type(other)),
                })
                .collect();
            (cell.row, cell.col, alts)
        })
        .collect();
    assert_eq!(
        cells,
        [(0, 0, vec![Some("Left cover")]), (0, 1, vec![Some("Right cover")])]
    );
}

#[test]
fn subfigures_are_nested_figures() {
    let cnd = cnd_for_example("figure_kinds.typ");
    let outer = figure_with_caption(&cnd, "KC3 two parts");
    assert_eq!(node_types(&outer.children), ["figure", "figure"]);
    let parts: Vec<(Option<&str>, Option<&str>, Vec<&str>)> = outer
        .children
        .iter()
        .map(|child| {
            let CndNode::Figure(part) = child else { unreachable!() };
            (part.base.label.as_deref(), value(&part.caption), node_types(&part.children))
        })
        .collect();
    assert_eq!(
        parts,
        [
            (Some("fig-part-a"), Some("KC1 first part"), vec!["image"]),
            (Some("fig-part-b"), Some("KC2 second part"), vec!["image"]),
        ]
    );
    // Each subfigure is nested once, never also a sibling of its parent.
    let figures: Vec<_> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter(|n| matches!(n, CndNode::Figure(_)))
        .collect();
    assert_eq!(figures.len(), 8);
    let counts = word_counts(&cnd);
    for token in ["KC1", "KC2", "KC3"] {
        assert_eq!(counts.get(token).copied(), Some(1), "{token} once");
    }
}

#[test]
fn drawing_figure_has_raw_and_no_children() {
    let cnd = cnd_for_example("figure_kinds.typ");
    let drawing = figure_with_caption(&cnd, "KD1 a drawing");
    assert!(drawing.children.is_empty(), "{:?}", node_types(&drawing.children));
    let raw = drawing.raw.as_ref().expect("raw");
    assert_eq!(raw.format, "typst");
    assert!(raw.value.contains("rect("), "{:?}", raw.value);
    assert_eq!(raw.value, "rect(width: 2cm, height: 1cm)");
    // Only a drawing carries its source.
    let with_raw: Vec<_> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|n| match n {
            CndNode::Figure(f) if f.raw.is_some() => value(&f.caption),
            _ => None,
        })
        .collect();
    assert_eq!(with_raw, ["KD1 a drawing"]);
}

#[test]
fn inline_images_side_by_side_are_the_figure_children() {
    // Two boxed images on one line realize a paragraph holding only the
    // space between them: the images are nodes of their own, so that
    // paragraph has nothing left and is no child.
    let cnd = cnd_for_example("figure_kinds.typ");
    let pair = figure_with_caption(&cnd, "KF1 inline pair");
    assert_eq!(node_types(&pair.children), ["image", "image"]);
    let alts: Vec<Option<&str>> = pair
        .children
        .iter()
        .map(|child| match child {
            CndNode::Image(image) => image.alt.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(alts, [Some("Inline left"), Some("Inline right")]);
    assert!(pair.raw.is_none());
}

#[test]
fn a_floating_figure_keeps_its_body() {
    // A figure with a `placement` floats through a `place` its own show
    // rule writes inside it: the figure is not in its own body, and stays
    // in the list item it is written in.
    let cnd = cnd_for_example("figure_kinds.typ");
    let floating = figure_with_caption(&cnd, "KE3 floating caption");
    assert_eq!(floating.base.label.as_deref(), Some("fig-float"));
    assert_eq!(paragraph_values(&floating.children), ["KE2 floating body."]);
    let lists = find_lists(&cnd.nodes);
    let [list] = lists.as_slice() else { panic!("one list: {}", lists.len()) };
    assert_eq!(node_types(&list.items[0].children), ["paragraph", "figure"]);
    let counts = word_counts(&cnd);
    for token in ["KE1", "KE2", "KE3"] {
        assert_eq!(counts.get(token).copied(), Some(1), "{token} once");
    }
}

/// The paragraph texts anywhere in the tree, in reading order.
fn every_paragraph_text(nodes: &[CndNode]) -> Vec<&str> {
    all_nodes(nodes)
        .into_iter()
        .filter_map(|n| match n {
            CndNode::Paragraph(p) => Some(p.text.value.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_boxed_canvas_with_placed_labels_is_raw() {
    // The labels placed in a box are part of the box's paragraph, yet they
    // are positioned pieces of a drawing: they are not lifted out as text.
    let cnd = cnd_for_example("figure_drawings.typ");
    let canvas = figure_with_caption(&cnd, "DA3 boxed canvas");
    assert!(canvas.children.is_empty(), "{:?}", node_types(&canvas.children));
    let raw = canvas.raw.as_ref().expect("raw");
    for part in ["place(line(", "DA1 node start", "DA2 node end"] {
        assert!(raw.value.contains(part), "{part:?} in {:?}", raw.value);
    }
    let paragraphs = every_paragraph_text(&cnd.nodes);
    assert!(
        paragraphs.iter().all(|p| !p.contains("DA1") && !p.contains("DA2")),
        "{paragraphs:?}"
    );
}

#[test]
fn a_boxed_shape_is_a_drawing_and_empty_paragraphs_are_no_nodes() {
    let cnd = cnd_for_example("figure_drawings.typ");
    let shape = figure_with_caption(&cnd, "DB1 boxed shape");
    assert!(shape.children.is_empty(), "{:?}", node_types(&shape.children));
    assert_eq!(
        shape.raw.as_ref().map(|raw| raw.value.as_str()),
        Some("box(rect(width: 1cm, height: 1cm))")
    );
    // A boxed shape alone in the flow realizes a paragraph with no text: it
    // is no node, and the paragraphs around it are kept.
    let paragraphs = every_paragraph_text(&cnd.nodes);
    assert!(paragraphs.iter().all(|p| !p.trim().is_empty()), "{paragraphs:?}");
    assert_eq!(
        paragraph_texts_in_order(&cnd.nodes),
        ["DC1 before the shape.", "DC2 after the shape."]
    );
}

#[test]
fn a_figure_placed_by_a_show_rule_is_no_drawing() {
    // `show figure: it => place(center, it)` lays the figure out in a
    // `place` that is not written in its body: it keeps its children.
    let cnd = cnd_for_example("figure_drawings.typ");
    let framed = figure_with_caption(&cnd, "DD2 framed");
    assert_eq!(framed.kind.as_deref(), Some("framed"));
    assert_eq!(paragraph_values(&framed.children), ["DD1 placed by a rule."]);
    assert!(framed.raw.is_none());
}

/// Content wrapped in `pdf.artifact` is decorative and never reaches the CND
/// (cnd-sdk spec §2, "Decorative content"): no node of any kind, no `refs`
/// edge to a label inside it, no footnote pool entry or edge for a footnote
/// written inside it — not even through a kept re-reference — and no
/// footnote or citation edge from a marker written inside it.
#[test]
fn decorative_content_is_never_emitted() {
    let cnd = cnd_for_example("decorative_content.typ");
    assert_pool_refs_resolve(&cnd);

    let json = serde_json::to_string(&cnd).unwrap();
    assert!(
        !json.to_lowercase().contains("ornament"),
        "decorative content reached the CND: {json}"
    );

    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    for expected in ["Kept paragraph.", "Inline word kept."] {
        assert!(
            paragraphs.iter().any(|p| p == expected),
            "kept paragraph {expected:?} missing: {paragraphs:?}"
        );
    }
    assert!(
        paragraphs.iter().any(|p| p.starts_with("Kept text refers to")),
        "the paragraph referring to decorative labels is kept: {paragraphs:?}"
    );

    let notes: Vec<String> = cnd.footnotes.iter().map(footnote_text).collect();
    assert_eq!(notes, ["Kept note."], "only the kept footnote is pooled");
    assert_eq!(
        cnd.footnotes[0].label, "3",
        "a kept footnote keeps the ordinal the page shows"
    );
    let mut edges = Vec::new();
    collect_footnote_labels(&cnd.nodes, &mut edges);
    assert_eq!(
        edges,
        ["3"],
        "one footnote edge, from the kept marker; none from decorative markers"
    );

    // `refs` labels and `links` hrefs are part of the JSON checked above, so
    // a decorative `@kept-fig` or `#link(..)` would surface as an edge to
    // "kept-fig" here: the only node pointing at the kept figure is none.
    assert_eq!(
        incoming_count(&cnd.nodes, "kept-fig"),
        0,
        "a reference or link written in decorative content adds no edge"
    );
    assert!(
        paragraphs.iter().any(|p| p == "Kept reused words."),
        "content reused in a decorative float stays where it is kept: {paragraphs:?}"
    );
    assert!(
        paragraphs.iter().any(|p| p == "Kept after the decorative block."),
        "the paragraph after a decorative block is kept: {paragraphs:?}"
    );

    let cites = cites_of_paragraph(&cnd.nodes, "Kept cites")
        .expect("the citing paragraph is kept");
    assert_eq!(cites, ["smith2024"], "a decorative citation adds no edge");
}

fn cites_of_paragraph(nodes: &[CndNode], prefix: &str) -> Option<Vec<String>> {
    all_nodes(nodes).into_iter().find_map(|node| match node {
        CndNode::Paragraph(p) if p.text.value.starts_with(prefix) => {
            Some(p.text.cites.iter().map(|c| c.label.clone()).collect())
        }
        _ => None,
    })
}

fn collect_footnote_labels(nodes: &[CndNode], out: &mut Vec<String>) {
    for text in all_texts(nodes) {
        out.extend(text.footnotes.iter().map(|r| r.label.clone()));
    }
}

/// A decorative float whose `place` comes from a helper must not take kept
/// text with it, nor leak: its source range spans the paragraphs written
/// between the helper and the call, and content reused from a `#let` shares
/// its spans. A float nested in a decorative one is decorative too, and so is
/// a heading a show rule wraps in an artifact.
#[test]
fn decorative_float_helpers_never_drop_kept_text() {
    let cnd = cnd_for_example("decorative_float_helpers.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    for expected in [
        "Kept alpha paragraph.",
        "Kept beta paragraph.",
        "Kept gamma paragraph.",
        "Kept aside words.",
        "Kept delta between.",
        "Kept end.",
    ] {
        assert!(
            paragraphs.iter().any(|p| p == expected),
            "kept paragraph {expected:?} lost: {paragraphs:?}"
        );
    }
    let json = serde_json::to_string(&cnd).unwrap();
    for kept in ["Kept list item", "Kept reused item", "Kept reused cell"] {
        assert!(json.contains(kept), "kept content {kept:?} lost: {json}");
    }
    assert_eq!(
        json.matches("Kept reused item").count(),
        1,
        "the decorative float's copy of a `#let` is dropped, the kept one stays"
    );
    assert_eq!(
        json.matches("Shared item").count(),
        1,
        "a `#let` in a kept float and in a decorative block appears once"
    );
    assert!(!json.contains("Ornament"), "decorative content leaked: {json}");
}

/// A kept float drawn while a multi-page artifact is still open in the
/// frames stays kept, and content written inside a decorative float but
/// shown in the flow through `metadata` or `state` is kept where it shows.
#[test]
fn decorative_multipage_keeps_flow_and_kept_floats() {
    let cnd = cnd_for_example("decorative_multipage.typ");
    let paragraphs = paragraph_texts_in_order(&cnd.nodes);
    for expected in [
        "Kept intro.",
        "Kept via metadata query.",
        "Kept via state.",
        "Kept float paragraph written after.",
        "Kept end.",
    ] {
        assert!(
            paragraphs.iter().any(|p| p == expected),
            "kept paragraph {expected:?} lost: {paragraphs:?}"
        );
    }
    let json = serde_json::to_string(&cnd).unwrap();
    for kept in ["Kept context float item", "Kept context cell"] {
        assert!(json.contains(kept), "kept float content {kept:?} lost: {json}");
    }
    assert!(!json.contains("Ornament"), "decorative content leaked: {json}");
}

#[test]
fn the_paged_layout_is_kept_for_export() {
    // The CND is read from a paged layout; the document keeps it, so a PDF
    // or a page image can be exported without laying it out again.
    assert!(!compile_example("minimal.typ").paged().pages().is_empty());
    assert!(compile_example("complex_multipage.typ").paged().pages().len() > 1);
}

/// Every example's CND, compiled once for the structural tests below.
fn example_cnds() -> &'static [(String, typst_cnd::Cnd)] {
    static CNDS: std::sync::OnceLock<Vec<(String, typst_cnd::Cnd)>> =
        std::sync::OnceLock::new();
    CNDS.get_or_init(|| {
        let dir = common::examples_dir();
        let mut out: Vec<(String, typst_cnd::Cnd)> = all_example_files()
            .into_iter()
            .map(|path| {
                let name = path.strip_prefix(&dir).unwrap_or(&path).to_string_lossy();
                let name = name.replace('\\', "/");
                let cnd = cnd_for_example(&name);
                (name, cnd)
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    })
}

/// Every example's CND as the JSON this producer writes.
fn example_json() -> Vec<(String, serde_json::Value)> {
    example_cnds()
        .iter()
        .map(|(name, cnd)| {
            let json = cnd_to_json(cnd).expect("serialize");
            (name.clone(), serde_json::from_str(&json).expect("parse"))
        })
        .collect()
}

/// Walk a CND's JSON structurally, calling `f(path, parent key, object,
/// is_node)` on every object. A node is an element of an array under
/// `nodes` or `children`. Free-form objects (`state_metadata`, `fields`)
/// are never entered.
fn walk_json(
    value: &serde_json::Value,
    path: &str,
    key: &str,
    in_node_list: bool,
    f: &mut impl FnMut(&str, &str, &serde_json::Map<String, serde_json::Value>, bool),
) {
    match value {
        serde_json::Value::Object(object) => {
            f(path, key, object, in_node_list);
            for (k, v) in object {
                if k == "state_metadata" || k == "fields" {
                    continue;
                }
                walk_json(v, &format!("{path}.{k}"), k, false, f);
            }
        }
        serde_json::Value::Array(items) => {
            let nodes = key == "nodes" || key == "children";
            for (i, item) in items.iter().enumerate() {
                walk_json(item, &format!("{path}[{i}]"), key, nodes, f);
            }
        }
        _ => {}
    }
}

#[test]
fn every_node_has_a_page() {
    for (name, cnd) in example_cnds() {
        for node in all_nodes_full(cnd) {
            assert!(
                node.base().location.page >= 1,
                "{name}: node {} has no page: {node:?}",
                node.id()
            );
        }
    }
}

#[test]
fn cnd_version_is_0_5_0() {
    for (name, cnd) in example_cnds() {
        assert_eq!(cnd.cnd_version, "0.5.0", "{name}");
    }
}

/// No key the 0.5 node model removed is written.
#[test]
fn no_removed_04_keys() {
    for (name, json) in example_json() {
        walk_json(&json, "", "", false, &mut |path, key, object, is_node| {
            for removed in ["tight", "block", "counter_label", "content_kind"] {
                assert!(!object.contains_key(removed), "{name}: {path} has {removed:?}");
            }
            let kind = object.get("type").and_then(|t| t.as_str());
            if is_node && matches!(kind, Some("heading" | "figure" | "math")) {
                assert!(!object.contains_key("number"), "{name}: {path} has \"number\"");
            }
            if key == "items" && object.contains_key("term") {
                assert!(
                    !object.contains_key("description"),
                    "{name}: {path} is a terms item with \"description\""
                );
            }
            if is_node {
                for family in ["refs", "cites", "footnotes", "links"] {
                    assert!(
                        !object.contains_key(family),
                        "{name}: node {path} has {family:?}; edges live on texts"
                    );
                }
            }
        });
    }
}

/// Every default-valued field is omitted, as the reference writer does
/// (spec §6, *Defaults*). Required fields are written even when empty: of
/// the list and object fields, that is `heading_path` and the top-level
/// `nodes` (`schema/cnd.schema.json`).
#[test]
fn defaults_are_omitted() {
    fn is_empty(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Array(items) => items.is_empty(),
            serde_json::Value::Object(object) => object.is_empty(),
            _ => false,
        }
    }
    for (name, json) in example_json() {
        walk_json(&json, "", "", false, &mut |path, _, object, is_node| {
            let required: &[&str] = if path.is_empty() {
                &["nodes"]
            } else if is_node
                && object.get("type").and_then(|t| t.as_str()) == Some("heading")
            {
                &["heading_path"]
            } else {
                &[]
            };
            for (key, value) in object {
                let at = format!("{name}: {path}.{key}");
                assert!(!value.is_null(), "{at} is null");
                if !required.contains(&key.as_str()) {
                    assert!(!is_empty(value), "{at} is {value}");
                }
                if let serde_json::Value::Array(items) = value {
                    for (i, item) in items.iter().enumerate() {
                        assert!(!is_empty(item), "{at}[{i}] is {item}");
                    }
                }
                match key.as_str() {
                    "ordered" | "is_header" => {
                        assert_ne!(value, &serde_json::Value::Bool(false), "{at}");
                    }
                    "rowspan" | "colspan" => {
                        assert_ne!(value, &serde_json::json!(1), "{at}");
                    }
                    _ => {}
                }
            }
        });
    }
}

// Container slots (CND 0.5 §6): a list item's or a term's body is the
// realized blocks laid out in it, each a node of its own.

/// Every term list of the document, every slot entered.
fn find_terms(nodes: &[CndNode]) -> Vec<&typst_cnd::TermsNode> {
    all_nodes(nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Terms(terms) => Some(terms),
            _ => None,
        })
        .collect()
}

/// The paragraph that is the `index`-th child of `nodes`.
fn paragraph_at(nodes: &[CndNode], index: usize) -> &typst_cnd::ParagraphNode {
    match &nodes[index] {
        CndNode::Paragraph(paragraph) => paragraph,
        other => panic!("child {index} is a {}, not a paragraph", node_type(other)),
    }
}

#[test]
fn list_item_children_are_realized_blocks() {
    let cnd = cnd_for_example("containers_lists.typ");
    let lists = outer_lists(&cnd.nodes);
    let bullets = list_starting_with(&lists, "First item.");
    assert!(!bullets.ordered);
    assert_eq!(
        item_summary(&bullets.items),
        [
            ("First item.", None),
            ("Second item, first paragraph.", None),
            ("Third item ends with a note.1", None),
        ]
    );
    assert_eq!(node_types(&bullets.items[0].children), ["paragraph"]);
    assert_eq!(node_types(&bullets.items[2].children), ["paragraph"]);

    let second = &bullets.items[1];
    assert_eq!(node_types(&second.children), ["paragraph", "paragraph", "list"]);
    assert_eq!(
        paragraph_values(&second.children),
        ["Second item, first paragraph.", "Second item, second paragraph."]
    );
    let CndNode::List(nested) = &second.children[2] else { unreachable!() };
    assert!(nested.ordered);
    assert_eq!(
        item_summary(&nested.items),
        [("Nested five.", Some(5)), ("Nested six.", Some(6))]
    );
    for item in &nested.items {
        assert_eq!(node_types(&item.children), ["paragraph"]);
    }
}

#[test]
fn nested_list_keeps_its_own_kind() {
    let cnd = cnd_for_example("containers_lists.typ");
    let lists = outer_lists(&cnd.nodes);
    let enumeration = list_starting_with(&lists, "Enum item with bullets:");
    assert!(enumeration.ordered);
    assert_eq!(
        item_summary(&enumeration.items),
        [("Enum item with bullets:", Some(1)), ("Second enum item.", Some(2))]
    );
    let first = &enumeration.items[0];
    assert_eq!(node_types(&first.children), ["paragraph", "list"]);
    let sub = nested_lists(first);
    assert_eq!(sub.len(), 1);
    assert!(!sub[0].ordered, "the sub-list under the enum item is a bullet list");
    assert_eq!(
        item_summary(&sub[0].items),
        [("Inner bullet one.", None), ("Inner bullet two.", None)]
    );
}

#[test]
fn term_item_splits_term_and_definition() {
    let cnd = cnd_for_example("containers_lists.typ");
    let terms = find_terms(&cnd.nodes);
    assert_eq!(terms.len(), 1);
    let items = &terms[0].items;
    assert_eq!(items.len(), 2);

    let latency = &items[0];
    assert_eq!(latency.term.value, "Latency");
    assert_eq!(node_types(&latency.children), ["paragraph", "paragraph"]);
    let values = paragraph_values(&latency.children);
    assert_eq!(values, ["The time a request waits.", "It is measured end to end [1]."]);
    for value in values {
        assert!(
            !value.contains("Latency") && !value.contains(':'),
            "the term and its separator stay out of the definition: {value:?}"
        );
    }

    assert_eq!(items[1].term.value, "Throughput");
    assert_eq!(paragraph_values(&items[1].children), ["The work done per second."]);
}

#[test]
fn edges_inside_containers_sit_on_the_inner_paragraph() {
    let cnd = cnd_for_example("containers_lists.typ");

    // The footnote marker ends the third item: its edge is on that item's
    // paragraph, spanning the rendered marker digit.
    let lists = outer_lists(&cnd.nodes);
    let bullets = list_starting_with(&lists, "First item.");
    let third = paragraph_at(&bullets.items[2].children, 0);
    assert_eq!(third.text.footnotes.len(), 1);
    let note = &third.text.footnotes[0];
    let span = note.text_span.as_ref().expect("the footnote edge has a span");
    assert_eq!(codepoint_slice(&third.text.value, span), "1");
    assert_eq!(cnd.footnotes.len(), 1);
    assert_eq!(cnd.footnotes[0].label, note.label);

    // The citation sits on the definition's second paragraph.
    let terms = find_terms(&cnd.nodes);
    let cited = paragraph_at(&terms[0].items[0].children, 1);
    assert_eq!(cited.text.cites.len(), 1);
    let cite = &cited.text.cites[0];
    assert_eq!(cite.label, "smith2024");
    let span = cite.text_span.as_ref().expect("the citation edge has a span");
    assert_eq!(codepoint_slice(&cited.text.value, span), "[1]");

    // Each edge exists once in the whole document: no copy is left on a
    // container.
    let texts = all_texts(&cnd.nodes);
    assert_eq!(texts.iter().map(|text| text.footnotes.len()).sum::<usize>(), 1);
    assert_eq!(texts.iter().map(|text| text.cites.len()).sum::<usize>(), 1);
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn list_markers_are_not_paragraphs() {
    for name in ["containers_lists.typ", "nested_lists.typ"] {
        let cnd = cnd_for_example(name);
        for node in all_nodes_full(&cnd) {
            if let CndNode::Paragraph(paragraph) = node {
                let value = paragraph.text.value.as_str();
                assert!(
                    !["•", "‣", "–", "1.", "2.", "5.", "6."].contains(&value),
                    "{name}: a list marker became a paragraph: {value:?}"
                );
            }
        }
    }
}

/// The first heading of the tree, in reading order, whose text is `text`.
fn find_heading<'a>(nodes: &'a [CndNode], text: &str) -> &'a typst_cnd::HeadingNode {
    all_nodes(nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Heading(heading) if heading.text.value == text => Some(heading),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no heading {text:?}"))
}

/// The cells of a table or a layout.
fn cells_of(node: &CndNode) -> &[typst_cnd::Cell] {
    match node {
        CndNode::Table(table) => &table.cells,
        CndNode::Layout(layout) => &layout.cells,
        other => panic!("a {} has no cells", node_type(other)),
    }
}

/// How many times `needle` occurs over every text of the CND: the body,
/// every slot entered, and the footnote pool.
fn occurrences(cnd: &typst_cnd::Cnd, needle: &str) -> usize {
    all_nodes_full(cnd)
        .into_iter()
        .flat_map(|node| node.texts())
        .map(|text| text.value.matches(needle).count())
        .sum()
}

#[test]
fn table_cells_hold_blocks() {
    let cnd = cnd_for_example("containers_grids.typ");
    let outer = find_heading(&cnd.nodes, "Outer");
    let table = outer
        .children
        .iter()
        .find(|node| matches!(node, CndNode::Table(_)))
        .expect("the 2×2 table under Outer");
    let cells = cells_of(table);
    assert_eq!(cells.len(), 4);
    assert_eq!(
        cells
            .iter()
            .map(|cell| (cell.row, cell.col, cell.is_header))
            .collect::<Vec<_>>(),
        [(0, 0, true), (0, 1, true), (1, 0, false), (1, 1, false)]
    );
    assert_eq!(paragraph_values(&cells[0].children), ["Notes"]);
    assert_eq!(node_types(&cells[2].children), ["paragraph", "paragraph"]);
    assert_eq!(
        paragraph_values(&cells[2].children),
        ["First paragraph of the alpha notes.", "Second paragraph of the alpha notes."]
    );
    assert_eq!(node_types(&cells[3].children), ["image"]);
    let CndNode::Image(image) = &cells[3].children[0] else { unreachable!() };
    assert_eq!(image.alt.as_deref(), Some("Alpha cover"));
    // The image is the cell's only copy: none escapes beside the table.
    let images = all_nodes(&cnd.nodes)
        .into_iter()
        .filter(|node| matches!(node, CndNode::Image(_)))
        .count();
    assert_eq!(images, 1);
}

#[test]
fn bare_grid_is_a_layout() {
    let cnd = cnd_for_example("containers_grids.typ");
    let outer = find_heading(&cnd.nodes, "Outer");
    let left = "Left block of the bare grid.";
    let right = "Right block of the bare grid.";
    // Outer holds two grids: this one and the sectioned one. Exactly one
    // layout holds the bare grid's blocks.
    let bare: Vec<&CndNode> = outer
        .children
        .iter()
        .filter(|node| {
            matches!(node, CndNode::Layout(_))
                && cells_of(node)
                    .iter()
                    .any(|cell| first_paragraph(&cell.children) == left)
        })
        .collect();
    assert_eq!(bare.len(), 1, "one layout for the bare grid");
    let cells = cells_of(bare[0]);
    assert_eq!(cells.len(), 2);
    assert_eq!(
        cells.iter().map(|cell| (cell.row, cell.col)).collect::<Vec<_>>(),
        [(0, 0), (0, 1)]
    );
    assert_eq!(paragraph_values(&cells[0].children), [left]);
    assert_eq!(paragraph_values(&cells[1].children), [right]);
    for text in [left, right] {
        assert_eq!(occurrences(&cnd, text), 1, "{text:?} is kept once");
    }
    // Each grid of the document is one layout; none is a table.
    let mut stats = NodeStats::default();
    walk_nodes(&cnd.nodes, &mut stats);
    assert_eq!(stats.layouts, 2);
}

#[test]
fn heading_in_grid_cell_stays_in_cell() {
    let cnd = cnd_for_example("containers_grids.typ");
    let outer = find_heading(&cnd.nodes, "Outer");
    let sectioned = outer
        .children
        .iter()
        .find(|node| {
            matches!(node, CndNode::Layout(_))
                && cells_of(node).iter().any(|cell| {
                    matches!(cell.children.first(), Some(CndNode::Heading(_)))
                })
        })
        .expect("the sectioned grid under Outer");
    let cells = cells_of(sectioned);
    assert_eq!(node_types(&cells[0].children), ["heading"]);
    let CndNode::Heading(inner) = &cells[0].children[0] else { unreachable!() };
    assert_eq!(inner.text.value, "Inner");
    assert_eq!(inner.heading_path, ["Outer", "Inner"]);
    assert_eq!(node_types(&inner.children), ["paragraph"]);
    assert_eq!(paragraph_values(&inner.children), ["Paragraph under the inner heading."]);
    assert_eq!(
        paragraph_values(&cells[1].children),
        ["Second cell of the sectioned grid."]
    );
    // The heading's section ends with its cell: the paragraph after the
    // grid belongs to Outer.
    assert!(
        paragraph_values(&outer.children).contains(&"After the sectioned grid."),
        "the paragraph after the grid is Outer's"
    );
    assert_eq!(occurrences(&cnd, "After the sectioned grid."), 1);
    assert_eq!(occurrences(&cnd, "Paragraph under the inner heading."), 1);
}

#[test]
fn edge_inside_a_cell_sits_on_the_cell_paragraph() {
    let cnd = cnd_for_example("containers_grids.typ");
    let related = find_heading(&cnd.nodes, "Related");
    let table = related
        .children
        .iter()
        .find(|node| matches!(node, CndNode::Table(_)))
        .expect("the table under Related");
    assert!(table.texts().is_empty(), "a table has no texts");
    let cells = cells_of(table);
    assert_eq!(node_types(&cells[1].children), ["paragraph"]);
    let paragraph = paragraph_at(&cells[1].children, 0);
    let value = paragraph.text.value.as_str();
    assert!(value.starts_with("Cell text pointing to"), "{value:?}");

    assert_eq!(paragraph.text.refs.len(), 1);
    let reference = &paragraph.text.refs[0];
    assert_eq!(reference.label, "grid-note");
    let span = reference.text_span.as_ref().expect("the ref edge has a span");
    assert_eq!(codepoint_slice(value, span), "Section\u{a0}1");

    assert_eq!(paragraph.text.footnotes.len(), 1);
    let note = &paragraph.text.footnotes[0];
    let span = note.text_span.as_ref().expect("the footnote edge has a span");
    assert_eq!(codepoint_slice(value, span), note.label);

    // No other node carries the edge: not the heading of the section, nor
    // any paragraph of it.
    for node in all_nodes(&cnd.nodes) {
        if node.id() == paragraph.base.id {
            continue;
        }
        for text in node.texts() {
            assert!(
                text.refs.iter().all(|r| r.label != "grid-note"),
                "a {} also refers to grid-note: {:?}",
                node_type(node),
                text.refs
            );
        }
    }
    let texts = all_texts(&cnd.nodes);
    let edges_to_note = texts
        .iter()
        .flat_map(|text| &text.footnotes)
        .filter(|edge| edge.label == note.label)
        .count();
    assert_eq!(edges_to_note, 1);
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn a_list_in_a_cell_is_kept_once_whatever_holds_the_table() {
    let cnd = cnd_for_example("containers_grids.typ");
    for text in [
        "Quoted cell item one.",
        "Quoted cell item two.",
        "Noted cell item one.",
        "Noted cell item two.",
        "Itemized cell entry one.",
        "Itemized cell entry two.",
    ] {
        assert_eq!(occurrences(&cnd, text), 1, "{text:?} is kept once");
    }

    // A table in a list item is a block of that item, and its cell holds
    // the list.
    let lists = outer_lists(&cnd.nodes);
    let holder = list_starting_with(&lists, "Item holding a table:");
    let item = &holder.items[0];
    assert_eq!(node_types(&item.children), ["paragraph", "table"]);
    let cells = cells_of(&item.children[1]);
    assert_eq!(cells.len(), 1);
    assert_eq!(node_types(&cells[0].children), ["list"]);
    let CndNode::List(list) = &cells[0].children[0] else { unreachable!() };
    assert_eq!(
        list.items.iter().map(item_text).collect::<Vec<_>>(),
        ["Itemized cell entry one.", "Itemized cell entry two."]
    );

    // A table in a block quote is a block of that quote, and one in a
    // footnote a block of the note: in each, the cell holds the list.
    let in_cell_list = |children: &[CndNode]| -> Vec<String> {
        assert_eq!(node_types(children), ["table"]);
        let cells = cells_of(&children[0]);
        assert_eq!(cells.len(), 1);
        assert_eq!(node_types(&cells[0].children), ["list"]);
        let CndNode::List(list) = &cells[0].children[0] else { unreachable!() };
        list.items.iter().map(|item| item_text(item).to_string()).collect()
    };
    let quote = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Quote(quote) => Some(quote),
            _ => None,
        })
        .expect("the block quote");
    assert_eq!(
        in_cell_list(&quote.children),
        ["Quoted cell item one.", "Quoted cell item two."]
    );
    let note = cnd
        .footnotes
        .iter()
        .find(|note| footnote_text(note).contains("Noted"))
        .expect("the footnote holding a table");
    assert_eq!(
        in_cell_list(&note.children),
        ["Noted cell item one.", "Noted cell item two."]
    );

    // No table of the body holds the note's list: the 2×2 table, the
    // Related table, the quote's table and the item's table.
    let tables: Vec<&CndNode> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter(|node| matches!(node, CndNode::Table(_)))
        .collect();
    for table in &tables {
        for cell in cells_of(table) {
            for node in all_nodes(&cell.children) {
                for text in node.texts() {
                    assert!(!text.value.contains("Noted"), "{:?}", text.value);
                }
            }
        }
    }
    assert_eq!(tables.len(), 4);
}

#[test]
fn an_inline_list_in_a_box_is_kept_once() {
    let cnd = cnd_for_example("containers_grids.typ");
    assert_eq!(occurrences(&cnd, "Boxed list entry."), 1);
    let sentence = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value.starts_with("A sentence with") => {
                Some(p.text.value.as_str())
            }
            _ => None,
        })
        .expect("the sentence holding the box");
    assert!(sentence.contains("Boxed list entry."), "{sentence:?}");
    // The boxed list is the sentence's text, not a list of its own: the only
    // lists are the one in the quote's cell, the item holding a table and
    // the list in its cell (the note's list is in the pool), and no item is
    // left empty.
    let lists = find_lists(&cnd.nodes);
    assert_eq!(
        lists.iter().map(|list| item_text(&list.items[0])).collect::<Vec<_>>(),
        ["Quoted cell item one.", "Item holding a table:", "Itemized cell entry one."]
    );
    for list in lists {
        for item in &list.items {
            assert!(!item.children.is_empty(), "an empty item in {list:?}");
        }
    }
}

/// The table of a figure, by the figure's label.
fn figure_table<'a>(nodes: &'a [CndNode], label: &str) -> &'a typst_cnd::TableNode {
    let Some(CndNode::Figure(figure)) = find_by_label(nodes, label) else {
        panic!("no figure {label}");
    };
    figure
        .children
        .iter()
        .find_map(|child| match child {
            CndNode::Table(table) => Some(table),
            _ => None,
        })
        .unwrap_or_else(|| panic!("figure {label} holds no table"))
}

/// Ported from the unit test of the removed source-text caption parser: a
/// table figure's caption is the element's own, exactly.
#[test]
fn parse_caption_from_figure_block() {
    let cnd = cnd_for_example("structured.typ");
    let Some(CndNode::Figure(figure)) = find_by_label(&cnd.nodes, "tab-params-nominaux")
    else {
        panic!("expected the table figure");
    };
    assert_eq!(value(&figure.caption), Some("Paramètres nominaux de fonctionnement."));
    assert!(!figure_table(&cnd.nodes, "tab-params-nominaux").cells.is_empty());
}

#[test]
fn a_table_across_pages_keeps_each_cell_once() {
    let cnd = cnd_for_example("tables_across_pages.typ");
    // The header and footer repeat on every page the table spans; each is
    // one cell, once.
    for text in ["Header key", "Header value", "Footer key", "Footer value"] {
        assert_eq!(occurrences(&cnd, text), 1, "{text:?} is kept once");
    }
    for i in 1..=8 {
        for text in [format!("Body key {i}"), format!("Body value {i}")] {
            assert_eq!(occurrences(&cnd, &text), 1, "{text:?} is kept once");
        }
    }

    let section = find_heading(&cnd.nodes, "Long table");
    assert_eq!(node_types(&section.children), ["table"]);
    let cells = cells_of(&section.children[0]);
    assert_eq!(cells.len(), 2 * 11);
    assert_eq!(
        cells[..2]
            .iter()
            .map(|cell| (
                cell.row,
                cell.col,
                cell.is_header,
                first_paragraph(&cell.children)
            ))
            .collect::<Vec<_>>(),
        [(0, 0, true, "Header key"), (0, 1, true, "Header value")]
    );
    assert!(cells[2..].iter().all(|cell| !cell.is_header));
    let first_page = section.children[0].base().location.page;
    for cell in &cells[..2] {
        assert_eq!(cell.children[0].base().location.page, first_page);
    }

    // The row too tall for one page: each of its cells holds all of its
    // content, in order, though layout draws it outside the cells' tags and
    // both columns continue on the next page.
    let tall = &cells[2 * 9..2 * 10];
    for (cell, prefix, count, suffix) in [
        (&tall[0], "Left paragraph", 8, "of the broken row."),
        (&tall[1], "Tall paragraph", 12, "of the broken cell."),
    ] {
        let expected: Vec<String> =
            (1..=count).map(|i| format!("{prefix} {i} {suffix}")).collect();
        assert_eq!(paragraph_values(&cell.children), expected);
        let pages: Vec<i32> =
            cell.children.iter().map(|node| node.base().location.page).collect();
        assert!(
            pages.last() > pages.first() && *pages.first().unwrap() > first_page,
            "{prefix}: the row breaks across pages: {pages:?}"
        );
    }
    assert_eq!(
        cells[2 * 10..]
            .iter()
            .map(|cell| (cell.row, first_paragraph(&cell.children)))
            .collect::<Vec<_>>(),
        [(10, "Footer key"), (10, "Footer value")]
    );
}

#[test]
fn table_cells_carry_spans_and_a_multi_row_header() {
    let cnd = cnd_for_example("tables_across_pages.typ");
    let section = find_heading(&cnd.nodes, "Spans");
    let cells = cells_of(&section.children[0]);
    assert_eq!(
        cells
            .iter()
            .map(|cell| (
                cell.row,
                cell.col,
                cell.rowspan,
                cell.colspan,
                cell.is_header,
                first_paragraph(&cell.children),
            ))
            .collect::<Vec<_>>(),
        [
            (0, 0, 1, 3, true, "Top header"),
            (1, 0, 1, 1, true, "Sub one"),
            (1, 1, 1, 1, true, "Sub two"),
            (1, 2, 1, 1, true, "Sub three"),
            (2, 0, 2, 1, false, "Tall span"),
            (2, 1, 1, 1, false, "Row one middle"),
            (2, 2, 1, 1, false, "Row one end"),
            (3, 1, 1, 2, false, "Wide span"),
        ]
    );
}

/// The paragraph values of each cell of the only table or layout under the
/// heading `section`.
fn cell_paragraphs(cnd: &typst_cnd::Cnd, section: &str) -> Vec<Vec<String>> {
    let heading = find_heading(&cnd.nodes, section);
    let grid = heading
        .children
        .iter()
        .find(|node| matches!(node, CndNode::Table(_) | CndNode::Layout(_)))
        .unwrap_or_else(|| panic!("no table under {section:?}"));
    cells_of(grid)
        .iter()
        .map(|cell| {
            paragraph_values(&cell.children)
                .into_iter()
                .map(String::from)
                .collect()
        })
        .collect()
}

fn numbered(prefix: &str, count: usize) -> Vec<String> {
    (1..=count).map(|i| format!("{prefix} {i}.")).collect()
}

#[test]
fn a_broken_row_survives_empty_header_and_footer_cells() {
    let cnd = cnd_for_example("tables_across_pages.typ");
    assert_eq!(
        cell_paragraphs(&cnd, "Empty header cell"),
        [
            vec![],
            vec!["Header right".to_string()],
            numbered("Empty header left", 11),
            numbered("Empty header right", 13),
        ]
    );
    assert_eq!(
        cell_paragraphs(&cnd, "Empty footer cell"),
        [
            numbered("Empty footer left", 11),
            numbered("Empty footer right", 13),
            vec!["Footer total".to_string()],
            vec![],
        ]
    );
    // Every paragraph of the two rows is kept, once.
    for (prefix, count) in [
        ("Empty header left", 11),
        ("Empty header right", 13),
        ("Empty footer left", 11),
        ("Empty footer right", 13),
    ] {
        for text in numbered(prefix, count) {
            assert_eq!(occurrences(&cnd, &text), 1, "{text:?} is kept once");
        }
    }
}

#[test]
fn a_broken_row_in_a_grid_stays_in_its_cell() {
    let cnd = cnd_for_example("tables_across_pages.typ");
    assert_eq!(
        cell_paragraphs(&cnd, "Broken grid"),
        [vec!["Short grid cell.".to_string()], numbered("Grid paragraph", 13)]
    );
    for text in numbered("Grid paragraph", 13) {
        assert_eq!(occurrences(&cnd, &text), 1, "{text:?} is kept once");
    }
    // The grid breaks across pages, and the paragraph after it is the
    // section's, not the grid's.
    let section = find_heading(&cnd.nodes, "Broken grid");
    assert_eq!(node_types(&section.children), ["layout", "paragraph"]);
    let cells = cells_of(&section.children[0]);
    let pages: Vec<i32> = cells[1]
        .children
        .iter()
        .map(|node| node.base().location.page)
        .collect();
    assert!(pages.last() > pages.first(), "the grid breaks: {pages:?}");
}

/// The block quotes of the body, in reading order.
fn block_quotes(nodes: &[CndNode]) -> Vec<&typst_cnd::QuoteNode> {
    all_nodes(nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Quote(quote) => Some(quote),
            _ => None,
        })
        .collect()
}

/// The pool entry whose content mentions `needle`.
fn footnote_with<'a>(cnd: &'a typst_cnd::Cnd, needle: &str) -> &'a typst_cnd::Footnote {
    cnd.footnotes
        .iter()
        .find(|note| footnote_text(note).contains(needle))
        .unwrap_or_else(|| panic!("no footnote holds {needle:?}"))
}

#[test]
fn block_quote_children_are_blocks() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    let quotes = block_quotes(&cnd.nodes);
    assert_eq!(quotes.len(), 3, "only the block quotes are nodes");
    let quote = quotes[0];
    assert_eq!(node_types(&quote.children), ["paragraph", "paragraph", "list"]);
    assert_eq!(value(&quote.attribution), Some("Ada"));
    for node in all_nodes(&quote.children) {
        for text in node.texts() {
            assert!(!text.value.contains("Ada"), "the attribution leaked: {text:?}");
        }
    }
    assert_eq!(
        paragraph_values(&quote.children),
        [
            "QA1 first quote paragraph.",
            "QA2 second quote paragraph cites [1] and carries a note.1",
        ]
    );
    let CndNode::List(list) = &quote.children[2] else { unreachable!() };
    assert_eq!(
        list.items.iter().map(item_text).collect::<Vec<_>>(),
        ["QA3 quote list item", "QA4 quote list item"]
    );
    assert_cnd_contract(&cnd);
}

#[test]
fn edge_inside_a_block_quote_sits_on_its_paragraph() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    let quote = block_quotes(&cnd.nodes)[0];
    let CndNode::Paragraph(second) = &quote.children[1] else {
        panic!("the second child is a paragraph");
    };
    let text = &second.text;
    assert_eq!(
        text.cites.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
        ["knuth"]
    );
    let cite_span = text.cites[0].text_span.as_ref().expect("the cite has a span");
    assert_eq!(codepoint_slice(&text.value, cite_span), "[1]");
    assert_eq!(text.footnotes.len(), 1);
    let note_span = text.footnotes[0].text_span.as_ref().expect("the note has a span");
    assert_eq!(codepoint_slice(&text.value, note_span), "1");
    assert_eq!(footnote_with(&cnd, "QN1").label, text.footnotes[0].label);

    let attribution = quote.attribution.as_ref().expect("an attribution");
    assert!(
        attribution.refs.is_empty()
            && attribution.cites.is_empty()
            && attribution.footnotes.is_empty()
            && attribution.links.is_empty(),
        "the attribution carries no edge: {attribution:?}"
    );
    // The cite sits on that paragraph only among the quote's texts.
    let carriers = all_texts(std::slice::from_ref(&cnd.nodes[0]))
        .into_iter()
        .filter(|text| text.cites.iter().any(|c| c.label == "knuth"))
        .count();
    assert_eq!(carriers, 1);
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn decorative_footnote_entry_is_not_emitted() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    for node in all_nodes_full(&cnd) {
        for text in node.texts() {
            assert!(!text.value.contains("DN1"), "decorative note emitted: {text:?}");
        }
    }
    let json = cnd_to_json(&cnd).expect("serialize");
    assert!(!json.contains("DN1"), "decorative note reached the CND");
    // The kept note keeps the number the page shows, and its edge.
    let kept = footnote_with(&cnd, "NK1");
    assert_eq!(kept.label, "4");
    let carrier = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value.starts_with("Decorative") => Some(p),
            _ => None,
        })
        .expect("the paragraph beside the decorative note");
    assert_eq!(
        carrier
            .text
            .footnotes
            .iter()
            .map(|f| f.label.as_str())
            .collect::<Vec<_>>(),
        ["4"]
    );
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn inline_quote_is_not_a_node() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    for quote in block_quotes(&cnd.nodes) {
        for node in all_nodes(&quote.children) {
            for text in node.texts() {
                assert!(!text.value.contains("short"), "an inline quote is a node");
            }
        }
    }
    let paragraph = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value.contains("short") => Some(p),
            _ => None,
        })
        .expect("the paragraph holding the inline quote");
    assert_eq!(
        paragraph.text.value,
        "The paragraph holds an inline \u{201c}short\u{201d} quotation."
    );
    assert_eq!(occurrences(&cnd, "short"), 1);
}

#[test]
fn footnote_children_are_nodes() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    let note = footnote_with(&cnd, "NA1");
    assert_eq!(node_types(&note.children), ["paragraph", "paragraph", "list"]);
    let values = paragraph_values(&note.children);
    assert!(!values[0].starts_with(&note.label), "the number leaked: {values:?}");
    assert_eq!(values[0], "NA1 first note paragraph.");
    let CndNode::List(list) = &note.children[2] else { unreachable!() };
    assert_eq!(
        list.items.iter().map(item_text).collect::<Vec<_>>(),
        ["NA3 note item", "NA4 note item"]
    );
    // Every note starts with its own text, never with its number.
    for note in &cnd.footnotes {
        let first = first_paragraph(&note.children);
        assert!(
            !first.starts_with(char::is_numeric),
            "note {} starts with its number: {first:?}",
            note.label
        );
    }
    // No word of a note is in the body.
    for token in ["NA1", "NA2", "NA3", "NA4", "QN1", "NK1", "NP1"] {
        assert_eq!(occurrences(&cnd, token), 1, "{token} is kept once");
        assert!(
            all_nodes(&cnd.nodes)
                .into_iter()
                .flat_map(|node| node.texts())
                .all(|text| !text.value.contains(token)),
            "{token} leaked into the body"
        );
    }
    assert_cnd_contract(&cnd);
}

#[test]
fn footnote_content_carries_print_page() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    let note = footnote_with(&cnd, "NP1");
    assert!(!note.children.is_empty());
    for node in all_nodes(&note.children) {
        assert_eq!(node.base().location.page, 2, "{node:?}");
    }
    for node in all_nodes(&footnote_with(&cnd, "NA1").children) {
        assert_eq!(node.base().location.page, 1, "{node:?}");
    }
}

#[test]
fn ref_inside_footnote_resolves() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    let note = footnote_with(&cnd, "NA2");
    let paragraph = note
        .children
        .iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value.starts_with("NA2") => Some(p),
            _ => None,
        })
        .expect("the second note paragraph");
    let refs: Vec<&str> = paragraph.text.refs.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(refs, ["eq-1"]);
    let span = paragraph.text.refs[0].text_span.as_ref().expect("a span");
    assert_eq!(codepoint_slice(&paragraph.text.value, span), "Equation\u{a0}1");
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn edges_in_an_attribution_sit_on_the_attribution() {
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    let quotes = block_quotes(&cnd.nodes);

    // A citation written in the attribution is an edge of the attribution;
    // the attribution's value is not the paragraph it is laid out in, so
    // the edge has no span. The body keeps its own reference.
    let grace = quotes[1];
    let attribution = grace.attribution.as_ref().expect("an attribution");
    assert!(attribution.value.starts_with("Grace, after"), "{attribution:?}");
    assert_eq!(
        attribution
            .cites
            .iter()
            .map(|c| (c.label.as_str(), c.text_span.is_none()))
            .collect::<Vec<_>>(),
        [("knuth", true)]
    );
    assert!(attribution.refs.is_empty());
    assert_eq!(node_types(&grace.children), ["paragraph"]);
    let CndNode::Paragraph(body) = &grace.children[0] else { unreachable!() };
    assert_eq!(body.text.value, "QB1 body referring to Equation\u{a0}1.");
    assert!(body.text.cites.is_empty(), "the attribution's cite is on the body");
    assert_eq!(
        body.text.refs.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        ["eq-1"]
    );
    assert!(body.text.refs[0].text_span.is_some());

    // A label attribution is a citation of that label.
    let cited = quotes[2];
    let attribution = cited.attribution.as_ref().expect("an attribution");
    assert_eq!(
        attribution.cites.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
        ["knuth"]
    );
    assert!(attribution.cites[0].text_span.is_none());
    assert_eq!(paragraph_values(&cited.children), ["QC1 body."]);
    for node in all_nodes(&cited.children) {
        for text in node.texts() {
            assert!(text.cites.is_empty(), "the attribution's cite is on the body");
        }
    }
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn a_boxed_block_quote_is_text_of_its_paragraph() {
    // A block quote in an inline `box` is laid out in the line of the
    // paragraph that holds it, which renders its text: it is no node.
    let cnd = cnd_for_example("containers_quotes_notes.typ");
    assert_eq!(occurrences(&cnd, "QD1 boxed block quote"), 1);
    let paragraph = paragraph_texts_in_order(&cnd.nodes)
        .into_iter()
        .find(|p| p.contains("QD1"))
        .expect("the sentence holding the box");
    assert_eq!(paragraph, "A sentence holds QD1 boxed block quote inline.");
    assert_eq!(block_quotes(&cnd.nodes).len(), 3);
}

#[test]
fn a_broken_footnote_keeps_its_rest() {
    // The rest of a note too long for its page is printed on the next page
    // with no entry around it, while a list of the flow is open across the
    // same break. It is the note's content, never the body's, and it
    // carries the page it is printed on.
    let cnd = cnd_for_example("footnote_across_pages.typ");
    assert_eq!(cnd.footnotes.len(), 1);
    let note = &cnd.footnotes[0];
    assert_eq!(
        node_types(&note.children),
        ["paragraph", "paragraph", "paragraph", "paragraph", "list", "paragraph"]
    );
    let CndNode::List(list) = &note.children[4] else { unreachable!() };
    assert_eq!(
        list.items.iter().map(item_text).collect::<Vec<_>>(),
        ["SN5 note item one", "SN6 note item two"]
    );
    let pages: Vec<i32> =
        note.children.iter().map(|node| node.base().location.page).collect();
    assert_eq!(pages.first(), Some(&1));
    assert_eq!(pages.last(), Some(&2), "the rest is printed on page 2: {pages:?}");
    for token in ["SN1", "SN2", "SN3", "SN4", "SN5", "SN6", "SN7"] {
        assert_eq!(occurrences(&cnd, token), 1, "{token} once");
    }
    for text in all_texts(&cnd.nodes) {
        assert!(!text.value.contains("SN"), "the note leaked into the body: {text:?}");
    }
    // The flow list keeps its nine items, each its own paragraph.
    let flow = list_starting_with(&find_lists(&cnd.nodes), "LA1");
    assert_eq!(flow.items.len(), 9);
    for item in &flow.items {
        assert_eq!(node_types(&item.children), ["paragraph"]);
    }
    assert_eq!(
        paragraph_texts_in_order(&cnd.nodes),
        [
            "BA1 body line one.",
            "BA2 body line two carries a long note.1",
            "BA3 after the list."
        ]
    );
}

#[test]
fn a_list_broken_with_its_footnote_stays_the_notes() {
    // The note's list itself breaks: its second item is printed on the next
    // page while a flow list is open across the same break. It stays the
    // second item of the note's list, and the flow list keeps its own nine
    // items, each with its own text.
    let cnd = cnd_for_example("footnote_list_across_pages.typ");
    assert_eq!(cnd.footnotes.len(), 1);
    let note = &cnd.footnotes[0];
    assert_eq!(
        node_types(&note.children),
        ["paragraph", "paragraph", "paragraph", "paragraph", "list", "paragraph"]
    );
    let CndNode::List(list) = &note.children[4] else { unreachable!() };
    assert_eq!(
        list.items.iter().map(item_text).collect::<Vec<_>>(),
        ["SN5 note item one", "SN6 note item two"]
    );
    let pages: Vec<i32> = list
        .items
        .iter()
        .map(|item| item.children[0].base().location.page)
        .collect();
    assert_eq!(pages, [1, 2], "the second item is printed on the next page");
    let flow = list_starting_with(&find_lists(&cnd.nodes), "LA1");
    let expected: Vec<String> =
        ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine"]
            .iter()
            .enumerate()
            .map(|(i, word)| format!("LA{} flow item {word}", i + 1))
            .collect();
    assert_eq!(flow.items.iter().map(item_text).collect::<Vec<_>>(), expected);
    for token in ["SN5", "SN6", "SN7", "LA9"] {
        assert_eq!(occurrences(&cnd, token), 1, "{token} once");
    }
}

#[test]
fn a_footnote_of_reused_content_takes_nothing_from_the_flow() {
    let cnd = cnd_for_example("footnote_reused_content.typ");
    // The flow keeps everything written between a `#let` and the note that
    // uses it, and its own copy of content a note shows too.
    let section = &find_heading(&cnd.nodes, "Section").children;
    assert_eq!(
        node_types(section),
        [
            "paragraph",
            "list",
            "paragraph",
            "paragraph",
            "paragraph",
            "paragraph",
            "list",
            "paragraph",
        ]
    );
    assert_eq!(
        paragraph_values(section),
        [
            "RA1 first flow paragraph.",
            "RA2 second flow paragraph.1",
            "RA3 introduces a note.2",
            "RS2 shared sentence used twice.",
            "RA4 introduces a list note.3",
            "RA5 after.",
        ]
    );
    let notes: Vec<Vec<&str>> =
        cnd.footnotes.iter().map(|note| node_types(&note.children)).collect();
    assert_eq!(notes, [vec!["paragraph"], vec!["paragraph"], vec!["list"]]);
    assert_eq!(
        paragraph_values(&cnd.footnotes[0].children),
        ["RS1 source: annual report. See page 4."]
    );
    assert_eq!(
        paragraph_values(&cnd.footnotes[1].children),
        ["RS2 shared sentence used twice."]
    );
    // Shown twice by the author: once in the flow, once in a note.
    for token in ["RS2 shared sentence", "RL1 shared list item", "RL2 shared list item"] {
        assert_eq!(occurrences(&cnd, token), 2, "{token}");
    }
    for token in ["RS1", "RA1", "RP1", "RP2", "RA2"] {
        assert_eq!(occurrences(&cnd, token), 1, "{token}");
    }
}

#[test]
fn a_quote_body_of_reused_content_keeps_its_attribution_apart() {
    let cnd = cnd_for_example("quote_attribution_reuse.typ");
    let quotes = block_quotes(&cnd.nodes);
    let summary: Vec<(Option<&str>, Vec<&str>)> = quotes
        .iter()
        .map(|quote| (value(&quote.attribution), paragraph_values(&quote.children)))
        .collect();
    assert_eq!(
        summary,
        [
            (Some("HA1 Ada"), vec!["HI1 intro shared. HB1 body."]),
            (Some("HA2 Bob"), vec!["HB2 first.", "HB3 second paragraph."]),
            (Some("HA3 Cy"), vec!["HB4 kept."]),
        ]
    );
    for token in ["HA1", "HA2", "HA3", "HI1"] {
        assert_eq!(occurrences(&cnd, token), 1, "{token} once");
    }
    assert_eq!(occurrences(&cnd, "DQ1"), 0, "decorative text is never emitted");

    // Footnote numbers in any style are left out of the notes' text. A
    // custom entry rule that drops the number leaves nothing to exclude.
    let notes: Vec<(&str, Vec<&str>)> = cnd
        .footnotes
        .iter()
        .map(|note| (note.label.as_str(), paragraph_values(&note.children)))
        .collect();
    assert_eq!(
        notes,
        [
            ("i", vec!["FR1 roman note."]),
            ("ii", vec!["FR2 second."]),
            ("x", vec!["FT1 tenth note."]),
            ("xi", vec!["FC1 custom entry."]),
        ]
    );
}

/// The text of the first paragraph whose value contains `needle`.
fn paragraph_text<'a>(cnd: &'a typst_cnd::Cnd, needle: &str) -> &'a RichText {
    match find_paragraph(&cnd.nodes, needle) {
        Some(CndNode::Paragraph(p)) => &p.text,
        _ => panic!("no paragraph containing {needle:?}"),
    }
}

/// The text each mark of `kind` in `text` covers.
fn mark_slices(text: &RichText, kind: &str) -> Vec<String> {
    text.marks
        .iter()
        .filter(|m| m.kind == kind)
        .map(|m| codepoint_slice(&text.value, &[m.start, m.end]))
        .collect()
}

#[test]
fn strong_emph_overlap_is_kept() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Start");
    assert_eq!(text.value, "Start bold both italic end.");
    assert_eq!(mark_slices(text, typst_cnd::STRONG), ["bold both"]);
    assert_eq!(mark_slices(text, typst_cnd::EMPH), ["both italic"]);
    let strong = text.marks.iter().find(|m| m.kind == typst_cnd::STRONG).unwrap();
    let emph = text.marks.iter().find(|m| m.kind == typst_cnd::EMPH).unwrap();
    assert!(
        strong.start < emph.start && emph.start < strong.end && strong.end < emph.end
    );
}

#[test]
fn inline_code_is_a_code_mark() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "now");
    assert_eq!(text.value.matches("x = 1").count(), 1, "{:?}", text.value);
    assert_eq!(mark_slices(text, typst_cnd::CODE), ["x = 1"]);
}

#[test]
fn inline_math_mark_carries_source() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Speed");
    assert_eq!(text.value, "Speed is v = d/t here.");
    let math: Vec<_> = text.marks.iter().filter(|m| m.kind == typst_cnd::MATH).collect();
    assert_eq!(math.len(), 1, "{:?}", text.marks);
    assert_eq!(codepoint_slice(&text.value, &[math[0].start, math[0].end]), "v = d/t");
    let raw = math[0].raw.as_ref().expect("the math source");
    assert_eq!(raw.format, "typst");
    assert_eq!(raw.value, "v = d/t");
}

#[test]
fn a_paragraph_of_only_inline_math_is_kept() {
    let cnd = cnd_for_example("marks.typ");
    let text = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value == "z" => Some(&p.text),
            _ => None,
        })
        .expect("the paragraph holding only `$z$`");
    assert_eq!(text.marks.len(), 1);
    assert_eq!(text.marks[0].kind, typst_cnd::MATH);
    assert_eq!((text.marks[0].start, text.marks[0].end), (0, 1));
}

/// The same once an artifact was left out of the paragraph before the
/// equation: the source still counts as the paragraph's own text.
#[test]
fn inline_math_after_an_artifact_is_kept() {
    let cnd = cnd_for_example("marks.typ");
    let text = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value == "w" => Some(&p.text),
            _ => None,
        })
        .expect("the paragraph holding an artifact and `$w$`");
    assert_eq!(mark_slices(text, typst_cnd::MATH), ["w"]);
}

#[test]
fn strike_highlight_sub_underline_are_marks() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Styles:");
    assert_eq!(mark_slices(text, typst_cnd::STRIKE), ["gone"]);
    assert_eq!(mark_slices(text, typst_cnd::HIGHLIGHT), ["lit"]);
    assert_eq!(mark_slices(text, typst_cnd::SUB), ["2"]);
    assert_eq!(mark_slices(text, typst_cnd::UNDERLINE), ["under"]);
}

/// The math marks of `text`: (covered text, raw source).
fn math_marks(text: &RichText) -> Vec<(String, Option<String>)> {
    text.marks
        .iter()
        .filter(|m| m.kind == typst_cnd::MATH)
        .map(|m| {
            (
                codepoint_slice(&text.value, &[m.start, m.end]),
                m.raw.as_ref().map(|raw| raw.value.clone()),
            )
        })
        .collect()
}

/// An equation in a parameterized function's body has the function's
/// source, not this call's: the text is read from this instance instead,
/// with no `raw`. A parameterless one is its own source.
#[test]
fn math_from_a_function_is_this_instance() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Calls:");
    assert!(!text.value.contains("#a"), "{:?}", text.value);
    assert_eq!(text.value, "Calls: y^2 and x + y here.");
    assert_eq!(
        math_marks(text),
        [("y^2".to_string(), None), ("x + y".to_string(), Some("x + y".to_string()))]
    );
}

#[test]
fn math_with_inserted_content_keeps_its_source() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Inserted");
    assert_eq!(
        math_marks(text),
        [("#[x] + 1".to_string(), Some("#[x] + 1".to_string()))]
    );
}

#[test]
fn author_superscript_is_a_sup_mark() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "The area");
    assert_eq!(mark_slices(text, typst_cnd::SUP), ["2"]);
}

#[test]
fn footnote_marker_has_no_sup_mark() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "A claim");
    assert_eq!(text.footnotes.len(), 1);
    let span = text.footnotes[0].text_span.as_ref().expect("a span");
    assert_eq!(codepoint_slice(&text.value, span), "1");
    assert!(
        !text.marks.iter().any(|m| m.start < span[1] && m.end > span[0]),
        "no mark may cover the footnote marker: {:?}",
        text.marks
    );
}

#[test]
fn marks_count_code_points_past_astral_chars() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "\u{1F600}");
    assert_eq!(mark_slices(text, typst_cnd::STRONG), ["bold"]);
    assert_eq!(text.refs.len(), 1, "{:?}", text.refs);
    let span = text.refs[0].text_span.as_ref().expect("a span");
    assert_eq!(codepoint_slice(&text.value, span), "Figure\u{a0}1");
}

#[test]
fn adjacent_strong_runs_merge() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Join");
    assert_eq!(mark_slices(text, typst_cnd::STRONG), ["ab"]);
}

#[test]
fn link_over_whole_code_keeps_span() {
    let cnd = cnd_for_example("marks.typ");
    let text = paragraph_text(&cnd, "Read the");
    let code = text.marks.iter().find(|m| m.kind == typst_cnd::CODE).expect("code");
    assert_eq!(codepoint_slice(&text.value, &[code.start, code.end]), "api");
    assert_eq!(text.links.len(), 1);
    assert_eq!(text.links[0].text_span, Some(vec![code.start, code.end]));
}

// Label texts (CND 0.5 §4.1): headings, captions, terms and bibliography
// entries are rich text walked from what the author wrote.

#[test]
fn heading_text_has_marks() {
    let cnd = cnd_for_example("marks.typ");
    let heading = find_heading(&cnd.nodes, "The core model");
    assert_eq!(mark_slices(&heading.text, typst_cnd::STRONG), ["core"]);
    assert_eq!(heading.text.marks.len(), 1, "{:?}", heading.text.marks);
}

#[test]
fn heading_inline_math_keeps_its_source() {
    let cnd = cnd_for_example("marks.typ");
    let heading = find_heading(&cnd.nodes, "Power x^2 rule");
    let math: Vec<_> = heading
        .text
        .marks
        .iter()
        .filter(|m| m.kind == typst_cnd::MATH)
        .collect();
    assert_eq!(math.len(), 1, "{:?}", heading.text.marks);
    assert_eq!(
        codepoint_slice(&heading.text.value, &[math[0].start, math[0].end]),
        "x^2"
    );
    assert_eq!(math[0].raw.as_ref().map(|raw| raw.value.as_str()), Some("x^2"));
    assert_eq!(heading.heading_path.last().map(String::as_str), Some("Power x^2 rule"));
}

#[test]
fn caption_has_marks_and_no_supplement() {
    let cnd = cnd_for_example("marks.typ");
    let figure = find_by_label(&cnd.nodes, "fig-b").expect("figure fig-b");
    let CndNode::Figure(figure) = figure else { panic!("fig-b is a figure") };
    let caption = figure.caption.as_ref().expect("a caption");
    assert_eq!(caption.value, "A small atom");
    assert_eq!(mark_slices(caption, typst_cnd::EMPH), ["small"]);
}

#[test]
fn term_has_no_presentation_strong() {
    let cnd = cnd_for_example("marks.typ");
    let terms = find_terms(&cnd.nodes);
    let plain = &terms[0].items[0];
    assert_eq!(plain.term.value, "Plain");
    assert!(plain.term.marks.is_empty(), "{:?}", plain.term.marks);
    // The author's own strong in a term is a mark.
    let marked = &terms[0].items[1];
    assert_eq!(marked.term.value, "The key idea");
    assert_eq!(mark_slices(&marked.term, typst_cnd::STRONG), ["key"]);
}

#[test]
fn edges_written_in_a_term_sit_on_the_term() {
    let cnd = cnd_for_example("marks.typ");
    let terms = find_terms(&cnd.nodes);
    let marked = &terms[0].items[2];
    assert_eq!(marked.term.value, "Marked");
    let refs: Vec<_> = marked
        .term
        .refs
        .iter()
        .map(|r| (r.label.as_str(), r.text_span.clone()))
        .collect();
    assert_eq!(refs, [("fig-b", None)]);
    // A footnote written in a term has no edge yet: its rendered marker is
    // not in the term's authored content.
    assert!(marked.term.footnotes.is_empty());

    // Nothing written in the term lands on the definition.
    let definition = paragraph_at(&marked.children, 0);
    assert_eq!(definition.text.value, "The marked definition.");
    assert!(definition.text.refs.is_empty(), "{:?}", definition.text.refs);
    assert!(definition.text.footnotes.is_empty(), "{:?}", definition.text.footnotes);
    assert!(definition.text.cites.is_empty(), "{:?}", definition.text.cites);

    // A citation in a term sits on the term, span-less.
    let cnd = cnd_for_example("bib_rich.typ");
    let terms = find_terms(&cnd.nodes);
    let item = &terms[0].items[0];
    assert_eq!(item.term.value, "Prior art");
    let cites: Vec<_> = item
        .term
        .cites
        .iter()
        .map(|c| (c.label.as_str(), c.text_span.clone()))
        .collect();
    assert_eq!(cites, [("jones2022", None)]);
    assert!(item.term.refs.is_empty(), "a citation key is no cross-reference");
    let definition = paragraph_at(&item.children, 0);
    assert!(definition.text.cites.is_empty(), "{:?}", definition.text.cites);
    assert_pool_refs_resolve(&cnd);
}

#[test]
fn bib_formatted_is_rich_text() {
    let cnd = cnd_for_example("bib_rich.typ");
    let smith = cnd
        .bibliography
        .iter()
        .find(|entry| entry.label == "smith2024")
        .expect("smith2024");
    let formatted = smith.formatted.as_ref().expect("formatted");
    assert!(formatted.value.contains("Journal of Document Systems"), "{formatted:?}");
    assert_eq!(
        mark_slices(formatted, typst_cnd::EMPH),
        ["Journal of Document Systems"],
        "{formatted:?}"
    );
}

#[test]
fn lang_only_where_it_differs() {
    let cnd = cnd_for_example("marks.typ");
    assert_eq!(cnd.doc.lang.as_deref(), Some("en"));
    let french = paragraph_text(&cnd, "français");
    assert_eq!(french.lang.as_deref(), Some("fr"));
    let with_lang: Vec<_> = all_nodes_full(&cnd)
        .into_iter()
        .flat_map(|node| node.texts())
        .filter(|text| text.lang.is_some())
        .map(|text| text.value.as_str())
        .collect();
    assert_eq!(with_lang, ["Une phrase en français."]);
}

#[test]
fn default_lang_goes_on_doc_not_texts() {
    let cnd = cnd_for_example("bib_rich.typ");
    assert_eq!(cnd.doc.lang.as_deref(), Some("en"));
    let mut texts: Vec<&RichText> = all_nodes_full(&cnd)
        .into_iter()
        .flat_map(|node| node.texts())
        .collect();
    texts.extend(cnd.bibliography.iter().filter_map(|entry| entry.formatted.as_ref()));
    assert!(texts.len() > 3);
    for text in texts {
        assert_eq!(text.lang, None, "{:?}", text.value);
    }
}

#[test]
fn label_texts_take_their_own_lang() {
    let cnd = cnd_for_example("lang.typ");
    assert_eq!(cnd.doc.lang.as_deref(), Some("en"));
    let mut langs: Vec<(&str, Option<&str>)> = all_nodes_full(&cnd)
        .into_iter()
        .flat_map(|node| node.texts())
        .map(|text| (text.value.as_str(), text.lang.as_deref()))
        .collect();
    langs.sort();
    assert_eq!(
        langs,
        [
            ("Languages", None),
            ("Mostly English words with un mot inside.", None),
            ("Terme", Some("fr")),
            ("Un auteur", Some("fr")),
            ("Une citation.", Some("fr")),
            ("Une définition.", Some("fr")),
            ("Une légende", Some("fr")),
            ("abcd abcd", Some("fr")),
        ]
    );
}

#[test]
fn a_dropped_citation_leaves_one_space_in_a_label() {
    let cnd = cnd_for_example("bib_rich.typ");
    let caption = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Figure(figure) => figure.caption.as_ref(),
            _ => None,
        })
        .expect("a caption");
    assert_eq!(caption.value, "Shown in and later.");
    let cites: Vec<_> = caption.cites.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(cites, ["knuth"]);
}

/// The figure with `label`, wherever it sits.
fn figure_by_label<'a>(
    cnd: &'a typst_cnd::Cnd,
    label: &str,
) -> &'a typst_cnd::FigureNode {
    all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Figure(figure) if figure.base.label.as_deref() == Some(label) => {
                Some(figure)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no figure labelled {label}"))
}

#[test]
fn figure_alt_is_description() {
    let cnd = cnd_for_example("figure_description.typ");
    let shares = figure_by_label(&cnd, "fig-shares");
    let description = shares.description.as_ref().expect("a description");
    assert_eq!(description.value, "Bars: A 38 %, B 27 %");
    assert!(description.marks.is_empty() && description.refs.is_empty());
    assert_eq!(value(&shares.caption), Some("Shares"));
    // The image's own alt stays the image's: it is not the figure's.
    let plain = figure_by_label(&cnd, "fig-plain");
    assert!(plain.description.is_none(), "{:?}", plain.description);
    let CndNode::Image(image) = &plain.children[0] else { panic!("an image") };
    assert_eq!(image.alt.as_deref(), Some("Cover art"));
    // A blank `alt` describes nothing.
    let blank = figure_by_label(&cnd, "fig-blank");
    assert!(blank.description.is_none(), "{:?}", blank.description);
}

/// Every table of `table_kinds.typ`, in reading order, figures entered.
fn tables_of(cnd: &typst_cnd::Cnd) -> Vec<&typst_cnd::TableNode> {
    all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Table(table) => Some(table),
            _ => None,
        })
        .collect()
}

#[test]
fn table_kind_comes_from_the_hint() {
    let cnd = cnd_for_example("table_kinds.typ");
    let kinds: Vec<_> = tables_of(&cnd)
        .iter()
        .map(|table| (first_paragraph(&table.cells[0].children), table.kind.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("Sensor", Some("data")),
            ("Plan", Some("content")),
            ("North", None),
            ("1", Some("matrix")),
            // A non-string hint says nothing.
            ("Numbered hint", None),
            ("Year", Some("data")),
        ]
    );
    // The kind is the table's: the figure around one has its own.
    let figure = figure_by_label(&cnd, "tab-counts");
    assert_eq!(figure.kind.as_deref(), Some("table"));
}

#[test]
fn table_kind_hint_never_leaks() {
    for path in all_example_files() {
        let name = path.file_name().and_then(|n| n.to_str()).expect("name");
        let cnd = cnd_for_example(name);
        for node in all_nodes_full(&cnd) {
            assert!(
                !node.base().state_metadata.contains_key("table-kind"),
                "{name}: a {} keeps the hint: {:?}",
                node_type(node),
                node.base().state_metadata
            );
        }
    }
    // The author's own metadata stays, on the tables and in their cells.
    let cnd = cnd_for_example("table_kinds.typ");
    for table in tables_of(&cnd) {
        let section = table.base.state_metadata.get("section");
        assert_eq!(section.and_then(|v| v.as_str()), Some("tables"));
        for cell in &table.cells {
            for node in &cell.children {
                let section = node.base().state_metadata.get("section");
                assert_eq!(section.and_then(|v| v.as_str()), Some("tables"));
            }
        }
    }
}

#[test]
fn ref_never_lands_on_an_unlabelled_target() {
    // Over every example: an edge names a label a node carries, and no
    // heading holds an edge, since no example but `math_refs.typ` writes a
    // reference in a heading's own text (that one is pinned in
    // `refs_written_in_a_heading_or_caption_sit_there`).
    for path in all_example_files() {
        let name = path.file_name().and_then(|n| n.to_str()).expect("name");
        let refs_in_headings = name == "math_refs.typ";
        let cnd = cnd_for_example(name);
        let nodes = all_nodes_full(&cnd);
        let carries = |label: &str| {
            nodes.iter().any(|node| node.base().label.as_deref() == Some(label))
        };
        for node in &nodes {
            for text in node.texts() {
                for reference in &text.refs {
                    assert!(
                        carries(&reference.label),
                        "{name}: @{} names no node",
                        reference.label
                    );
                }
            }
            if let CndNode::Heading(heading) = node
                && !refs_in_headings
            {
                assert!(
                    heading.text.refs.is_empty(),
                    "{name}: heading {:?} holds {:?}",
                    heading.text.value,
                    heading.text.refs
                );
            }
        }
    }

    // The last paragraph of a section holds no copy of the references
    // written elsewhere in it (here, in a term above it).
    let cnd = cnd_for_example("marks.typ");
    let last = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Paragraph(p) if p.text.value == "Une phrase en français." => {
                Some(p)
            }
            _ => None,
        })
        .expect("the French paragraph");
    assert!(last.text.refs.is_empty(), "{:?}", last.text.refs);

    // The figure placed in a drawing is no node: a reference to it has no
    // edge, and its label goes to no other node.
    let cnd = cnd_for_example("ref_exact_targets.typ");
    let labels: Vec<_> = all_nodes_full(&cnd)
        .into_iter()
        .filter_map(|node| node.base().label.as_deref())
        .collect();
    assert_eq!(labels, ["fig-plain", "fig-drawing"]);
    let refs = |text: &RichText| -> Vec<(String, String)> {
        text.refs
            .iter()
            .map(|r| {
                let span = r.text_span.as_ref().expect("a ref in a paragraph has a span");
                (r.label.clone(), codepoint_slice(&text.value, span))
            })
            .collect()
    };
    let targets = find_heading(&cnd.nodes, "Targets");
    assert!(targets.text.refs.is_empty(), "{:?}", targets.text.refs);
    let paragraph = paragraph_at(&targets.children, 2);
    assert_eq!(
        refs(&paragraph.text),
        [
            ("fig-plain".to_string(), "Table\u{a0}1".to_string()),
            ("fig-drawing".to_string(), "Figure\u{a0}1".to_string()),
        ]
    );
    let elsewhere = find_heading(&cnd.nodes, "Elsewhere");
    assert!(elsewhere.text.refs.is_empty(), "{:?}", elsewhere.text.refs);
    let paragraph = paragraph_at(&elsewhere.children, 0);
    assert_eq!(
        refs(&paragraph.text),
        [("fig-plain".to_string(), "Table\u{a0}1".to_string())]
    );
}

/// Every `(label, span)` of a text's `refs`.
fn ref_pairs(text: &RichText) -> Vec<(&str, Option<Vec<i64>>)> {
    text.refs
        .iter()
        .map(|r| (r.label.as_str(), r.text_span.clone()))
        .collect()
}

/// Every `(label, span)` of a text's `cites`.
fn cite_pairs(text: &RichText) -> Vec<(&str, Option<Vec<i64>>)> {
    text.cites
        .iter()
        .map(|c| (c.label.as_str(), c.text_span.clone()))
        .collect()
}

#[test]
fn edges_in_inline_math_sit_on_their_text() {
    let cnd = cnd_for_example("math_refs.typ");
    let target = find_heading(&cnd.nodes, "Target");
    let paragraph = paragraph_at(&target.children, 0);
    assert!(paragraph.text.value.starts_with("Inline math"), "{:?}", paragraph.text);
    // Span-less: a span may not reach into the atomic math mark.
    assert_eq!(ref_pairs(&paragraph.text), [("sec-target", None)]);
    assert_eq!(cite_pairs(&paragraph.text), [("knuth", None)]);

    // In a caption, the same.
    let figure = figure_by_label(&cnd, "fig-math");
    let caption = figure.caption.as_ref().expect("a caption");
    assert_eq!(ref_pairs(caption), [("sec-target", None)]);

    // In a term, on the term only.
    let terms = find_terms(&cnd.nodes);
    let item = &terms[0].items[0];
    assert_eq!(ref_pairs(&item.term), [("sec-target", None)]);
    let definition = paragraph_at(&item.children, 0);
    assert!(definition.text.refs.is_empty(), "{:?}", definition.text.refs);

    // A block equation is a math node: no edge, and no copy of its reference
    // anywhere else: every edge to `sec-target` is one of the three above,
    // or the heading that names it.
    let holders: Vec<_> = all_nodes_full(&cnd)
        .into_iter()
        .flat_map(|node| {
            node.texts()
                .into_iter()
                .filter(|text| text.refs.iter().any(|r| r.label == "sec-target"))
                .map(move |text| (node_type(node), text.value.clone()))
        })
        .collect();
    assert_eq!(
        holders,
        [
            ("paragraph", paragraph.text.value.clone()),
            ("figure", caption.value.clone()),
            ("terms", item.term.value.clone()),
            ("heading", "Section naming".to_string()),
        ]
    );
    let after = paragraph_at(&target.children, 2);
    assert_eq!(after.text.value, "After the block equation.");
    assert!(after.text.refs.is_empty(), "{:?}", after.text.refs);
}

#[test]
fn refs_written_in_a_heading_or_caption_sit_there() {
    let cnd = cnd_for_example("math_refs.typ");
    let heading = find_heading(&cnd.nodes, "Section naming");
    assert_eq!(ref_pairs(&heading.text), [("sec-target", None)]);
    let figure = figure_by_label(&cnd, "fig-naming");
    let caption = figure.caption.as_ref().expect("a caption");
    assert_eq!(ref_pairs(caption), [("fig-math", None)]);
    // Nothing else refers to `fig-math`.
    let count = all_nodes_full(&cnd)
        .into_iter()
        .flat_map(|node| node.texts())
        .flat_map(|text| &text.refs)
        .filter(|r| r.label == "fig-math")
        .count();
    assert_eq!(count, 1);
}

#[test]
fn doc_status_version_extra() {
    let cnd = cnd_for_example("doc_metadata.typ");
    assert_eq!(cnd.doc.status.as_deref(), Some("Approved"));
    assert_eq!(cnd.doc.version.as_deref(), Some("1.2"));
    let extra: Vec<_> = cnd.doc.extra.iter().collect();
    assert_eq!(extra, [(&"client".to_string(), &"ACME".to_string())]);
}

#[test]
fn doc_markers_are_not_nodes() {
    let cnd = cnd_for_example("doc_metadata.typ");
    for node in all_nodes_full(&cnd) {
        let json = serde_json::to_string(node).expect("node json");
        assert!(!json.contains("Approved"), "a node holds the marker: {json}");
        assert!(!json.contains("ACME"), "a node holds the marker: {json}");
    }
}

#[test]
fn code_title_from_helper() {
    let cnd = cnd_for_example("doc_metadata.typ");
    let codes: Vec<_> = all_nodes(&cnd.nodes)
        .into_iter()
        .filter_map(|node| match node {
            CndNode::Code(code) => Some(code),
            _ => None,
        })
        .collect();
    let titles: Vec<_> = codes.iter().map(|c| c.title.as_deref()).collect();
    assert_eq!(titles, [Some("main.py"), None]);
    for node in all_nodes_full(&cnd) {
        assert!(
            !node.base().state_metadata.contains_key("code-title"),
            "a {} keeps the hint",
            node_type(node)
        );
    }
}

#[test]
fn doc_markers_override_and_drop() {
    let cnd = cnd_for_example("doc_metadata_markers.typ");
    assert_eq!(cnd.doc.status.as_deref(), Some("Final"));
    assert_eq!(cnd.doc.version.as_deref(), Some("0.1"));
    let extra: Vec<_> =
        cnd.doc.extra.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_eq!(extra, [("client", "Globex"), ("team", "Docs")]);
    assert_eq!(cnd.doc.title, "Marker rules");
}
