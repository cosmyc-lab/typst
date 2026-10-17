use ecow::EcoString;
use typst_library::World;
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, Label, NativeElement, Packed, StyleChain, Synthesize,
};
use typst_library::introspection::Introspector;
use typst_library::layout::GridElem;
use typst_library::layout::grid::resolve::{CellGrid, Entry};
use typst_library::model::{
    FigureCaption, FigureElem, TableChild, TableElem, TableHeader, TableItem,
};
use typst_syntax::Span;
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

    let caption = figure
        .caption
        .get_cloned(styles)
        .as_ref()
        .map(caption_text)
        .or_else(|| caption_from_source(engine, content.span()))
        .or_else(|| caption_from_source(engine, figure.span()))
        .or_else(|| caption_from_source(engine, Span::detached()));
    let fig_number = convert::figure_number(engine, &figure, styles);
    let (mut wrapper_record, wrapper_edges) =
        convert::make_record(engine, introspector, content, &[])?;

    let (table_node, mut table_records) = convert(engine, introspector, table, styles)?;
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
    layout.cells = cells_from_cell_grid(
        grid.grid.as_ref().map(|grid| grid.as_ref()),
        &mut synthetic,
    );

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
/// caption of its own — a captioned table is this node wrapped in a
/// `FigureNode` by the caller (`from_figure`). Its span-less edges go to its
/// first cell paragraph (interim rule).
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    table: &Packed<TableElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(TableNode, Records)> {
    let mut table = table.clone();
    if table.grid.is_none() {
        table.synthesize(engine, styles)?;
    }

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = table.clone().pack();
    let (record, edges) = convert::make_record(engine, introspector, &packed, &[])?;
    let mut synthetic = Synthetic::new(record.location);

    let header_rows = count_header_rows(&table);
    let mut node = TableNode::new(id, location);
    node.cells = cells_from_grid(&table, header_rows, &mut synthetic);
    for cell in &node.cells {
        if synthetic
            .attach_to_first_paragraph(&cell.children, edges.clone())
            .is_none()
        {
            break;
        }
    }

    Ok((
        node,
        Records {
            own: vec![(id, record)],
            synthetic: synthetic.records,
        },
    ))
}

fn count_header_rows(table: &Packed<TableElem>) -> i32 {
    let mut rows = 0_usize;
    for child in &table.children {
        if let TableChild::Header(header) = child {
            rows += count_header_items(header);
        }
    }
    if rows == 0 && table.children.iter().any(|c| matches!(c, TableChild::Header(_))) {
        rows = 1;
    }
    rows as i32
}

fn count_header_items(header: &Packed<TableHeader>) -> usize {
    let mut max_row = 0_usize;
    for item in &header.children {
        if let TableItem::Cell(cell) = item {
            max_row = max_row.max(1);
            let _ = cell;
        }
    }
    max_row.max(1)
}

fn cells_from_grid(
    table: &Packed<TableElem>,
    header_rows: i32,
    synthetic: &mut Synthetic,
) -> Vec<Cell> {
    cells_from_cell_grid(table.grid.as_ref().map(|grid| grid.as_ref()), synthetic)
        .into_iter()
        .map(|mut cell| {
            cell.is_header = cell.row < header_rows;
            cell
        })
        .collect()
}

/// The cells of a resolved grid. A cell with text gets one paragraph child
/// holding it; an empty cell has no children.
pub fn cells_from_cell_grid(
    grid: Option<&CellGrid>,
    synthetic: &mut Synthetic,
) -> Vec<Cell> {
    let Some(grid) = grid else {
        return Vec::new();
    };

    let cols = grid.non_gutter_column_count();
    if cols == 0 {
        return Vec::new();
    }

    let mut cells = Vec::new();
    for (idx, entry) in grid.entries.iter().enumerate() {
        let Entry::Cell(cell) = entry else { continue };
        let row = (idx / cols) as i32;
        let col = (idx % cols) as i32;
        let text = extract_text(&cell.body);
        let children = if text.is_empty() {
            Vec::new()
        } else {
            vec![synthetic.paragraph(RichText::plain(text))]
        };
        cells.push(Cell {
            row,
            col,
            rowspan: cell.rowspan.get() as i32,
            colspan: cell.colspan.get() as i32,
            is_header: false,
            children,
        });
    }
    cells
}

fn raw_typst_from_span(engine: &Engine, span: Span) -> Option<String> {
    let id = span.id()?;
    let range = engine.world.range(span)?;
    let source = engine.world.source(id).ok()?;
    let text = source.text();
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start >= end {
        return None;
    }
    Some(extract_table_snippet(&text[start..end]))
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

fn caption_from_source(engine: &Engine, span: Span) -> Option<EcoString> {
    if let Some(snippet) = raw_typst_from_span(engine, span)
        && let Some(caption) = parse_caption(&snippet)
    {
        return Some(caption);
    }
    let main = engine.world.main();
    let source = engine.world.source(main).ok()?;
    parse_caption(source.text())
}

fn parse_caption(text: &str) -> Option<EcoString> {
    let marker = "caption:";
    let start = text.find(marker)? + marker.len();
    let rest = text[start..].trim_start();
    if !rest.starts_with('[') {
        return None;
    }
    let mut depth = 0_usize;
    let mut end = None;
    for (idx, ch) in rest.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end = Some(idx + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    Some(rest[..end].trim_matches(|c| c == '[' || c == ']').into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_caption_from_figure_block() {
        let text = "#figure(
  table(columns: 2),
  caption: [Paramètres nominaux de fonctionnement.],
) <tab-params>";
        let caption = parse_caption(text).unwrap();
        assert_eq!(caption.as_str(), "Paramètres nominaux de fonctionnement.");
    }
}
