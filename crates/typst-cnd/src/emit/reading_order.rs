//! Reading-order sorting for laid-out document nodes.
//!
//! Multi-column layouts (block `#columns` or page-level columns) place content
//! side-by-side. CND export must flatten nodes in human reading order:
//! column-major for LTR text — top-to-bottom within a column, then the next
//! column to the right.

use std::cmp::Ordering;

use typst_library::foundations::Selector;
use typst_library::introspection::{Introspector, Location};
use typst_library::layout::Abs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ReadingKey {
    /// Global document position from the introspector (primary).
    doc: usize,
    page: u32,
    /// Column-major tie-break inside a multicol region.
    x: i64,
    y: i64,
}

/// Sort `(location, content)` pairs in reading order for paged documents.
pub fn sort_by_reading_order(
    items: &mut [(Location, typst_library::foundations::Content)],
    introspector: &dyn Introspector,
    doc_selector: &Selector,
) {
    items.sort_by(|(loc_a, _), (loc_b, _)| {
        reading_key(introspector, doc_selector, *loc_a)
            .cmp(&reading_key(introspector, doc_selector, *loc_b))
    });
}

fn reading_key(
    introspector: &dyn Introspector,
    doc_selector: &Selector,
    location: Location,
) -> ReadingKey {
    let page = introspector
        .page(location)
        .map(|page| page.get() as u32)
        .unwrap_or(1);
    let (x, y) = introspector
        .position(location)
        .map(|pos| pos.as_paged_or_default().point)
        .map(|point| (abs_key(point.x), abs_key(point.y)))
        .unwrap_or((0, 0));
    let doc = introspector.query_count_before(doc_selector, location);

    ReadingKey { doc, page, x, y }
}

fn abs_key(value: Abs) -> i64 {
    // Fixed-point key stable across Abs representations.
    (value.to_pt() * 1_000.0).round() as i64
}

/// Compare two locations in reading order (useful for tests).
pub fn compare_locations(
    introspector: &dyn Introspector,
    doc_selector: &Selector,
    left: Location,
    right: Location,
) -> Ordering {
    reading_key(introspector, doc_selector, left)
        .cmp(&reading_key(introspector, doc_selector, right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_key_orders_document_then_columns() {
        let same_doc_a = ReadingKey { doc: 5, page: 1, x: 10, y: 20 };
        let same_doc_b = ReadingKey { doc: 5, page: 1, x: 10, y: 40 };
        let later_doc = ReadingKey { doc: 6, page: 1, x: 0, y: 0 };

        assert!(same_doc_a < same_doc_b);
        assert!(same_doc_b < later_doc);
    }

    #[test]
    fn reading_key_column_major_within_same_doc() {
        let left = ReadingKey { doc: 3, page: 1, x: 10, y: 100 };
        let right = ReadingKey { doc: 3, page: 1, x: 200, y: 10 };

        assert!(left < right);
    }
}
