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

use crate::emit::convert::{self, Records, Synthetic};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::model::{
    ListItem as CndListItem, ListNode, RichText, TermItem as CndTermItem, TermsNode,
};

/// Nested items, grouped into runs of the same kind: each run becomes one
/// nested `list` node, `true` for an enum.
type Runs = Vec<(bool, Vec<CndListItem>)>;

pub fn from_list(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    list: &Packed<ListElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ListNode, Records)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = list.clone().pack();
    let (record, edges) = convert::make_record(engine, introspector, &packed, &[])?;
    let mut synthetic = Synthetic::new(record.location);

    let mut node = ListNode::new(id, location);
    node.ordered = false;
    node.items = list
        .children
        .iter()
        .map(|item| list_item(introspector, item, styles, &mut synthetic))
        .collect();
    // The list's span-less edges go to its first paragraph (interim rule).
    let _ = synthetic.attach_to_first_slot(
        node.items.iter().map(|item| item.children.as_slice()),
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

pub fn from_enum(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    enum_: &Packed<EnumElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ListNode, Records)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = enum_.clone().pack();
    let (record, edges) = convert::make_record(engine, introspector, &packed, &[])?;
    let mut synthetic = Synthetic::new(record.location);

    let mut node = ListNode::new(id, location);
    node.ordered = true;
    node.items = enum_items(introspector, enum_, styles, &mut synthetic);
    // The list's span-less edges go to its first paragraph (interim rule).
    let _ = synthetic.attach_to_first_slot(
        node.items.iter().map(|item| item.children.as_slice()),
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

/// Convert a Typst definition list (`/ term: description`) into a
/// `TermsNode`. Each description is one paragraph child of its item.
pub fn from_terms(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    terms: &Packed<TermsElem>,
) -> typst_library::diag::SourceResult<(TermsNode, Records)> {
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = terms.clone().pack();
    let (record, edges) = convert::make_record(engine, introspector, &packed, &[])?;
    let mut synthetic = Synthetic::new(record.location);

    let mut node = TermsNode::new(id, location);
    node.items = terms
        .children
        .iter()
        .map(|item| term_item(item, &mut synthetic))
        .collect();
    let _ = synthetic.attach_to_first_slot(
        node.items.iter().map(|item| item.children.as_slice()),
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

fn term_item(item: &Packed<TermItem>, synthetic: &mut Synthetic) -> CndTermItem {
    let description =
        synthetic.paragraph(RichText::plain(extract_text(&item.description)));
    CndTermItem {
        term: RichText::plain(extract_text(&item.term)),
        children: vec![description],
    }
}

/// One list item: a paragraph with its own text, then one nested list per
/// run of same-kind nested items.
///
/// Items nested in markup (`- parent` followed by an indented `- child`) are
/// not wrapped in a list inside the parent's body: they sit there as bare
/// `ListItem`/`EnumItem`s, and are grouped into a list only when the parent
/// is laid out. An explicit `#list(..)`/`#enum(..)` call does sit there as a
/// list. Either way they become nested lists, and the paragraph keeps only
/// what is not nested.
fn list_item(
    introspector: &dyn Introspector,
    item: &Packed<ListItem>,
    styles: StyleChain,
    synthetic: &mut Synthetic,
) -> CndListItem {
    item_from_body(introspector, &item.body, None, styles, synthetic)
}

fn enum_item(
    introspector: &dyn Introspector,
    item: &Packed<EnumItem>,
    number: u64,
    styles: StyleChain,
    synthetic: &mut Synthetic,
) -> CndListItem {
    item_from_body(introspector, &item.body, Some(number as i32), styles, synthetic)
}

fn item_from_body(
    introspector: &dyn Introspector,
    body: &Content,
    number: Option<i32>,
    styles: StyleChain,
    synthetic: &mut Synthetic,
) -> CndListItem {
    let mut own = Vec::new();
    let mut runs = Runs::new();
    split_body(introspector, body, styles, &mut own, &mut runs, synthetic);
    let text = extract_text(&Content::sequence(own)).trim().to_string();
    let mut children = vec![synthetic.paragraph(RichText::plain(text))];
    for (ordered, items) in runs {
        children.push(synthetic.list(ordered, items));
    }
    CndListItem { number, checked: None, children }
}

/// Append `item` to the run of its kind, opening a new run on a change.
fn push_run(runs: &mut Runs, ordered: bool, item: CndListItem) {
    match runs.last_mut() {
        Some((kind, items)) if *kind == ordered => items.push(item),
        _ => runs.push((ordered, vec![item])),
    }
}

/// Sort an item body into its own content and its nested items.
fn split_body(
    introspector: &dyn Introspector,
    body: &Content,
    styles: StyleChain,
    own: &mut Vec<Content>,
    runs: &mut Runs,
    synthetic: &mut Synthetic,
) {
    let parts: Vec<&Content> = match body.to_packed::<SequenceElem>() {
        Some(sequence) => sequence.children.iter().collect(),
        None => vec![body],
    };
    // Consecutive bare enum items form one enum, numbered as it is displayed.
    let mut group: Vec<&Packed<EnumItem>> = Vec::new();
    let flush = |group: &mut Vec<&Packed<EnumItem>>,
                 runs: &mut Runs,
                 synthetic: &mut Synthetic| {
        if let Some(first) = group.first() {
            let (start, reversed) = realized_enum(introspector, first.span())
                .map(|realized| enum_settings(&realized, StyleChain::default()))
                .unwrap_or_else(|| {
                    (styles.get(EnumElem::start), styles.get(EnumElem::reversed))
                });
            let numbers = enum_numbers(group.iter().copied(), start, reversed, styles);
            for (item, n) in group.drain(..).zip(numbers) {
                let item = enum_item(introspector, item, n, styles, synthetic);
                push_run(runs, true, item);
            }
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
        flush(&mut group, runs, synthetic);
        if let Some(item) = part.to_packed::<ListItem>() {
            let item = list_item(introspector, item, styles, synthetic);
            push_run(runs, false, item);
        } else if let Some(list) = part.to_packed::<ListElem>() {
            for item in &list.children {
                let item = list_item(introspector, item, styles, synthetic);
                push_run(runs, false, item);
            }
        } else if let Some(enum_) = part.to_packed::<EnumElem>() {
            for item in enum_items(introspector, enum_, styles, synthetic) {
                push_run(runs, true, item);
            }
        } else if let Some(styled) = part.to_packed::<StyledElem>() {
            // A set rule inside the body (`#set enum(start: 5)`) styles the
            // rest of it, nested items included.
            split_body(
                introspector,
                &styled.child,
                styles.chain(&styled.styles),
                own,
                runs,
                synthetic,
            );
        } else {
            own.push(part.clone());
        }
    }
    flush(&mut group, runs, synthetic);
}

/// An enum's items with the numbers it displays.
fn enum_items(
    introspector: &dyn Introspector,
    enum_: &Packed<EnumElem>,
    styles: StyleChain,
    synthetic: &mut Synthetic,
) -> Vec<CndListItem> {
    let (start, reversed) = enum_settings(enum_, styles);
    let numbers = enum_numbers(enum_.children.iter(), start, reversed, styles);
    enum_
        .children
        .iter()
        .zip(numbers)
        .map(|(item, n)| enum_item(introspector, item, n, styles, synthetic))
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
