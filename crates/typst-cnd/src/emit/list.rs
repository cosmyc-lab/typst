use typst_library::engine::Engine;
use typst_library::foundations::{NativeElement, Packed, Smart, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{EnumElem, EnumItem, ListElem, ListItem};

use crate::emit::convert::{self, NodeRecord};
use crate::location::placeholder_location;
use crate::manifest::{ListItem as CndListItem, ListNode};

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
        text: item.body.plain_text().into(),
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
        text: item.body.plain_text().into(),
        number,
        children: nested,
    }
}
