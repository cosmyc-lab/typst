//! Which table cells are header cells in the emitted CND. The author-facing
//! `pdf.header-cell` / `pdf.data-cell` need the `a11y-extras` feature, which
//! `CndWorld` does not enable by default, so these tests turn it on.

mod common;

use std::fs;

use common::{all_nodes, first_paragraph};
use typst::Feature;
use typst_cnd::{
    CndDocument, CndNode, cnd_from_document,
    world::{CndWorld, built_at_now, source_info},
};

/// Compile `source` and return `(text, is_header)` of the cells of its first
/// table, in order. `a11y` enables the `a11y-extras` feature.
fn table_cells(source: &str, a11y: bool) -> Vec<(String, bool)> {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("main.typ");
    fs::write(&input, source).expect("write source");
    let mut world = CndWorld::new(&input).expect("world");
    if a11y {
        world = world.with_features([Feature::A11yExtras]);
    }
    let doc = typst::compile::<CndDocument>(&world)
        .output
        .unwrap_or_else(|errors| panic!("compile failed: {errors:?}"));
    let cnd = cnd_from_document(&doc, source_info(&world), built_at_now());
    let table = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Table(table) => Some(table),
            _ => None,
        })
        .expect("a table");
    table
        .cells
        .iter()
        .map(|cell| (first_paragraph(&cell.children).to_string(), cell.is_header))
        .collect()
}

fn headers(cells: &[(String, bool)]) -> Vec<&str> {
    cells.iter().filter(|(_, h)| *h).map(|(t, _)| t.as_str()).collect()
}

const NUTRITION: &str = r#"#set document(title: "Table test")
#set text(lang: "en")
= Tables

#table(
  columns: 3,
  table.header(
    pdf.header-cell(scope: "both")[Nutrient], [Per 100g], [Per serving],
  ),
  pdf.header-cell(scope: "row")[Calories], [250 kcal], [375 kcal],
  table.header(level: 2, table.cell(colspan: 3)[Macros]),
  pdf.header-cell(scope: "row")[Protein], table.cell(rowspan: 2)[8 g], [12 g],
  pdf.header-cell(scope: "row")[Fat], [18 g],
  table.footer([Total], [—], [—]),
)
"#;

#[test]
fn every_header_row_and_header_cell_is_marked() {
    let cells = table_cells(NUTRITION, true);
    assert_eq!(
        headers(&cells),
        ["Nutrient", "Per 100g", "Per serving", "Calories", "Macros", "Protein", "Fat"]
    );
}

#[test]
fn a_footer_stays_data() {
    let cells = table_cells(NUTRITION, true);
    for text in ["Total", "—"] {
        assert!(cells.iter().filter(|(t, _)| t == text).all(|(_, h)| !h), "{text}");
    }
}

#[test]
fn a_mid_table_header_without_cell_kinds_is_marked() {
    let cells = table_cells(
        "#table(columns: 2,\n  table.header[A][B],\n  [1], [2],\n  table.header[C][D],\n  [3], [4],\n)",
        false,
    );
    assert_eq!(headers(&cells), ["A", "B", "C", "D"]);
}

#[test]
fn a_data_cell_in_a_header_row_is_not_a_header() {
    let cells = table_cells(
        "#table(columns: 2,\n  table.header[A][B],\n  pdf.data-cell[x], [y],\n  table.header[C][D],\n)",
        true,
    );
    assert_eq!(headers(&cells), ["A", "B", "C", "D"]);
    let table =
        "#table(columns: 2,\n  table.header(pdf.data-cell[x], [B]),\n  [1], [2],\n)";
    let cells = table_cells(table, true);
    assert_eq!(headers(&cells), ["B"]);
}

#[test]
fn a_plain_table_is_unchanged() {
    let cells = table_cells(
        "#table(columns: 2, table.header[H1][H2], [a], [b], [c], [d])",
        false,
    );
    assert_eq!(headers(&cells), ["H1", "H2"]);
    let cells = table_cells("#table(columns: 2, [a], [b], [c], [d])", false);
    assert!(headers(&cells).is_empty());
}

#[test]
fn a_table_with_gutter_marks_its_headers() {
    let cells = table_cells(
        "#table(columns: 2, row-gutter: 2pt,\n  table.header[A][B], [1], [2], table.header[C][D], [3], [4])",
        false,
    );
    assert_eq!(headers(&cells), ["A", "B", "C", "D"]);
}

#[test]
fn a_grid_declaring_a_header_keeps_plain_cells() {
    // A grid arranges content; it has no header cells, even with `grid.header`.
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("main.typ");
    fs::write(&input, "#grid(columns: 2, grid.header[A][B], [1], [2])")
        .expect("write source");
    let world = CndWorld::new(&input).expect("world");
    let doc = typst::compile::<CndDocument>(&world).output.expect("compile");
    let cnd = cnd_from_document(&doc, source_info(&world), built_at_now());
    let layout = all_nodes(&cnd.nodes)
        .into_iter()
        .find_map(|node| match node {
            CndNode::Layout(layout) => Some(layout),
            _ => None,
        })
        .expect("a layout");
    assert_eq!(layout.cells.len(), 4);
    assert!(layout.cells.iter().all(|cell| !cell.is_header));
}
