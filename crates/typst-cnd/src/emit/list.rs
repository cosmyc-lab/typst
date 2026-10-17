//! Lists, enums and term lists.
//!
//! Each item's `children` are the blocks laid out in its body — paragraphs,
//! nested lists, code, tables… — which `convert::assemble` routes into it
//! from the realized layout. The nodes built here only hold the items, in
//! the order the list lays them out, with what the realized body does not
//! say: an enum item's number and a term.
//!
//! A list has no text of its own, so it captures no edge: a footnote or a
//! citation written in an item is an edge of the paragraph it sits in. A
//! term is a text of its own, walked as authored (`extract::label_text`):
//! what is written in it is its edges.

use typst_library::engine::Engine;
use typst_library::foundations::{Packed, Smart, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{EnumElem, EnumItem, ListElem, TermsElem};

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::{label_text, source_text};
use crate::location::placeholder_location;
use crate::model::{
    ListItem as CndListItem, ListNode, TermItem as CndTermItem, TermsNode, TextSlot,
};

pub fn from_list(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    list: &Packed<ListElem>,
) -> typst_library::diag::SourceResult<(ListNode, NodeRecord)> {
    let (record, _) =
        convert::make_record(engine, introspector, &list.clone().pack(), &[])?;
    let mut node = ListNode::new(uuid::Uuid::new_v4(), placeholder_location());
    node.items = list.children.iter().map(|_| item(None)).collect();
    Ok((node, record))
}

pub fn from_enum(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    enum_: &Packed<EnumElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ListNode, NodeRecord)> {
    let (record, _) =
        convert::make_record(engine, introspector, &enum_.clone().pack(), &[])?;
    let mut node = ListNode::new(uuid::Uuid::new_v4(), placeholder_location());
    node.ordered = true;
    // The element is materialized: `start` and `reversed` hold the values
    // its styles gave them, a `#set enum(start: 5)` in a parent item
    // included.
    let start = enum_.start.get(styles);
    let reversed = enum_.reversed.get(styles);
    node.items = enum_numbers(enum_.children.iter(), start, reversed, styles)
        .into_iter()
        .map(|number| item(Some(number as i32)))
        .collect();
    Ok((node, record))
}

/// Convert a Typst definition list (`/ term: description`) into a
/// `TermsNode`. Each description's blocks are routed into its item.
///
/// A term is read from what the author wrote, not from its realization,
/// so the `strong` the terms rule lays it out in is no mark of it.
pub fn from_terms(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    terms: &Packed<TermsElem>,
) -> typst_library::diag::SourceResult<(TermsNode, NodeRecord)> {
    let (mut record, _) =
        convert::make_record(engine, introspector, &terms.clone().pack(), &[])?;
    let mut node = TermsNode::new(uuid::Uuid::new_v4(), placeholder_location());
    for (index, item) in terms.children.iter().enumerate() {
        let (term, mut edges) = label_text(&item.term, &|span| source_text(engine, span));
        edges.slot = TextSlot::Term(index);
        record.add_text(edges);
        node.items.push(CndTermItem { term, children: Vec::new() });
    }
    Ok((node, record))
}

fn item(number: Option<i32>) -> CndListItem {
    CndListItem { number, checked: None, children: Vec::new() }
}

/// The numbers an enum displays, following its layout: it starts at `start`
/// (1, or the item count when reversed), an item with an explicit number
/// (`5.` in markup) restarts the count there, and each item steps it by one.
fn enum_numbers<'a>(
    items: impl ExactSizeIterator<Item = &'a Packed<EnumItem>>,
    start: Smart<u64>,
    reversed: bool,
    styles: StyleChain,
) -> Vec<u64> {
    let count = items.len() as u64;
    let mut number = start.custom().unwrap_or(if reversed { count } else { 1 });
    items
        .map(|item| {
            number = item.number.get(styles).custom().unwrap_or(number);
            let current = number;
            number = if reversed {
                number.saturating_sub(1)
            } else {
                number.saturating_add(1)
            };
            current
        })
        .collect()
}
