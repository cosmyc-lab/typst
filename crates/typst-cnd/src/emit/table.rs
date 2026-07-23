use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::World;
use typst_library::WorldExt;
use typst_library::foundations::{Content, Label, NativeElement, Packed, StyleChain, Synthesize};
use typst_library::introspection::Introspector;
use typst_library::layout::GridElem;
use typst_library::layout::grid::resolve::{CellGrid, Entry};
use typst_library::model::{FigureCaption, FigureElem, TableChild, TableElem, TableHeader, TableItem};
use typst_syntax::Span;
use uuid::Uuid;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::emit::figure::figure_kind;
use crate::location::placeholder_location;
use crate::model::{CndNode, FigureNode, RawSource, TableCell, TableKind, TableNode};

/// Find a table element nested inside realized figure (or other) content.
pub fn table_in_content(content: &Content) -> Option<Packed<TableElem>> {
    table_content_in(content).and_then(|table| table.to_packed().cloned())
}

/// Like [`table_in_content`], but keeps the introspector [`Content`] (with location).
pub fn table_content_in(content: &Content) -> Option<Content> {
    content.query_first_naive(&TableElem::ELEM.select())
}

/// Find a grid element nested inside figure content.
pub fn grid_in_content(content: &Content) -> Option<Packed<GridElem>> {
    content
        .query_first_naive(&GridElem::ELEM.select())
        .and_then(|grid| grid.to_packed().cloned())
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
    let fingerprint = table_fingerprint(&table_packed);
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
) -> typst_library::diag::SourceResult<(FigureNode, Vec<(Uuid, NodeRecord)>)> {
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
    let fig_number = convert::figure_number(engine, &figure, styles)
        .map(Into::into)
        .or_else(|| figure_number_fallback(introspector, content));
    let wrapper_record = convert::make_record(engine, introspector, content, &[])?;

    let (table_node, table_record) = convert(
        engine,
        introspector,
        table,
        styles,
        Some(content.span()),
        content.label(),
    )?;
    let table_id = table_node.base.id;

    let wrapper_id = Uuid::new_v4();
    let mut wrapper = FigureNode::new(wrapper_id, placeholder_location());
    wrapper.caption = caption.map(Into::into);
    wrapper.number = fig_number.map(Into::into);
    wrapper.counter_label =
        convert::figure_counter_label(&figure, styles).map(Into::into);
    wrapper.kind = figure_kind(&figure, styles);
    wrapper.children.push(CndNode::Table(table_node));

    Ok((wrapper, vec![(wrapper_id, wrapper_record), (table_id, table_record)]))
}

/// Convert a grid wrapped in a figure into a `FigureNode` wrapper holding a
/// `TableNode { kind: Grid }` child.
pub fn from_figure_grid(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    figure: &Packed<FigureElem>,
    grid: &Packed<GridElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(FigureNode, Vec<(Uuid, NodeRecord)>)> {
    let mut figure = figure.clone();
    figure.synthesize(engine, styles)?;

    let caption = figure
        .caption
        .get_cloned(styles)
        .as_ref()
        .map(caption_text)
        .map(Into::into);
    let fig_number = convert::figure_number(engine, &figure, styles).map(Into::into);
    let wrapper_record = convert::make_record(engine, introspector, content, &[])?;

    let mut grid = grid.clone();
    if grid.grid.is_none() {
        grid.synthesize(engine, styles)?;
    }

    let table_id = Uuid::new_v4();
    let cells = cells_from_cell_grid(grid.grid.as_ref().map(|grid| grid.as_ref()));

    let mut table_node = TableNode::new(table_id, placeholder_location());
    table_node.kind = TableKind::Grid;
    table_node.content_kind = content_kind_from_metadata(&wrapper_record.state_metadata);
    table_node.cells = cells;
    table_node.raw = raw_typst_for_label(engine, content.label()).map(RawSource::typst);

    let table_record = NodeRecord {
        location: content.location(),
        label: None,
        ref_targets: Vec::new(),
        footnote_locs: Vec::new(),
        cite_markers: Vec::new(),
        ref_markers: Vec::new(),
        state_metadata: std::collections::HashMap::new(),
    };

    let wrapper_id = Uuid::new_v4();
    let mut wrapper = FigureNode::new(wrapper_id, placeholder_location());
    wrapper.caption = caption;
    wrapper.number = fig_number;
    wrapper.counter_label =
        convert::figure_counter_label(&figure, styles).map(Into::into);
    wrapper.kind = figure_kind(&figure, styles);
    wrapper.children.push(CndNode::Table(table_node));

    Ok((wrapper, vec![(wrapper_id, wrapper_record), (table_id, table_record)]))
}

fn caption_text(caption: &Packed<FigureCaption>) -> EcoString {
    convert::caption_text(caption)
}

/// Convert a table element into a CND table node. `TableNode` carries no
/// caption of its own — a captioned table is this node wrapped in a
/// `FigureNode` by the caller (`from_figure`).
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    table: &Packed<TableElem>,
    styles: StyleChain,
    source_span: Option<Span>,
    label: Option<Label>,
) -> typst_library::diag::SourceResult<(TableNode, NodeRecord)> {
    let mut table = table.clone();
    if table.grid.is_none() {
        table.synthesize(engine, styles)?;
    }

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = table.clone().pack();
    let record = convert::make_record(engine, introspector, &packed, &[])?;

    let header_rows = count_header_rows(&table);
    let cells = cells_from_grid(&table, header_rows);
    let raw_typst = source_span
        .and_then(|span| raw_typst_from_span(engine, span))
        .or_else(|| raw_typst_for_label(engine, label))
        .or_else(|| raw_typst_from_span(engine, table.span()));

    let mut node = TableNode::new(id, location);
    node.content_kind = content_kind_from_metadata(&record.state_metadata);
    node.cells = cells;
    node.raw = raw_typst.map(RawSource::typst);

    Ok((node, record))
}

/// Promote a `content_kind` hint from the generic `cnd.metadata` state bag
/// (already captured on every node's `state_metadata`, see
/// `emit/convert.rs`'s `make_record`/`apply_metadata`) into the table's own
/// typed field — the one `cnd.core.node_text`'s "auto"/"inline" rendering
/// actually reads. Only a string value is accepted; anything else (wrong
/// type, or simply absent) leaves the hint unset rather than guessing.
fn content_kind_from_metadata(metadata: &std::collections::HashMap<String, serde_json::Value>) -> Option<String> {
    match metadata.get("content_kind") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn count_header_rows(table: &Packed<TableElem>) -> i32 {
    let mut rows = 0usize;
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
    let mut max_row = 0usize;
    for item in &header.children {
        if let TableItem::Cell(cell) = item {
            max_row = max_row.max(1);
            let _ = cell;
        }
    }
    max_row.max(1)
}

fn cells_from_grid(table: &Packed<TableElem>, header_rows: i32) -> Vec<TableCell> {
    cells_from_cell_grid(table.grid.as_ref().map(|grid| grid.as_ref()))
        .into_iter()
        .map(|mut cell| {
            cell.is_header = cell.row < header_rows;
            cell
        })
        .collect()
}

pub fn cells_from_cell_grid(grid: Option<&CellGrid>) -> Vec<TableCell> {
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
        cells.push(TableCell {
            row,
            col,
            rowspan: cell.rowspan.get() as i32,
            colspan: cell.colspan.get() as i32,
            is_header: false,
            text: extract_text(&cell.body).into(),
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
    if let Some(start) = text.find("table(") {
        if let Some(end) = matching_paren_end(&text[start..]) {
            return text[start..start + end].trim().to_string();
        }
    }
    text.trim().to_string()
}

fn matching_paren_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
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

fn figure_number_fallback(
    introspector: &dyn Introspector,
    content: &Content,
) -> Option<EcoString> {
    let loc = content.location()?;
    let index = introspector.query_count_before(&FigureElem::ELEM.select(), loc) + 1;
    Some(ecow::eco_format!("Tableau {index}"))
}

fn caption_from_source(engine: &Engine, span: Span) -> Option<EcoString> {
    if let Some(snippet) = raw_typst_from_span(engine, span) {
        if let Some(caption) = parse_caption(&snippet) {
            return Some(caption);
        }
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
    let mut depth = 0usize;
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
        let text = r#"#figure(
  table(columns: 2),
  caption: [Paramètres nominaux de fonctionnement.],
) <tab-params>"#;
        let caption = parse_caption(text).unwrap();
        assert_eq!(caption.as_str(), "Paramètres nominaux de fonctionnement.");
    }
}
