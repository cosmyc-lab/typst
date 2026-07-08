use std::ops::ControlFlow;

use ecow::{EcoString, eco_vec};
use rustc_hash::FxHashSet;
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{Content, Label, NativeElement, Packed, Selector, StyleChain};
use typst_library::introspection::{Introspector, Location};
use typst_library::math::EquationElem;
use typst_library::model::{
    EnumElem, EnumItem, FigureCaption, FigureElem, HeadingElem, ListElem, ListItem, ParElem,
    QuoteElem, RefElem, Supplement, TableElem,
};
use typst_library::text::RawElem;
use typst_syntax::{FileId, Span};
use uuid::Uuid;

use crate::emit::{code, extract, figure, heading, list, math, paragraph, quote, reading_order, table};
use crate::manifest::{CndNode, HeadingNode};

/// Metadata captured for each emitted node.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub location: Option<Location>,
    pub label: Option<Label>,
    pub ref_targets: Vec<Label>,
    pub state_metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Conversion context shared across the document walk.
#[derive(Debug, Default)]
pub struct ConvertContext {
    pub roots: Vec<CndNode>,
    pub records: rustc_hash::FxHashMap<Uuid, NodeRecord>,
    pub location_to_id: rustc_hash::FxHashMap<Location, Uuid>,
    pub label_to_id: rustc_hash::FxHashMap<Label, Uuid>,
}

impl ConvertContext {
    pub fn register(&mut self, id: Uuid, record: NodeRecord) {
        if let Some(label) = record.label {
            self.label_to_id.insert(label, id);
        }
        if let Some(location) = record.location {
            self.location_to_id.insert(location, id);
        }
        self.records.insert(id, record);
    }
}

/// Active heading container while walking the document.
pub struct HeadingFrame {
    pub level: i32,
    pub path: Vec<String>,
    pub node: HeadingNode,
}

fn doc_selector() -> Selector {
    Selector::Or(eco_vec![
        HeadingElem::ELEM.select(),
        ParElem::ELEM.select(),
        TableElem::ELEM.select(),
        FigureElem::ELEM.select(),
        QuoteElem::ELEM.select(),
        RawElem::ELEM.select(),
        EquationElem::ELEM.select(),
        ListElem::ELEM.select(),
        EnumElem::ELEM.select(),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceRange {
    file: FileId,
    start: usize,
    end: usize,
}

fn source_range(engine: &Engine, span: Span) -> Option<SourceRange> {
    let file = span.id()?;
    let range = engine.world.range(span)?;
    Some(SourceRange {
        file,
        start: range.start,
        end: range.end,
    })
}

fn range_contains(outer: SourceRange, inner: SourceRange) -> bool {
    outer.file == inner.file && inner.start >= outer.start && inner.end <= outer.end
}

fn is_strictly_nested(inner: SourceRange, outer: SourceRange) -> bool {
    range_contains(outer, inner) && inner != outer
}

fn build_list_enum_ranges(engine: &Engine, introspector: &dyn Introspector) -> Vec<SourceRange> {
    let mut ranges = Vec::new();
    for selector in [ListElem::ELEM.select(), EnumElem::ELEM.select()] {
        for elem in introspector.query(&selector) {
            if let Some(range) = source_range(engine, elem.span()) {
                ranges.push(range);
            }
        }
    }
    ranges
}

fn build_inline_nested_list_enum_locations(
    engine: &Engine,
    introspector: &dyn Introspector,
) -> FxHashSet<Location> {
    let mut locations = FxHashSet::default();

    for elem in introspector.query(&ListElem::ELEM.select()) {
        let Some(list) = elem.to_packed::<ListElem>() else {
            continue;
        };
        for item in &list.children {
            collect_inline_list_enum_locations(engine, introspector, &item.body, &mut locations);
        }
    }

    for elem in introspector.query(&EnumElem::ELEM.select()) {
        let Some(enum_) = elem.to_packed::<EnumElem>() else {
            continue;
        };
        for item in &enum_.children {
            collect_inline_list_enum_locations(engine, introspector, &item.body, &mut locations);
        }
    }

    locations
}

fn collect_inline_list_enum_locations(
    engine: &Engine,
    introspector: &dyn Introspector,
    body: &Content,
    locations: &mut FxHashSet<Location>,
) {
    if let Some(list) = body.query_first_naive(&ListElem::ELEM.select()) {
        if let Some(loc) = list
            .location()
            .or_else(|| location_for_content(engine, introspector, &list))
        {
            locations.insert(loc);
        }
        if let Some(list) = list.to_packed::<ListElem>() {
            for item in &list.children {
                collect_inline_list_enum_locations(engine, introspector, &item.body, locations);
            }
        }
    }
    if let Some(enum_) = body.query_first_naive(&EnumElem::ELEM.select()) {
        if let Some(loc) = enum_
            .location()
            .or_else(|| location_for_content(engine, introspector, &enum_))
        {
            locations.insert(loc);
        }
        if let Some(enum_) = enum_.to_packed::<EnumElem>() {
            for item in &enum_.children {
                collect_inline_list_enum_locations(engine, introspector, &item.body, locations);
            }
        }
    }
}

fn location_for_content(
    engine: &Engine,
    introspector: &dyn Introspector,
    content: &Content,
) -> Option<Location> {
    let target = source_range(engine, content.span())?;
    for selector in [ListElem::ELEM.select(), EnumElem::ELEM.select()] {
        for elem in introspector.query(&selector) {
            if source_range(engine, elem.span()) == Some(target) {
                return elem.location();
            }
        }
    }
    None
}

fn is_nested_list_or_enum(
    engine: &Engine,
    introspector: &dyn Introspector,
    content: &Content,
    list_enum_ranges: &[SourceRange],
    inline_nested_locations: &FxHashSet<Location>,
) -> bool {
    if let Some(loc) = content.location() {
        if inline_nested_locations.contains(&loc) {
            return true;
        }
    }
    if is_inline_nested_list_or_enum(engine, introspector, content) {
        return true;
    }
    let Some(inner) = source_range(engine, content.span()) else {
        return false;
    };
    if list_enum_ranges
        .iter()
        .any(|outer| is_strictly_nested(inner, *outer))
    {
        return true;
    }
    is_list_or_enum_inside_item(engine, introspector, inner)
}

fn is_inline_nested_list_or_enum(
    engine: &Engine,
    introspector: &dyn Introspector,
    content: &Content,
) -> bool {
    let Some(target) = source_range(engine, content.span()) else {
        return false;
    };

    for elem in introspector.query(&ListElem::ELEM.select()) {
        let Some(list) = elem.to_packed::<ListElem>() else {
            continue;
        };
        for item in &list.children {
            if inline_list_or_enum_in_body(engine, &item.body, target) {
                return true;
            }
        }
    }

    for elem in introspector.query(&EnumElem::ELEM.select()) {
        let Some(enum_) = elem.to_packed::<EnumElem>() else {
            continue;
        };
        for item in &enum_.children {
            if inline_list_or_enum_in_body(engine, &item.body, target) {
                return true;
            }
        }
    }

    false
}

fn inline_list_or_enum_in_body(engine: &Engine, body: &Content, target: SourceRange) -> bool {
    for selector in [ListElem::ELEM.select(), EnumElem::ELEM.select()] {
        if let Some(found) = body.query_first_naive(&selector) {
            if source_range(engine, found.span()) == Some(target) {
                return true;
            }
            if let Some(list) = found.to_packed::<ListElem>() {
                for item in &list.children {
                    if inline_list_or_enum_in_body(engine, &item.body, target) {
                        return true;
                    }
                }
            }
            if let Some(enum_) = found.to_packed::<EnumElem>() {
                for item in &enum_.children {
                    if inline_list_or_enum_in_body(engine, &item.body, target) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn is_list_or_enum_inside_item(
    engine: &Engine,
    introspector: &dyn Introspector,
    inner: SourceRange,
) -> bool {
    let item_selectors = [ListItem::ELEM.select(), EnumItem::ELEM.select()];
    for selector in item_selectors {
        for elem in introspector.query(&selector) {
            let Some(item_range) = source_range(engine, elem.span()) else {
                continue;
            };
            if is_strictly_nested(inner, item_range) {
                return true;
            }
        }
    }
    false
}

fn build_skip_ranges(engine: &Engine, introspector: &dyn Introspector) -> Vec<SourceRange> {
    let selectors = [
        QuoteElem::ELEM.select(),
        ListElem::ELEM.select(),
        EnumElem::ELEM.select(),
        FigureElem::ELEM.select(),
    ];
    let mut ranges = Vec::new();
    for selector in selectors {
        for elem in introspector.query(&selector) {
            if let Some(range) = source_range(engine, elem.span()) {
                ranges.push(range);
            }
        }
    }
    ranges
}

fn should_skip_paragraph(
    engine: &Engine,
    par: &Packed<ParElem>,
    content: &Content,
    skip_ranges: &[SourceRange],
    skip_texts: &FxHashSet<String>,
) -> bool {
    if let Some(par_range) = source_range(engine, content.span()) {
        if skip_ranges
            .iter()
            .any(|outer| range_contains(*outer, par_range))
        {
            return true;
        }
    }
    skip_texts.contains(&normalize_paragraph_key(&extract::extract_text(&par.body)))
        || paragraph_text_is_skipped(
            &normalize_paragraph_key(&extract::extract_text(&par.body)),
            skip_texts,
        )
}

fn normalize_paragraph_key(text: &str) -> String {
    text.chars()
        .filter(|ch| {
            !matches!(
                ch,
                '\u{201c}' | '\u{201d}' | '\u{2018}' | '\u{2019}' | '"'
                    | '\u{00ab}' | '\u{00bb}' | '\u{2039}' | '\u{203a}'
                    | '\u{202f}' | '\u{a0}'
            )
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn paragraph_text_is_skipped(key: &str, skip_texts: &FxHashSet<String>) -> bool {
    if key.is_empty() {
        return false;
    }
    if skip_texts.contains(key) {
        return true;
    }
    skip_texts.iter().any(|skip| {
        skip.starts_with(key) || (key.len() >= 10 && key.starts_with(skip))
    })
}

fn push_text_key(skip: &mut FxHashSet<String>, text: &str) {
    let key = normalize_paragraph_key(text);
    if !key.is_empty() {
        skip.insert(key);
    }
}

fn build_skip_paragraph_texts(introspector: &dyn Introspector) -> FxHashSet<String> {
    let mut skip = FxHashSet::default();

    for elem in introspector.query(&QuoteElem::ELEM.select()) {
        let Some(quote) = elem.to_packed::<QuoteElem>() else {
            continue;
        };
        push_text_key(&mut skip, &extract::extract_text(&quote.body));
    }

    for elem in introspector.query(&ListElem::ELEM.select()) {
        let Some(list) = elem.to_packed::<ListElem>() else {
            continue;
        };
        for item in &list.children {
            collect_list_item_texts(item, &mut skip);
        }
    }

    for elem in introspector.query(&EnumElem::ELEM.select()) {
        let Some(enum_) = elem.to_packed::<EnumElem>() else {
            continue;
        };
        for item in &enum_.children {
            collect_enum_item_texts(item, &mut skip);
        }
    }

    for elem in introspector.query(&FigureElem::ELEM.select()) {
        let Some(figure) = elem.to_packed::<FigureElem>() else {
            continue;
        };
        if let Some(caption) = figure.caption.get_cloned(StyleChain::default()) {
            push_text_key(&mut skip, &extract::extract_text(&caption.body));
        }
    }

    skip
}

fn collect_list_item_texts(item: &Packed<ListItem>, skip: &mut FxHashSet<String>) {
    push_text_key(skip, &extract::extract_text(&item.body));
    if let Some(nested) = item.body.query_first_naive(&ListElem::ELEM.select()) {
        if let Some(list) = nested.to_packed::<ListElem>() {
            for child in &list.children {
                collect_list_item_texts(child, skip);
            }
        }
    }
}

fn collect_enum_item_texts(item: &Packed<EnumItem>, skip: &mut FxHashSet<String>) {
    push_text_key(skip, &extract::extract_text(&item.body));
    if let Some(nested) = item.body.query_first_naive(&EnumElem::ELEM.select()) {
        if let Some(list) = nested.to_packed::<EnumElem>() {
            for child in &list.children {
                collect_enum_item_texts(child, skip);
            }
        }
    }
}

/// Build CND nodes from introspector queries (post-layout, stable locations).
pub fn convert_from_introspector(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    styles: StyleChain,
    doc_lang: Option<EcoString>,
    ctx: &mut ConvertContext,
) -> typst_library::diag::SourceResult<()> {
    let skip_ranges = build_skip_ranges(engine, introspector);
    let skip_texts = build_skip_paragraph_texts(introspector);
    let list_enum_ranges = build_list_enum_ranges(engine, introspector);
    let inline_nested_locations = build_inline_nested_list_enum_locations(engine, introspector);
    let doc_selector = doc_selector();
    let mut items: Vec<(Location, Content)> = Vec::new();

    for elem in introspector.query(&FigureElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&TableElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        if table::is_table_in_figure(introspector, &elem) {
            continue;
        }
        items.push((loc, elem));
    }

    for elem in introspector.query(&HeadingElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&QuoteElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&ListElem::ELEM.select()) {
        if is_nested_list_or_enum(engine, introspector, &elem, &list_enum_ranges, &inline_nested_locations) {
            continue;
        }
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&EnumElem::ELEM.select()) {
        if is_nested_list_or_enum(engine, introspector, &elem, &list_enum_ranges, &inline_nested_locations) {
            continue;
        }
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&RawElem::ELEM.select()) {
        let Some(raw) = elem.to_packed::<RawElem>() else { continue };
        if !raw.block.get(styles) {
            continue;
        }
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&EquationElem::ELEM.select()) {
        let Some(equation) = elem.to_packed::<EquationElem>() else { continue };
        if !equation.block.get(styles) {
            continue;
        }
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    for elem in introspector.query(&ParElem::ELEM.select()) {
        let Some(par) = elem.to_packed::<ParElem>() else { continue };
        let Some(loc) = elem.location() else { continue };
        if should_skip_paragraph(engine, par, &elem, &skip_ranges, &skip_texts) {
            continue;
        }
        items.push((loc, elem));
    }

    reading_order::sort_by_reading_order(&mut items, introspector, &doc_selector);

    let mut stack: Vec<HeadingFrame> = Vec::new();
    for (_, content) in items {
        dispatch(
            engine,
            introspector,
            &content,
            styles,
            doc_lang.as_deref(),
            ctx,
            &mut stack,
        )?;
    }
    finalize_headings(ctx, &mut stack);
    Ok(())
}

fn dispatch(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    styles: StyleChain,
    doc_lang: Option<&str>,
    ctx: &mut ConvertContext,
    stack: &mut Vec<HeadingFrame>,
) -> typst_library::diag::SourceResult<()> {
    if let Some(heading) = content.to_packed::<HeadingElem>() {
        emit_heading(engine, introspector, heading, styles, ctx, stack)?;
    } else if let Some(par) = content.to_packed::<ParElem>() {
        let (node, record) = paragraph::convert(engine, introspector, par, styles, doc_lang)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Paragraph(node), ctx, stack);
    } else if let Some(quote) = content.to_packed::<QuoteElem>() {
        let (node, record) = quote::convert(engine, introspector, quote, styles, doc_lang)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Quote(node), ctx, stack);
    } else if let Some(list) = content.to_packed::<ListElem>() {
        let (node, record) = list::from_list(engine, introspector, list, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::List(node), ctx, stack);
    } else if let Some(enum_) = content.to_packed::<EnumElem>() {
        let (node, record) = list::from_enum(engine, introspector, enum_, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::List(node), ctx, stack);
    } else if let Some(raw) = content.to_packed::<RawElem>() {
        let (node, record) = code::convert(engine, introspector, raw, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Code(node), ctx, stack);
    } else if let Some(equation) = content.to_packed::<EquationElem>() {
        let (node, record) = math::convert(engine, introspector, equation, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Math(node), ctx, stack);
    } else if let Some(figure) = content.to_packed::<FigureElem>() {
        if let Some(table_content) = table::table_content_in(content) {
            if let Some(table_elem) = table_content.to_packed::<TableElem>() {
                let (node, record) = table::from_figure(
                    engine,
                    introspector,
                    content,
                    figure,
                    table_elem,
                    styles,
                )?;
                ctx.register(node.base.id, record);
                push_node(CndNode::Table(node), ctx, stack);
            }
        } else if let Some(grid) = table::grid_in_content(content) {
            let (node, record) = table::from_figure_grid(
                engine,
                introspector,
                content,
                figure,
                &grid,
                styles,
            )?;
            ctx.register(node.base.id, record);
            push_node(CndNode::Table(node), ctx, stack);
        } else {
            let (node, record) =
                figure::from_figure(engine, introspector, content, figure, styles)?;
            ctx.register(node.base.id, record);
            push_node(CndNode::Figure(node), ctx, stack);
        }
    } else if let Some(table_elem) = content.to_packed::<TableElem>() {
        let (node, record) = table::convert(
            engine,
            introspector,
            table_elem,
            None,
            None,
            styles,
            None,
            content.label(),
        )?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Table(node), ctx, stack);
    }
    Ok(())
}

fn emit_heading(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    heading: &Packed<HeadingElem>,
    styles: StyleChain,
    ctx: &mut ConvertContext,
    stack: &mut Vec<HeadingFrame>,
) -> typst_library::diag::SourceResult<()> {
    let level = heading.resolve_level(styles).get() as i32;

    while stack.last().is_some_and(|frame| frame.level >= level) {
        let finished = stack.pop().unwrap();
        attach_heading(finished, ctx, stack);
    }

    let (node, record) = heading::convert(engine, introspector, heading, styles, stack)?;
    ctx.register(node.base.id, record);
    stack.push(HeadingFrame {
        level,
        path: node.heading_path.clone(),
        node,
    });

    Ok(())
}

fn attach_heading(frame: HeadingFrame, ctx: &mut ConvertContext, stack: &mut [HeadingFrame]) {
    let node = CndNode::Heading(frame.node);
    if let Some(parent) = stack.last_mut() {
        parent.node.children.push(node);
    } else {
        ctx.roots.push(node);
    }
}

fn push_node(node: CndNode, ctx: &mut ConvertContext, stack: &mut [HeadingFrame]) {
    if let Some(parent) = stack.last_mut() {
        parent.node.children.push(node);
    } else {
        ctx.roots.push(node);
    }
}

pub fn finalize_headings(ctx: &mut ConvertContext, stack: &mut Vec<HeadingFrame>) {
    while let Some(frame) = stack.pop() {
        attach_heading(frame, ctx, stack);
    }
}

pub fn collect_ref_targets(content: &Content) -> Vec<Label> {
    let mut labels = Vec::new();
    let _ = content.traverse(&mut |element| {
        if let Some(reference) = element.to_packed::<RefElem>() {
            labels.push(reference.target);
        }
        ControlFlow::<()>::Continue(())
    });
    labels.sort_by_key(|label| label.resolve().to_string());
    labels.dedup_by_key(|label| label.resolve());
    labels
}

pub fn metadata_at(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    location: Location,
) -> typst_library::diag::SourceResult<std::collections::HashMap<String, serde_json::Value>> {
    crate::metadata::metadata_at(engine, introspector, location)
}

pub fn make_record(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
) -> typst_library::diag::SourceResult<NodeRecord> {
    let location = content.location();
    let label = content.label();
    let mut ref_targets = collect_ref_targets(content);
    ref_targets.dedup_by_key(|label| label.resolve());

    let state_metadata = match location {
        Some(loc) => metadata_at(engine, introspector, loc)?,
        None => std::collections::HashMap::new(),
    };

    Ok(NodeRecord {
        location,
        label,
        ref_targets,
        state_metadata,
    })
}

pub fn apply_metadata(ctx: &mut ConvertContext) {
    fn walk(node: &mut CndNode, records: &rustc_hash::FxHashMap<Uuid, NodeRecord>) {
        let id = node.id();
        if let Some(record) = records.get(&id) {
            if let Some(label) = &record.label {
                set_label(node, label.resolve().as_str());
            }
            if !record.state_metadata.is_empty() {
                set_metadata(node, record.state_metadata.clone());
            }
        }
        if let Some(children) = node.children_mut() {
            for child in children {
                walk(child, records);
            }
        }
    }

    for root in &mut ctx.roots {
        walk(root, &ctx.records);
    }
}

fn set_label(node: &mut CndNode, label: &str) {
    node.base_mut().label = Some(label.into());
}

fn set_metadata(
    node: &mut CndNode,
    metadata: std::collections::HashMap<String, serde_json::Value>,
) {
    node.base_mut().state_metadata = metadata;
}

pub fn figure_number(
    engine: &mut Engine,
    figure: &Packed<FigureElem>,
    styles: StyleChain,
) -> Option<EcoString> {
    let numbering = figure.numbering.get_ref(styles).as_ref()?;
    let location = figure.location()?;
    let counter = figure.counter.as_ref()?.as_ref()?;
    let display = counter
        .display_at(engine, location, styles, numbering, figure.span())
        .ok()?;
    let number = extract::extract_text(&display);

    match figure.supplement.get_ref(styles) {
        typst_library::foundations::Smart::Custom(Some(Supplement::Content(supplement))) => {
            let supplement = extract::extract_text(supplement);
            if supplement.is_empty() {
                Some(number)
            } else {
                Some(ecow::eco_format!("{supplement} {number}"))
            }
        }
        _ => Some(number),
    }
}

pub fn caption_text(caption: &Packed<FigureCaption>) -> EcoString {
    extract::extract_text(&caption.body)
}
