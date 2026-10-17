use std::ops::ControlFlow;

use ecow::{EcoString, eco_vec};
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, Label, NativeElement, Packed, Selector, StyleChain,
};
use typst_library::introspection::{Introspector, Location, Tag, TagElem};
use typst_library::math::EquationElem;
use typst_library::model::{
    CiteElem, EnumElem, FigureCaption, FigureElem, FootnoteElem, HeadingElem, LinkElem,
    ListElem, ParElem, QuoteElem, RefElem, Supplement, TableElem, TermsElem,
};
use typst_library::text::RawElem;
use typst_library::visualize::ImageElem;
use typst_syntax::{FileId, Span};
use uuid::Uuid;

use crate::emit::ancestry::Ancestry;
use crate::emit::extract::{ExtractedMarker, LinkDest, MarkerKind};
use crate::emit::{
    code, extract, figure, heading, list, math, paragraph, quote, reading_order, table,
};
use crate::location::placeholder_location;
use crate::model::{
    CndNode, HeadingNode, ListItem, ListNode, Numbering, ParagraphNode, RichText,
    TextSlot,
};

/// A citation marker occurring in a node's content, captured for
/// resolution into a `CiteRef` once the bibliography pool exists. `span`
/// is the marker's `[start, end)` codepoint offsets in the node's rendered
/// text — `Some` only for flat-text nodes (paragraph/heading/quote), where
/// the text is a single string; `None` elsewhere (ADR 0013).
#[derive(Debug, Clone)]
pub struct CiteMarker {
    pub key: Label,
    pub loc: Location,
    pub form: Option<String>,
    pub supplement: Option<String>,
    pub span: Option<(i64, i64)>,
}

/// The edges captured for one text of a node, before they are resolved
/// into the `refs`/`cites`/`footnotes`/`links` of the `RichText` in `slot`.
#[derive(Debug, Clone)]
pub struct TextRecord {
    pub slot: TextSlot,
    pub ref_targets: Vec<Label>,
    /// Footnote markers occurring in this text: each marker's own location
    /// (resolved to a footnote-pool id in `pools::resolve_footnotes`) paired
    /// with its text span (`Some` only for flat-text nodes).
    pub footnote_locs: Vec<(Location, Option<(i64, i64)>)>,
    /// Citation markers occurring in this text, resolved to `CiteRef` edges
    /// in `pools::resolve_cite_edges`.
    pub cite_markers: Vec<CiteMarker>,
    /// Cross-reference markers with their text spans — only from flat-text
    /// nodes. Consulted in `refs::resolve_refs` to attach a span to the
    /// edge already created from `ref_targets` (never a separate edge).
    pub ref_markers: Vec<(Label, (i64, i64))>,
    /// `LinkElem` destinations found universally (any node, via a bare
    /// content traverse — mirrors `ref_targets`): always `text_span: None`,
    /// since it never sees the flat-text walk's spans (ADR 0013's rule,
    /// generalised to `links` in ADR 0024).
    pub link_targets: Vec<LinkDest>,
    /// `LinkElem` destinations with their text spans — only from flat-text
    /// nodes (mirrors `ref_markers`). Resolved in `refs::resolve_links` and
    /// `refs::resolve_refs` (a label/location destination is a
    /// cross-reference with a custom body).
    pub link_markers: Vec<(LinkDest, (i64, i64))>,
}

impl TextRecord {
    pub fn new(slot: TextSlot) -> Self {
        Self {
            slot,
            ref_targets: Vec::new(),
            footnote_locs: Vec::new(),
            cite_markers: Vec::new(),
            ref_markers: Vec::new(),
            link_targets: Vec::new(),
            link_markers: Vec::new(),
        }
    }

    /// Add `other`'s edges to this record, skipping any it already has, so
    /// that one text never receives the same marker twice.
    pub fn merge(&mut self, other: TextRecord) {
        for (loc, span) in other.footnote_locs {
            if self.footnote_locs.iter().all(|(known, _)| *known != loc) {
                self.footnote_locs.push((loc, span));
            }
        }
        for marker in other.cite_markers {
            if self.cite_markers.iter().all(|known| known.loc != marker.loc) {
                self.cite_markers.push(marker);
            }
        }
        for label in other.ref_targets {
            if !self.ref_targets.contains(&label) {
                self.ref_targets.push(label);
            }
        }
        for marker in other.ref_markers {
            if !self.ref_markers.contains(&marker) {
                self.ref_markers.push(marker);
            }
        }
        for dest in other.link_targets {
            if !self.link_targets.contains(&dest) {
                self.link_targets.push(dest);
            }
        }
        for marker in other.link_markers {
            if !self.link_markers.contains(&marker) {
                self.link_markers.push(marker);
            }
        }
    }
}

/// Metadata captured for each emitted node.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    pub location: Option<Location>,
    pub label: Option<Label>,
    pub state_metadata: serde_json::Map<String, serde_json::Value>,
    /// At most one record per text slot of the node.
    pub texts: Vec<TextRecord>,
}

impl NodeRecord {
    /// The record of a node the emitter builds itself (a container's
    /// paragraph, a nested list): it carries its container's `location`, so
    /// it gets the container's page, and nothing else.
    pub fn synthetic(location: Option<Location>) -> Self {
        Self {
            location,
            label: None,
            state_metadata: serde_json::Map::new(),
            texts: Vec::new(),
        }
    }

    /// Add `text`'s edges to the record of its slot.
    pub fn add_text(&mut self, text: TextRecord) {
        match self.texts.iter_mut().find(|known| known.slot == text.slot) {
            Some(known) => known.merge(text),
            None => self.texts.push(text),
        }
    }
}

/// The records an emitter produced. `own` are the records of the nodes the
/// Typst elements map to, and go through [`ConvertContext::register`].
/// `synthetic` are those of the nodes an emitter builds to fill a container
/// slot: they share their container's location and must not take over its
/// entry in `location_to_id`, so they are inserted as they are.
#[derive(Debug, Default)]
pub struct Records {
    pub own: Vec<(Uuid, NodeRecord)>,
    pub synthetic: Vec<(Uuid, NodeRecord)>,
}

impl Records {
    /// Give the records that have no location of their own `location`, the
    /// location of the container that holds them (a figure's body read from
    /// its unrealized content), so that every node gets a page. They then
    /// share that location, so they are filed as synthetic.
    pub fn locate_in(&mut self, location: Option<Location>) {
        let (unlocated, located): (Vec<_>, Vec<_>) =
            self.own.drain(..).partition(|(_, record)| record.location.is_none());
        self.own = located;
        for (id, mut record) in unlocated {
            record.location = location;
            self.synthetic.push((id, record));
        }
        for (_, record) in &mut self.synthetic {
            if record.location.is_none() {
                record.location = location;
            }
        }
    }
}

/// Builds the synthetic nodes that fill a container's slots: each gets its
/// own id and a record with the container's location.
///
/// Interim shape (until the container bodies are rebuilt from the realized
/// layout): the container's span-less edges move to its first paragraph
/// descendant, see [`Synthetic::attach_to_first_paragraph`].
#[derive(Debug)]
pub struct Synthetic {
    location: Option<Location>,
    pub records: Vec<(Uuid, NodeRecord)>,
}

impl Synthetic {
    pub fn new(location: Option<Location>) -> Self {
        Self { location, records: Vec::new() }
    }

    fn record(&mut self) -> Uuid {
        let id = Uuid::new_v4();
        self.records.push((id, NodeRecord::synthetic(self.location)));
        id
    }

    pub fn paragraph(&mut self, text: RichText) -> CndNode {
        let id = self.record();
        CndNode::Paragraph(ParagraphNode::new(id, text, placeholder_location()))
    }

    pub fn list(&mut self, ordered: bool, items: Vec<ListItem>) -> CndNode {
        let id = self.record();
        let mut list = ListNode::new(id, placeholder_location());
        list.ordered = ordered;
        list.items = items;
        CndNode::List(list)
    }

    /// Give `text` to the first paragraph among `nodes`, in reading order.
    /// Returns it back when there is none.
    /// Give `text` to the first paragraph of the first slot (an item's or
    /// a cell's children) that has one. Returns it back when none does.
    pub fn attach_to_first_slot<'a>(
        &mut self,
        slots: impl IntoIterator<Item = &'a [CndNode]>,
        text: TextRecord,
    ) -> Option<TextRecord> {
        let mut text = text;
        for slot in slots {
            // `None` means attached: stop there.
            text = self.attach_to_first_paragraph(slot, text)?;
        }
        Some(text)
    }

    pub fn attach_to_first_paragraph(
        &mut self,
        nodes: &[CndNode],
        text: TextRecord,
    ) -> Option<TextRecord> {
        let mut first = None;
        crate::model::walk(nodes, &mut |node| {
            if first.is_none() && matches!(node, CndNode::Paragraph(_)) {
                first = Some(node.id());
            }
        });
        let Some(first) = first else { return Some(text) };
        let mut text = text;
        text.slot = TextSlot::Text;
        match self.records.iter_mut().find(|(id, _)| *id == first) {
            Some((_, record)) => {
                record.add_text(text);
                None
            }
            None => Some(text),
        }
    }
}

/// Conversion context shared across the document walk.
#[derive(Debug, Default)]
pub struct ConvertContext {
    pub roots: Vec<CndNode>,
    pub records: rustc_hash::FxHashMap<Uuid, NodeRecord>,
    pub location_to_id: rustc_hash::FxHashMap<Location, Uuid>,
    pub label_to_id: rustc_hash::FxHashMap<Label, Uuid>,
    /// Footnote pool (proposal 0004), populated by `pools::resolve_footnotes`.
    pub footnotes: Vec<crate::model::Footnote>,
    /// Map from a footnote marker's own location (declaration *or*
    /// re-reference) to its pool-entry id. A node's `footnote_locs` are
    /// resolved through this map; both marker kinds land on the same entry.
    pub footnote_loc_to_id: rustc_hash::FxHashMap<Location, Uuid>,
    /// Bibliography pool (proposal 0004), populated by
    /// `pools::resolve_citations`.
    pub bibliography: Vec<crate::model::BibEntry>,
    /// Map from a bibliography `@key` to its pool-entry id. Also used to
    /// keep citation keys out of the cross-reference index (a `@key`
    /// citation is a `RefElem` but resolves in the bibliography, not nodes).
    pub bib_key_to_id: rustc_hash::FxHashMap<Label, Uuid>,
    /// Locations inside a PDF artifact (see `Ancestry::is_artifact`). A
    /// label on decorative content names nothing in the CND (cnd-sdk spec
    /// §2), so `refs::resolve_label` must not fall back to another node.
    pub artifact_locations: rustc_hash::FxHashSet<Location>,
}

impl ConvertContext {
    /// The records, ordered by node id.
    ///
    /// `records` is a hash map, whose iteration order is unspecified. Every
    /// pass that builds edges out of it pushes them in the order it visits
    /// the nodes, and those edges are then deduplicated first-wins — so the
    /// result must not depend on the map's internal layout.
    ///
    /// This does *not* make a CND reproducible across runs: node ids are
    /// `Uuid::new_v4()`, so the order is freshly arbitrary every time. It
    /// buys one thing only — the order is a function of the ids, so two
    /// passes over the same map agree, and an unrelated insertion cannot
    /// silently reshuffle which edge wins.
    pub fn records_sorted(&self) -> Vec<(Uuid, &NodeRecord)> {
        let mut records: Vec<(Uuid, &NodeRecord)> =
            self.records.iter().map(|(id, record)| (*id, record)).collect();
        records.sort_unstable_by_key(|(id, _)| *id);
        records
    }

    pub fn register_records(&mut self, records: Records) {
        for (id, record) in records.own {
            self.register(id, record);
        }
        for (id, record) in records.synthetic {
            if let Some(label) = record.label {
                self.label_to_id.insert(label, id);
            }
            self.records.insert(id, record);
        }
    }

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
        TermsElem::ELEM.select(),
        ImageElem::ELEM.select(),
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
    Some(SourceRange { file, start: range.start, end: range.end })
}

fn range_contains(outer: SourceRange, inner: SourceRange) -> bool {
    outer.file == inner.file && inner.start >= outer.start && inner.end <= outer.end
}

fn build_skip_ranges(
    engine: &Engine,
    introspector: &dyn Introspector,
) -> Vec<SourceRange> {
    let selectors = [
        QuoteElem::ELEM.select(),
        ListElem::ELEM.select(),
        EnumElem::ELEM.select(),
        TermsElem::ELEM.select(),
        FigureElem::ELEM.select(),
        // A heading show rule that re-emits `it.body` inside fresh markup
        // (`it => [ … #text(…, it.body) … ]`) realizes an extra ParElem
        // whose span is the heading's own body — inside the heading's
        // source range, so this selector catches it the same way a quote's
        // body paragraph is caught by the quote's range.
        HeadingElem::ELEM.select(),
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
    content: &Content,
    skip_ranges: &[SourceRange],
) -> bool {
    source_range(engine, content.span()).is_some_and(|par_range| {
        skip_ranges.iter().any(|outer| range_contains(*outer, par_range))
    })
}

/// Build CND nodes from introspector queries (post-layout, stable locations).
pub fn convert_from_introspector(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    styles: StyleChain,
    doc_lang: Option<EcoString>,
    ancestry: &Ancestry,
    ctx: &mut ConvertContext,
) -> typst_library::diag::SourceResult<()> {
    let skip_ranges = build_skip_ranges(engine, introspector);
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

    // Bare images only: the CND model's "bare image outside any figure is
    // an ImageNode with no wrapper". An image inside a figure is emitted as
    // that figure's child (figure::from_figure); one inside a table cell or
    // any other skipped container belongs to its container's text.
    let image_skip_ranges: Vec<SourceRange> = {
        let mut ranges = skip_ranges.clone();
        for elem in introspector.query(&TableElem::ELEM.select()) {
            if let Some(range) = source_range(engine, elem.span()) {
                ranges.push(range);
            }
        }
        ranges
    };
    for elem in introspector.query(&ImageElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        if let Some(range) = source_range(engine, elem.span())
            && image_skip_ranges.iter().any(|outer| range_contains(*outer, range))
        {
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

    // A list laid out inside another list, an enum, a definition list, a
    // quote or a footnote entry is that node's content: nested items are its
    // children, a description, a quote or the footnote pool carries the text.
    for selector in [ListElem::ELEM.select(), EnumElem::ELEM.select()] {
        for elem in introspector.query(&selector) {
            let Some(loc) = elem.location() else { continue };
            if ancestry.is_list_carried(loc) {
                continue;
            }
            items.push((loc, elem));
        }
    }

    for elem in introspector.query(&TermsElem::ELEM.select()) {
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

    // Paragraphs skipped as part of an enclosing one, kept so that enclosing
    // paragraph can absorb their edges.
    let mut absorbed: Vec<(Location, Content)> = Vec::new();

    for elem in introspector.query(&ParElem::ELEM.select()) {
        let Some(par) = elem.to_packed::<ParElem>() else { continue };
        let Some(loc) = elem.location() else { continue };
        // The body of an inline `box`, realized into a paragraph of its own.
        // The paragraph containing the box renders that text itself, so this
        // one is a duplicate — but it is the only copy that carries the
        // box's footnote and citation tags, which are absorbed below.
        if let Some(parent) = ancestry.covering_par(loc) {
            absorbed.push((parent, elem.clone()));
            continue;
        }
        // Likewise for a paragraph inside a list, table, code block or
        // equation: that container is emitted as its own node and renders
        // this text itself.
        if ancestry.is_in_emitted_container(loc) {
            continue;
        }
        if should_skip_paragraph(engine, &elem, &skip_ranges) {
            continue;
        }
        // A paragraph made only of text that belongs to another node (a
        // heading body re-emitted by a show rule, a boxed block quote) or to
        // no node (a PDF artifact) has nothing of its own left. One that was
        // empty to begin with is not affected.
        if extract::is_emptied_by_exclusion(&par.body) {
            continue;
        }
        items.push((loc, elem));
    }

    // Page furniture (running headers and footers, page decorations) is
    // marked as a PDF artifact by Typst's accessibility model, which defines
    // it as not being document content. It is repeated on every page, so
    // emitting it would add one copy of the footer — and one page number —
    // per page.
    items.retain(|(loc, _)| !ancestry.is_artifact(*loc));

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
    absorb_covered_paragraphs(ctx, &absorbed);
    finalize_headings(ctx, &mut stack);
    Ok(())
}

/// Move the footnote and citation edges of a paragraph that was skipped as
/// part of an enclosing one onto that enclosing paragraph.
///
/// The enclosing paragraph's own content holds the box body *unrealized*, so
/// a footnote or citation written inside the box is a bare element there —
/// which `collect_footnote_locs` and `collect_cite_markers` deliberately
/// ignore, since they read the introspection tags that only realized content
/// carries. Skipping the realized paragraph without this would take the edge
/// with it.
///
/// The absorbed markers carry no text span: their offsets index the skipped
/// paragraph's text, not the text the surviving node exposes. A span-less
/// edge is what a list item or a table cell already produces.
fn absorb_covered_paragraphs(ctx: &mut ConvertContext, absorbed: &[(Location, Content)]) {
    for (parent, nested) in absorbed {
        let Some(id) = ctx.location_to_id.get(parent).copied() else { continue };
        let footnotes = collect_footnote_locs(nested);
        let cites = collect_cite_markers(nested);
        let refs = collect_ref_targets(nested);
        let links = collect_link_targets(nested);
        let Some(record) = ctx.records.get_mut(&id) else { continue };

        let mut text = TextRecord::new(TextSlot::Text);
        text.footnote_locs = footnotes.into_iter().map(|loc| (loc, None)).collect();
        text.cite_markers = cites;
        text.ref_targets = refs;
        text.link_targets = links;
        record.add_text(text);
    }
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
        let (node, record) =
            paragraph::convert(engine, introspector, par, styles, doc_lang)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Paragraph(node), ctx, stack);
    } else if let Some(quote) = content.to_packed::<QuoteElem>() {
        let (node, records) =
            quote::convert(engine, introspector, quote, styles, doc_lang)?;
        ctx.register_records(records);
        push_node(CndNode::Quote(node), ctx, stack);
    } else if let Some(list) = content.to_packed::<ListElem>() {
        let (node, records) = list::from_list(engine, introspector, list, styles)?;
        ctx.register_records(records);
        push_node(CndNode::List(node), ctx, stack);
    } else if let Some(enum_) = content.to_packed::<EnumElem>() {
        let (node, records) = list::from_enum(engine, introspector, enum_, styles)?;
        ctx.register_records(records);
        push_node(CndNode::List(node), ctx, stack);
    } else if let Some(terms) = content.to_packed::<TermsElem>() {
        let (node, records) = list::from_terms(engine, introspector, terms)?;
        ctx.register_records(records);
        push_node(CndNode::Terms(node), ctx, stack);
    } else if let Some(raw) = content.to_packed::<RawElem>() {
        let (node, record) = code::convert(engine, introspector, raw, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Code(node), ctx, stack);
    } else if let Some(equation) = content.to_packed::<EquationElem>() {
        let (node, record) = math::convert(engine, introspector, equation, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Math(node), ctx, stack);
    } else if let Some(image) = content.to_packed::<ImageElem>() {
        let (node, record) =
            figure::bare_image(engine, introspector, content, image, styles)?;
        ctx.register(node.base.id, record);
        push_node(CndNode::Image(node), ctx, stack);
    } else if let Some(figure) = content.to_packed::<FigureElem>() {
        if let Some(table_content) = table::table_content_in(content) {
            if let Some(table_elem) = table_content.to_packed::<TableElem>() {
                let (node, records) = table::from_figure(
                    engine,
                    introspector,
                    content,
                    figure,
                    table_elem,
                    styles,
                )?;
                ctx.register_records(records);
                push_node(CndNode::Figure(node), ctx, stack);
            }
        } else if let Some(grid) = table::grid_in_content(content) {
            let (node, records) = table::from_figure_grid(
                engine,
                introspector,
                content,
                figure,
                &grid,
                styles,
            )?;
            ctx.register_records(records);
            push_node(CndNode::Figure(node), ctx, stack);
        } else {
            let (node, records) =
                figure::from_figure(engine, introspector, content, figure, styles)?;
            ctx.register_records(records);
            push_node(CndNode::Figure(node), ctx, stack);
        }
    } else if let Some(table_elem) = content.to_packed::<TableElem>() {
        let (node, records) = table::convert(engine, introspector, table_elem, styles)?;
        ctx.register_records(records);
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
    stack.push(HeadingFrame { level, path: node.heading_path.clone(), node });

    Ok(())
}

fn attach_heading(
    frame: HeadingFrame,
    ctx: &mut ConvertContext,
    stack: &mut [HeadingFrame],
) {
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
    extract::traverse_kept(content, &mut |element| {
        if let Some(reference) = element.to_packed::<RefElem>() {
            labels.push(reference.target);
        }
    });
    labels.sort_by_key(|label| label.resolve().to_string());
    labels.dedup_by_key(|label| label.resolve());
    labels
}

/// Collect `LinkElem` destinations occurring anywhere in `content`,
/// universally (mirrors `collect_ref_targets` exactly, bare-match only) —
/// always paired with `text_span: None` in `refs::resolve_links`/
/// `refs::resolve_refs`, since a span requires the flat-text walk
/// (`ExtractedMarker`, ADR 0013).
///
/// Deliberately bare-only, not `TagElem`-unwrapping like
/// `collect_cite_markers`/`collect_footnote_locs`: a realized (flat) node's
/// `LinkElem` is *only* reachable under a `TagElem` (`fields()` skips the
/// tag's `#[internal]` payload, so a bare traverse cannot see it there),
/// while a non-flat node's content (a list item, a table cell) is captured
/// bare, exactly as authored. Matching both shapes here — as cite/footnote
/// do — would double-count a flat node's links: they are already carried,
/// spanned, in `link_markers`, and `refs::resolve_links` combines both
/// lists additively (unlike cite/footnote's single, location-keyed list).
fn collect_link_targets(content: &Content) -> Vec<LinkDest> {
    let mut out = Vec::new();
    extract::traverse_kept(content, &mut |element| {
        if let Some(link) = element.to_packed::<LinkElem>()
            && let Some(dest) = extract::link_dest(&link.dest)
        {
            out.push(dest);
        }
    });
    out
}

pub fn metadata_at(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    location: Location,
) -> typst_library::diag::SourceResult<serde_json::Map<String, serde_json::Value>> {
    crate::metadata::metadata_at(engine, introspector, location)
}

/// Build a node's record and the edges of its content. `markers` carries
/// the text-span markers from the flat-text walk of the node's body
/// (paragraph/heading/quote); it is empty for non-flat nodes, whose marker
/// *locations* are still captured (so their footnote/cite edges resolve)
/// but whose `text_span`s stay `None` (ADR 0013).
///
/// The edges come back apart, in a `TextRecord` for the `Text` slot: the
/// caller decides which text holds them (its own, or for a container the
/// text of a paragraph inside it). The returned `NodeRecord` has no texts.
pub fn make_record(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    markers: &[ExtractedMarker],
) -> typst_library::diag::SourceResult<(NodeRecord, TextRecord)> {
    let location = content.location();
    let label = content.label();
    let mut ref_targets = collect_ref_targets(content);
    ref_targets.dedup_by_key(|label| label.resolve());

    // Span lookups from the flat-text walk (empty for non-flat nodes).
    let mut cite_span: rustc_hash::FxHashMap<Location, (i64, i64)> =
        rustc_hash::FxHashMap::default();
    let mut footnote_span: rustc_hash::FxHashMap<Location, (i64, i64)> =
        rustc_hash::FxHashMap::default();
    let mut ref_markers: Vec<(Label, (i64, i64))> = Vec::new();
    let mut link_markers: Vec<(LinkDest, (i64, i64))> = Vec::new();
    for marker in markers {
        match &marker.kind {
            MarkerKind::Cite(loc) => {
                cite_span.entry(*loc).or_insert((marker.start, marker.end));
            }
            MarkerKind::Footnote(loc) => {
                footnote_span.entry(*loc).or_insert((marker.start, marker.end));
            }
            MarkerKind::Ref(label) => {
                ref_markers.push((*label, (marker.start, marker.end)));
            }
            MarkerKind::Link(dest) => {
                link_markers.push((dest.clone(), (marker.start, marker.end)));
            }
        }
    }

    let link_targets = collect_link_targets(content);

    // Universal marker-location capture (all node types) + span layering.
    let footnote_locs = collect_footnote_locs(content)
        .into_iter()
        .map(|loc| (loc, footnote_span.get(&loc).copied()))
        .collect();
    let mut cite_markers = collect_cite_markers(content);
    for cite in &mut cite_markers {
        cite.span = cite_span.get(&cite.loc).copied();
    }

    let state_metadata = match location {
        Some(loc) => metadata_at(engine, introspector, loc)?,
        None => serde_json::Map::new(),
    };

    let record = NodeRecord { location, label, state_metadata, texts: Vec::new() };
    let text = TextRecord {
        slot: TextSlot::Text,
        ref_targets,
        footnote_locs,
        cite_markers,
        ref_markers,
        link_targets,
        link_markers,
    };
    Ok((record, text))
}

/// Collect citation markers (location + form/supplement) occurring in
/// `content`, in reading order — universally, from a full element traverse,
/// so cites inside any node (including list items and table cells) produce
/// an edge. Text spans are layered on separately in `make_record`.
fn collect_cite_markers(content: &Content) -> Vec<CiteMarker> {
    let mut markers = Vec::new();
    let _ = content.traverse(&mut |element| {
        if let Some(tag) = element.to_packed::<TagElem>()
            && let Tag::Start(inner, _) = &tag.tag
            && let Some(cite) = inner.to_packed::<CiteElem>()
            && let Some(loc) = inner.location()
        {
            let form = extract::citation_form_str(cite.form.get(StyleChain::default()));
            let supplement = cite
                .supplement
                .get_cloned(StyleChain::default())
                .map(|c| extract::extract_text(&c).into());
            markers.push(CiteMarker { key: cite.key, loc, form, supplement, span: None });
        }
        ControlFlow::<()>::Continue(())
    });
    markers
}

/// Collect the locations of footnote markers occurring in `content`.
///
/// The realized tree no longer carries the `FootnoteElem` in the flow (the
/// note body is pulled to the page bottom), but it leaves an introspection
/// `TagElem` at the marker position whose `Tag::Start` holds the original
/// footnote element. That is the reliable per-node signal: a node whose
/// content holds such a tag references that footnote (proposal 0004).
fn collect_footnote_locs(content: &Content) -> Vec<Location> {
    let mut locs = Vec::new();
    let _ = content.traverse(&mut |element| {
        if let Some(tag) = element.to_packed::<TagElem>()
            && let Tag::Start(inner, _) = &tag.tag
            && inner.to_packed::<FootnoteElem>().is_some()
            && let Some(loc) = inner.location()
            && !locs.contains(&loc)
        {
            locs.push(loc);
        }
        ControlFlow::<()>::Continue(())
    });
    locs
}

pub fn apply_metadata(ctx: &mut ConvertContext) {
    let records = &ctx.records;
    crate::model::walk_mut(&mut ctx.roots, &mut |node| {
        let Some(record) = records.get(&node.id()) else { return };
        if let Some(label) = &record.label {
            set_label(node, label.resolve().as_str());
        }
        if !record.state_metadata.is_empty() {
            set_metadata(node, record.state_metadata.clone());
        }
    });
}

fn set_label(node: &mut CndNode, label: &str) {
    node.base_mut().label = Some(label.into());
}

fn set_metadata(
    node: &mut CndNode,
    metadata: serde_json::Map<String, serde_json::Value>,
) {
    node.base_mut().state_metadata = metadata;
}

/// A displayed counter value and its word, as a `Numbering`; `None` when
/// nothing is displayed.
pub fn numbering(
    value: Option<impl Into<String>>,
    name: Option<impl Into<String>>,
) -> Option<Numbering> {
    let value: String = value?.into();
    if value.is_empty() {
        return None;
    }
    Some(Numbering { value, name: name.map(Into::into) })
}

/// The figure's counter value **as resolved and displayed** — `"3"`,
/// `"2.1"` — and nothing else.
///
/// The supplement ("Figure", "Table", "Abbildung") is deliberately not
/// prepended: it goes to `Numbering.name`. It is a localized rendering word, and baking it into data
/// makes the field unusable in any other locale; composing the prefix is a
/// consumer's decision, and `FigureNode::kind` already carries the counter
/// selector it composes from (spec §6).
///
/// The word itself is captured separately by `figure_counter_label`.
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
    Some(extract::extract_text(&display))
}

/// The figure's supplement — the word displayed in front of the number
/// ("Figure", "Tabelle", "Atom"), emitted as `Numbering.name` (spec §6).
///
/// Typst has already resolved this during `synthesize()`: either from the
/// element kind and the document language, or from the author for a custom
/// `kind`, where no localized name exists and Typst requires one. The
/// second case is why this cannot be derived downstream — nothing else in
/// the CND encodes the author's word.
pub fn figure_counter_label(
    figure: &Packed<FigureElem>,
    styles: StyleChain,
) -> Option<EcoString> {
    // Only meaningful next to a number; an unnumbered figure displays no
    // counter at all, so a label for it would be a word nothing precedes.
    figure.numbering.get_ref(styles).as_ref()?;
    match figure.supplement.get_ref(styles) {
        typst_library::foundations::Smart::Custom(Some(Supplement::Content(
            supplement,
        ))) => {
            let text = extract::extract_text(supplement);
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

pub fn caption_text(caption: &Packed<FigureCaption>) -> EcoString {
    extract::extract_text(&caption.body)
}
