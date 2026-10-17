use ecow::EcoString;
use typst_library::World;
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, Label, NativeElement, Packed, StyleChain, Synthesize,
};
use typst_library::introspection::{Introspector, Location};
use typst_library::layout::GridElem;
use typst_library::layout::grid::resolve::{CellGrid, Entry};
use typst_library::model::{FigureCaption, FigureElem, TableElem};
use uuid::Uuid;

use crate::emit::convert::{self, NodeRecord, Records, Synthetic, TextRecord};
use crate::emit::extract::extract_text;
use crate::emit::figure::figure_kind;
use crate::location::placeholder_location;
use crate::model::{
    Cell, CndNode, FigureNode, LayoutNode, RichText, TableNode, TextSlot,
};

/// Find a table element nested inside realized figure (or other) content.
pub fn table_in_content(content: &Content) -> Option<Packed<TableElem>> {
    let table = table_content_in(content)?;
    table.to_packed().cloned()
}

/// Like [`table_in_content`], but keeps the introspector [`Content`] (with location).
pub fn table_content_in(content: &Content) -> Option<Content> {
    content.query_first_naive(&TableElem::ELEM.select())
}

/// Find a grid element nested inside figure content.
pub fn grid_in_content(content: &Content) -> Option<Packed<GridElem>> {
    let grid = content.query_first_naive(&GridElem::ELEM.select())?;
    grid.to_packed().cloned()
}

/// Public alias used by figure emission.
pub fn raw_typst_for_label(engine: &Engine, label: Option<Label>) -> Option<String> {
    raw_typst_from_label(engine, label)
}

/// Returns whether a laid-out table belongs to a figure wrapper.
pub fn is_table_in_figure(introspector: &dyn Introspector, table: &Content) -> bool {
    let Some(table_packed) = table.to_packed::<TableElem>() else {
        return false;
    };
    let fingerprint = table_fingerprint(table_packed);
    introspector
        .query(&FigureElem::ELEM.select())
        .into_iter()
        .any(|figure| {
            table_in_content(&figure)
                .map(|nested| table_fingerprint(&nested) == fingerprint)
                .unwrap_or(false)
        })
}

fn table_fingerprint(table: &Packed<TableElem>) -> EcoString {
    extract_text(&table.clone().pack())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("|")
        .into()
}

/// The figure's span-less edges go to the first paragraph of its body
/// (interim rule), else to its caption; with neither they are dropped.
pub(crate) fn attach_figure_edges(
    figure: &FigureNode,
    record: &mut NodeRecord,
    edges: TextRecord,
    synthetic: &mut Synthetic,
) {
    if let Some(mut edges) = synthetic.attach_to_first_paragraph(&figure.children, edges)
        && figure.caption.is_some()
    {
        edges.slot = TextSlot::Caption;
        record.add_text(edges);
    }
}

/// Convert a table wrapped in a figure into a `FigureNode` wrapper holding
/// a `TableNode` child. The caption/number live on the wrapper (ADR 0010);
/// `TableNode` itself carries no caption.
pub fn from_figure(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    figure: &Packed<FigureElem>,
    table: &Packed<TableElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(FigureNode, Records)> {
    let mut figure = figure.clone();
    figure.synthesize(engine, styles)?;

    let caption = figure.caption.get_cloned(styles).as_ref().map(caption_text);
    let fig_number = convert::figure_number(engine, &figure, styles);
    let (mut wrapper_record, wrapper_edges) =
        convert::make_record(engine, introspector, content, &[])?;

    let (table_node, mut table_records) =
        figure_table(engine, introspector, table, styles)?;
    table_records.locate_in(content.location());

    let wrapper_id = Uuid::new_v4();
    let mut wrapper = FigureNode::new(wrapper_id, placeholder_location());
    wrapper.caption = caption.map(RichText::plain);
    wrapper.numbering =
        convert::numbering(fig_number, convert::figure_counter_label(&figure, styles));
    wrapper.kind = figure_kind(&figure, styles);
    wrapper.children.push(CndNode::Table(table_node));

    let mut synthetic = Synthetic::new(content.location());
    synthetic.records = table_records.synthetic;
    attach_figure_edges(&wrapper, &mut wrapper_record, wrapper_edges, &mut synthetic);

    let mut own = vec![(wrapper_id, wrapper_record)];
    own.extend(table_records.own);
    Ok((wrapper, Records { own, synthetic: synthetic.records }))
}

/// Convert a grid wrapped in a figure into a `FigureNode` wrapper holding a
/// `LayoutNode` child.
pub fn from_figure_grid(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    figure: &Packed<FigureElem>,
    grid: &Packed<GridElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(FigureNode, Records)> {
    let mut figure = figure.clone();
    figure.synthesize(engine, styles)?;

    let caption = figure
        .caption
        .get_cloned(styles)
        .as_ref()
        .map(caption_text)
        .map(RichText::plain);
    let fig_number = convert::figure_number(engine, &figure, styles);
    let (mut wrapper_record, wrapper_edges) =
        convert::make_record(engine, introspector, content, &[])?;

    let mut grid = grid.clone();
    if grid.grid.is_none() {
        grid.synthesize(engine, styles)?;
    }

    let mut synthetic = Synthetic::new(content.location());
    let layout_id = Uuid::new_v4();
    synthetic
        .records
        .push((layout_id, NodeRecord::synthetic(content.location())));
    let mut layout = LayoutNode::new(layout_id, placeholder_location());
    if let Some(grid) = grid.grid.as_ref() {
        layout.cells = text_cells(grid, 0, &mut synthetic);
    }

    let wrapper_id = Uuid::new_v4();
    let mut wrapper = FigureNode::new(wrapper_id, placeholder_location());
    wrapper.caption = caption;
    wrapper.numbering =
        convert::numbering(fig_number, convert::figure_counter_label(&figure, styles));
    wrapper.kind = figure_kind(&figure, styles);
    wrapper.children.push(CndNode::Layout(layout));
    attach_figure_edges(&wrapper, &mut wrapper_record, wrapper_edges, &mut synthetic);

    Ok((
        wrapper,
        Records {
            own: vec![(wrapper_id, wrapper_record)],
            synthetic: synthetic.records,
        },
    ))
}

fn caption_text(caption: &Packed<FigureCaption>) -> EcoString {
    convert::caption_text(caption)
}

/// Convert a table element into a CND table node. `TableNode` carries no
/// caption of its own — a captioned table is a node wrapped in a
/// `FigureNode`. Its cells' `children` are filled by `convert::assemble`
/// with the blocks laid out in each cell. A table has no text, so it keeps
/// no edge: a marker written in a cell is an edge of the cell's paragraph.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    table: &Packed<TableElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(TableNode, NodeRecord)> {
    let mut table = table.clone();
    if table.grid.is_none() {
        table.synthesize(engine, styles)?;
    }
    let (record, _) =
        convert::make_record(engine, introspector, &table.clone().pack(), &[])?;
    let mut node = TableNode::new(Uuid::new_v4(), placeholder_location());
    if let Some(grid) = table.grid.as_ref() {
        node.cells = cells_from_grid(grid, header_rows(grid));
    }
    Ok((node, record))
}

/// Convert a bare grid — one used for arrangement, outside any figure —
/// into a `LayoutNode`. Its cells are filled as a table's are. The
/// introspector does not know the grid, so its record takes `anchor`, the
/// first element laid out in it, for its page and state.
pub fn from_grid(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    grid: &Packed<GridElem>,
    anchor: Location,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(LayoutNode, NodeRecord)> {
    let mut grid = grid.clone();
    if grid.grid.is_none() {
        grid.synthesize(engine, styles)?;
    }
    let record = NodeRecord {
        location: Some(anchor),
        label: grid.label(),
        state_metadata: convert::metadata_at(engine, introspector, anchor)?,
        texts: Vec::new(),
    };
    let mut node = LayoutNode::new(Uuid::new_v4(), placeholder_location());
    if let Some(cells) = grid.grid.as_ref() {
        node.cells = cells_from_grid(cells, 0);
    }
    Ok((node, record))
}

/// The cells of a resolved grid, in `entries` order: row-major, which is
/// reading order. A cell spanning several positions comes once, at its
/// first. The first `header_rows` rows are header cells. Their `children`
/// are left empty: `convert::assemble` fills them through each cell's slot
/// (`Ancestry::cell_slot`), found by the same `(col, row)` position.
pub fn cells_from_grid(grid: &CellGrid, header_rows: usize) -> Vec<Cell> {
    let cols = grid.non_gutter_column_count();
    if cols == 0 {
        return Vec::new();
    }
    grid.entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            let cell = entry.as_cell()?;
            let row = idx / cols;
            Some(Cell {
                row: row as i32,
                col: (idx % cols) as i32,
                rowspan: cell.rowspan.get() as i32,
                colspan: cell.colspan.get() as i32,
                is_header: row < header_rows,
                children: Vec::new(),
            })
        })
        .collect()
}

/// How many rows, from the first, the grid's headers cover. A header's
/// range counts gutter rows, which sit between every two rows.
fn header_rows(grid: &CellGrid) -> usize {
    let is_header = |row: usize| {
        let track = if grid.has_gutter { 2 * row } else { row };
        grid.headers.iter().any(|header| header.range.contains(&track))
    };
    (0..).take_while(|row| is_header(*row)).count()
}

/// A table inside a figure, whose cells each hold one paragraph made from
/// their text. Interim, until figure bodies are routed: the figure's
/// span-less edges go to its first cell paragraph.
fn figure_table(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    table: &Packed<TableElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(TableNode, Records)> {
    let mut table = table.clone();
    if table.grid.is_none() {
        table.synthesize(engine, styles)?;
    }

    let id = Uuid::new_v4();
    let packed = table.clone().pack();
    let (record, edges) = convert::make_record(engine, introspector, &packed, &[])?;
    let mut synthetic = Synthetic::new(record.location);

    let mut node = TableNode::new(id, placeholder_location());
    if let Some(grid) = table.grid.as_ref() {
        node.cells = text_cells(grid, header_rows(grid), &mut synthetic);
    }
    let _ = synthetic.attach_to_first_slot(
        node.cells.iter().map(|cell| cell.children.as_slice()),
        edges,
    );

    Ok((
        node,
        Records {
            own: vec![(id, record)],
            synthetic: synthetic.records,
        },
    ))
}

/// The cells of a grid inside a figure: a cell with text gets one paragraph
/// child holding it; an empty cell has no children. Interim, until figure
/// bodies are routed.
fn text_cells(
    grid: &CellGrid,
    header_rows: usize,
    synthetic: &mut Synthetic,
) -> Vec<Cell> {
    let mut cells = cells_from_grid(grid, header_rows);
    let bodies = grid.entries.iter().filter_map(Entry::as_cell);
    for (cell, body) in cells.iter_mut().zip(bodies) {
        let text = extract_text(&body.body);
        if !text.is_empty() {
            cell.children.push(synthetic.paragraph(RichText::plain(text)));
        }
    }
    cells
}

fn raw_typst_from_label(engine: &Engine, label: Option<Label>) -> Option<String> {
    let label = label?;
    let main = engine.world.main();
    let source = engine.world.source(main).ok()?;
    let text = source.text();
    let marker = format!("<{}>", label.resolve().as_str());
    let label_end = text.find(marker.as_str())? + marker.len();
    let before = &text[..label_end];
    let figure_start = before.rfind("#figure(")?;
    let snippet = &text[figure_start..label_end];
    Some(extract_table_snippet(snippet))
}

fn extract_table_snippet(text: &str) -> String {
    if let Some(start) = text.find("table(")
        && let Some(end) = matching_paren_end(&text[start..])
    {
        return text[start..start + end].trim().to_string();
    }
    text.trim().to_string()
}

fn matching_paren_end(text: &str) -> Option<usize> {
    let mut depth = 0_usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx + 1);
                }
            }
            _ => {}
        }
    }
    None
}
