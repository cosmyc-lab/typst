use typst_library::engine::Engine;
use typst_library::foundations::{NativeElement, Packed, Smart, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{EnumElem, EnumItem, ListElem, ListItem, TermItem, TermsElem};

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::manifest::{ListItem as CndListItem, ListNode, TermItem as CndTermItem, TermsNode};

pub fn from_list(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    list: &Packed<ListElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ListNode, NodeRecord)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = list.clone().pack();
    let record = convert::make_record(engine, introspector, &packed)?;

    let mut node = ListNode::new(id, location);
    node.ordered = false;
    node.tight = list.tight.get(styles);
    node.items = list
        .children
        .iter()
        .map(|item| list_item(item, false, styles))
        .collect();

    Ok((node, record))
}

pub fn from_enum(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    enum_: &Packed<EnumElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ListNode, NodeRecord)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = enum_.clone().pack();
    let record = convert::make_record(engine, introspector, &packed)?;

    let mut node = ListNode::new(id, location);
    node.ordered = true;
    node.tight = enum_.tight.get(styles);
    node.items = enum_
        .children
        .iter()
        .enumerate()
        .map(|(index, item)| enum_item(item, index, styles))
        .collect();

    Ok((node, record))
}

/// Convert a Typst definition list (`/ term: description`) into a
/// `TermsNode`. Mirror of `from_list`: term/description are flat text by
/// schema, so there is no nesting recursion (proposal 0004).
pub fn from_terms(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    terms: &Packed<TermsElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(TermsNode, NodeRecord)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = terms.clone().pack();
    let record = convert::make_record(engine, introspector, &packed)?;

    let mut node = TermsNode::new(id, location);
    node.tight = terms.tight.get(styles);
    node.items = terms.children.iter().map(term_item).collect();

    Ok((node, record))
}

fn term_item(item: &Packed<TermItem>) -> CndTermItem {
    CndTermItem {
        term: extract_text(&item.term).into(),
        description: extract_text(&item.description).into(),
    }
}

fn list_item(item: &Packed<ListItem>, ordered: bool, styles: StyleChain) -> CndListItem {
    let nested = if let Some(list) = item.body.query_first_naive(&ListElem::ELEM.select()) {
        if let Some(list) = list.to_packed::<ListElem>() {
            list.children
                .iter()
                .map(|child| list_item(child, ordered, styles))
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    CndListItem {
        text: extract_text(&item.body).into(),
        number: None,
        children: nested,
    }
}

fn enum_item(item: &Packed<EnumItem>, index: usize, styles: StyleChain) -> CndListItem {
    let number = match item.number.get(styles) {
        Smart::Custom(n) => Some(n as i32),
        _ => Some((index + 1) as i32),
    };

    let nested = if let Some(list) = item.body.query_first_naive(&EnumElem::ELEM.select()) {
        if let Some(list) = list.to_packed::<EnumElem>() {
            list.children
                .iter()
                .enumerate()
                .map(|(i, child)| enum_item(child, i, styles))
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    CndListItem {
        text: extract_text(&item.body).into(),
        number,
        children: nested,
    }
}
