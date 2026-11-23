use typst_library::engine::Engine;
use typst_library::foundations::{Packed, Smart, StyleChain, Synthesize};
use typst_library::introspection::{Introspector, Location};
use typst_library::layout::GridElem;
use typst_library::layout::grid::resolve::{Cell, CellGrid};
use typst_library::model::{TableCell, TableCellKind, TableElem};
use uuid::Uuid;

use crate::emit::convert::{self, NodeRecord};
use crate::location::placeholder_location;
use crate::model::{Cell as CndCell, LayoutNode, TableNode};

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
        node.cells = cells_from_grid(grid);
    }
    Ok((node, record))
}

/// Convert a grid — one used for arrangement, in the flow or in a figure —
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
        node.cells = cells_from_grid(cells);
    }
    Ok((node, record))
}

/// The cells of a resolved grid, in `entries` order: row-major, which is
/// reading order. A cell spanning several positions comes once, at its
/// first. A table cell is a header cell when its resolved kind says so: the
/// cells of every `table.header` row, wherever it sits, and any cell the
/// author marked `pdf.header-cell`; a cell marked `pdf.data-cell` is data
/// even inside a header row, and a footer is data. A grid's cells are never
/// header cells. Their `children` are left empty: `convert::assemble` fills them through each cell's slot
/// (`Ancestry::cell_slot`), found by the same `(col, row)` position.
pub fn cells_from_grid(grid: &CellGrid) -> Vec<CndCell> {
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
            Some(CndCell {
                row: row as i32,
                col: (idx % cols) as i32,
                rowspan: cell.rowspan.get() as i32,
                colspan: cell.colspan.get() as i32,
                is_header: is_header_cell(grid, cell, row),
                children: Vec::new(),
            })
        })
        .collect()
}

/// Whether a resolved cell is a header cell. Grid resolution gives every
/// table cell a kind (its row group's, unless the author set one), which is
/// what tagged PDF reads too. A grid's cell is no table cell and has none.
fn is_header_cell(grid: &CellGrid, cell: &Cell, row: usize) -> bool {
    let Some(table_cell) = cell.body.to_packed::<TableCell>() else {
        return false;
    };
    match table_cell.kind.get(StyleChain::default()) {
        Smart::Custom(kind) => matches!(kind, TableCellKind::Header(..)),
        // Unreachable after resolution; fall back to the header ranges, which
        // count gutter rows between every two rows.
        Smart::Auto => {
            let track = if grid.has_gutter { 2 * row } else { row };
            grid.headers.iter().any(|header| header.range.contains(&track))
        }
    }
}
