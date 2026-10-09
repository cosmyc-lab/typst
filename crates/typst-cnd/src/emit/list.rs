use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, NativeElement, Packed, SequenceElem, Smart, StyleChain, StyledElem,
};
use typst_library::introspection::Introspector;
use typst_library::model::{
    EnumElem, EnumItem, ListElem, ListItem, ParbreakElem, TermItem, TermsElem,
};
use typst_library::text::SpaceElem;
use typst_syntax::Span;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::model::{
    ListItem as CndListItem, ListNode, TermItem as CndTermItem, TermsNode,
};

pub fn from_list(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    list: &Packed<ListElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ListNode, NodeRecord)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = list.clone().pack();
    let record = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = ListNode::new(id, location);
    node.ordered = false;
    node.tight = list.tight.get(styles);
    node.items = list
        .children
        .iter()
        .map(|item| list_item(introspector, item, styles))
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
    let record = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = ListNode::new(id, location);
    node.ordered = true;
    node.tight = enum_.tight.get(styles);
    node.items = enum_items(introspector, enum_, styles);

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
    let record = convert::make_record(engine, introspector, &packed, &[])?;

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

/// One list item: its own text, and the items nested under it as children.
///
/// Items nested in markup (`- parent` followed by an indented `- child`) are
/// not wrapped in a list inside the parent's body: they sit there as bare
/// `ListItem`/`EnumItem`s, and are grouped into a list only when the parent
/// is laid out. An explicit `#list(..)`/`#enum(..)` call does sit there as a
/// list. Either way they become `children`, and the parent's `text` keeps
/// only what is not nested.
fn list_item(
    introspector: &dyn Introspector,
    item: &Packed<ListItem>,
    styles: StyleChain,
) -> CndListItem {
    let mut own = Vec::new();
    let mut children = Vec::new();
    split_body(introspector, &item.body, styles, &mut own, &mut children);
    CndListItem {
        text: extract_text(&Content::sequence(own)).trim().into(),
        number: None,
        children,
    }
}

fn enum_item(
    introspector: &dyn Introspector,
    item: &Packed<EnumItem>,
    number: u64,
    styles: StyleChain,
) -> CndListItem {
    let mut own = Vec::new();
    let mut children = Vec::new();
    split_body(introspector, &item.body, styles, &mut own, &mut children);
    CndListItem {
        text: extract_text(&Content::sequence(own)).trim().into(),
        number: Some(number as i32),
        children,
    }
}

/// Sort an item body into its own content and its nested items.
fn split_body(
    introspector: &dyn Introspector,
    body: &Content,
    styles: StyleChain,
    own: &mut Vec<Content>,
    children: &mut Vec<CndListItem>,
) {
    let parts: Vec<&Content> = match body.to_packed::<SequenceElem>() {
        Some(sequence) => sequence.children.iter().collect(),
        None => vec![body],
    };
    // Consecutive bare enum items form one enum, numbered as it is displayed.
    let mut group: Vec<&Packed<EnumItem>> = Vec::new();
    let flush = |group: &mut Vec<&Packed<EnumItem>>, children: &mut Vec<CndListItem>| {
        if let Some(first) = group.first() {
            let (start, reversed) = realized_enum(introspector, first.span())
                .map(|realized| enum_settings(&realized, StyleChain::default()))
                .unwrap_or_else(|| {
                    (styles.get(EnumElem::start), styles.get(EnumElem::reversed))
                });
            let numbers = enum_numbers(group.iter().copied(), start, reversed, styles);
            children.extend(
                group
                    .drain(..)
                    .zip(numbers)
                    .map(|(item, n)| enum_item(introspector, item, n, styles)),
            );
        }
    };
    for part in parts {
        if let Some(item) = part.to_packed::<EnumItem>() {
            group.push(item);
            continue;
        }
        // Spacing between items does not end the enum they form.
        if part.is::<SpaceElem>() || part.is::<ParbreakElem>() {
            if group.is_empty() {
                own.push(part.clone());
            }
            continue;
        }
        flush(&mut group, children);
        if let Some(item) = part.to_packed::<ListItem>() {
            children.push(list_item(introspector, item, styles));
        } else if let Some(list) = part.to_packed::<ListElem>() {
            children.extend(
                list.children.iter().map(|item| list_item(introspector, item, styles)),
            );
        } else if let Some(enum_) = part.to_packed::<EnumElem>() {
            children.extend(enum_items(introspector, enum_, styles));
        } else if let Some(styled) = part.to_packed::<StyledElem>() {
            // A set rule inside the body (`#set enum(start: 5)`) styles the
            // rest of it, nested items included.
            split_body(
                introspector,
                &styled.child,
                styles.chain(&styled.styles),
                own,
                children,
            );
        } else {
            own.push(part.clone());
        }
    }
    flush(&mut group, children);
}

/// An enum's items with the numbers it displays.
fn enum_items(
    introspector: &dyn Introspector,
    enum_: &Packed<EnumElem>,
    styles: StyleChain,
) -> Vec<CndListItem> {
    let (start, reversed) = enum_settings(enum_, styles);
    let numbers = enum_numbers(enum_.children.iter(), start, reversed, styles);
    enum_
        .children
        .iter()
        .zip(numbers)
        .map(|(item, n)| enum_item(introspector, item, n, styles))
        .collect()
}

fn enum_settings(enum_: &Packed<EnumElem>, styles: StyleChain) -> (Smart<u64>, bool) {
    (enum_.start.get(styles), enum_.reversed.get(styles))
}

/// The laid-out enum that bare markup items starting at `span` were grouped
/// into. Layout gives the grouped enum its first item's span, and its fields
/// hold the values its styles gave them — a `start` set outside the parent
/// list included, which the parent's body does not carry.
fn realized_enum(
    introspector: &dyn Introspector,
    span: Span,
) -> Option<Packed<EnumElem>> {
    if span.is_detached() {
        return None;
    }
    let elem = introspector
        .query(&EnumElem::ELEM.select())
        .into_iter()
        .find(|elem| elem.span() == span)?;
    elem.into_packed::<EnumElem>().ok()
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
