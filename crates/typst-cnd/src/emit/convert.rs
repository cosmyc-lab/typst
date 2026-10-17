use std::ops::ControlFlow;

use ecow::{EcoString, eco_vec};
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, Label, NativeElement, Packed, Selector, StyleChain,
};
use typst_library::introspection::{Introspector, Location, Tag, TagElem};
use typst_library::layout::GridElem;
use typst_library::math::EquationElem;
use typst_library::model::{
    Attribution, CiteElem, EnumElem, FigureElem, FootnoteElem, HeadingElem, LinkElem,
    ListElem, ParElem, QuoteElem, RefElem, Supplement, TableElem, TermsElem,
};
use typst_library::text::RawElem;
use typst_library::visualize::ImageElem;
use typst_library::{World, WorldExt};
use typst_syntax::{FileId, Span};
use uuid::Uuid;

use crate::emit::ancestry::{Ancestry, SlotKey, SlotKind};
use crate::emit::extract::{ExtractedMarker, LinkDest, MarkerKind};
use crate::emit::{
    code, extract, figure, heading, list, math, paragraph, quote, reading_order, table,
};
use crate::location::placeholder_location;
use crate::model::{
    Cell, CndNode, HeadingNode, Numbering, ParagraphNode, RichText, TextSlot,
};

/// A citation marker occurring in a node's content, captured for
/// resolution into a `CiteRef` once the bibliography pool exists. `span`
/// is the marker's `[start, end)` codepoint offsets in the node's rendered
/// text — `Some` only for flat-text nodes (paragraph/heading), where
/// the text is a single string; `None` elsewhere (ADR 0013).
#[derive(Debug, Clone)]
pub struct CiteMarker {
    pub key: Label,
    /// The marker's own location; `None` for a citation met as authored (a
    /// label text, see `extract::label_text`), which has none.
    pub loc: Option<Location>,
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
    /// The span is `None` when it reached partway into a `code` or `math`
    /// mark.
    pub ref_markers: Vec<(Label, Option<(i64, i64)>)>,
    /// `LinkElem` destinations found universally (any node, via a bare
    /// content traverse — mirrors `ref_targets`): always `text_span: None`,
    /// since it never sees the flat-text walk's spans (ADR 0013's rule,
    /// generalised to `links` in ADR 0024).
    pub link_targets: Vec<LinkDest>,
    /// `LinkElem` destinations with their text spans — only from flat-text
    /// nodes (mirrors `ref_markers`). Resolved in `refs::resolve_links` and
    /// `refs::resolve_refs` (a label/location destination is a
    /// cross-reference with a custom body).
    pub link_markers: Vec<(LinkDest, Option<(i64, i64)>)>,
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
            let known = |known: &CiteMarker| match (known.loc, marker.loc) {
                (Some(a), Some(b)) => a == b,
                _ => {
                    known.key == marker.key
                        && known.form == marker.form
                        && known.supplement == marker.supplement
                }
            };
            if !self.cite_markers.iter().any(known) {
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

/// Builds the paragraphs a container falls back to when nothing is routed
/// into its body (a footnote with no printed entry, a quote whose body a
/// show rule boxes): each gets its own id and a record with the location
/// it is given, so it gets that page.
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

    /// A paragraph holding `text`, whose edges are `edges`.
    pub fn paragraph_with(&mut self, text: RichText, edges: TextRecord) -> CndNode {
        let node = self.paragraph(text);
        if let Some((_, record)) = self.records.last_mut() {
            let mut edges = edges;
            edges.slot = TextSlot::Text;
            record.add_text(edges);
        }
        node
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
    /// Map from a printed footnote entry's location to its pool-entry id:
    /// the content laid out in that entry is the entry's `children`.
    pub footnote_entry_to_pool: rustc_hash::FxHashMap<Location, Uuid>,
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

    /// The node with `id`, in the body or in a footnote's content.
    pub fn node_mut(&mut self, id: Uuid) -> Option<&mut CndNode> {
        if let Some(node) = crate::emit::refs::find_node_mut(&mut self.roots, id) {
            return Some(node);
        }
        self.footnotes
            .iter_mut()
            .find_map(|note| crate::emit::refs::find_node_mut(&mut note.children, id))
    }

    /// Visit every node: the body, then each footnote's content.
    pub fn walk_all(&self, f: &mut impl FnMut(&CndNode)) {
        crate::model::walk(&self.roots, f);
        for note in &self.footnotes {
            crate::model::walk(&note.children, f);
        }
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

/// The slot kinds whose content is routed into its container's node by
/// [`assemble`]: every container's body is.
const ROUTED: &[SlotKind] = &[
    SlotKind::Item,
    SlotKind::TermDescription,
    SlotKind::Cell,
    SlotKind::QuoteBody,
    SlotKind::FigureBody,
    SlotKind::FootnoteBody,
];

/// Active heading container while assembling a list of nodes.
struct HeadingFrame {
    level: i32,
    path: Vec<String>,
    node: HeadingNode,
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

/// The source ranges whose paragraphs are not nodes: those of the
/// figures, which carry their caption themselves, and those of the headings.
struct SkipRanges {
    /// Figures. A figure body's paragraphs are routed into its slot, so
    /// this only catches what is written in a figure outside its body.
    containers: Vec<SourceRange>,
    /// A heading show rule that re-emits `it.body` inside fresh markup
    /// (`it => [ … #text(…, it.body) … ]`) realizes an extra `ParElem` whose
    /// span is the heading's own body — inside the heading's source range,
    /// so this catches it the same way a quote's body paragraph is caught by
    /// the quote's range. It applies inside a slot too.
    headings: Vec<SourceRange>,
}

impl SkipRanges {
    fn all(&self) -> impl Iterator<Item = &SourceRange> {
        self.containers.iter().chain(&self.headings)
    }
}

fn build_skip_ranges(engine: &Engine, introspector: &dyn Introspector) -> SkipRanges {
    let ranges = |selectors: &[Selector]| {
        let mut ranges = Vec::new();
        for selector in selectors {
            for elem in introspector.query(selector) {
                if let Some(range) = source_range(engine, elem.span()) {
                    ranges.push(range);
                }
            }
        }
        ranges
    };
    SkipRanges {
        containers: ranges(&[FigureElem::ELEM.select()]),
        headings: ranges(&[HeadingElem::ELEM.select()]),
    }
}

/// What a block quote's body and attribution are made of, to tell the
/// paragraph its attribution is laid out in from those of its body.
///
/// Membership is by the spans of the elements themselves, never by a hull
/// of their source ranges: a body that uses content defined earlier by a
/// `#let` has a hull reaching across everything written in between, the
/// attribution included.
struct QuoteSource {
    /// The spans of everything the body holds.
    body: rustc_hash::FxHashSet<Span>,
    /// What the attribution's paragraph holds.
    attribution: Option<AttributionSource>,
}

enum AttributionSource {
    /// The spans of everything the attribution content holds.
    Content(rustc_hash::FxHashSet<Span>),
    /// A label, realized as a citation spanned at the quote itself.
    Label(Span),
}

/// Each block quote's [`QuoteSource`], by its location.
fn quote_sources(
    introspector: &dyn Introspector,
) -> rustc_hash::FxHashMap<Location, QuoteSource> {
    let mut out = rustc_hash::FxHashMap::default();
    for elem in introspector.query(&QuoteElem::ELEM.select()) {
        let Some(quote) = elem.to_packed::<QuoteElem>() else { continue };
        let Some(location) = elem.location() else { continue };
        let materialized = StyleChain::default();
        let attribution = match quote.attribution.get_ref(materialized) {
            Some(Attribution::Content(content)) => Some(AttributionSource::Content(
                crate::emit::ancestry::body_spans(content),
            )),
            Some(Attribution::Label(_)) => Some(AttributionSource::Label(elem.span())),
            None => None,
        };
        out.insert(
            location,
            QuoteSource {
                body: crate::emit::ancestry::body_spans(&quote.body),
                attribution,
            },
        );
    }
    out
}

/// Whether the paragraph `par`, laid out in the body of a block quote, is
/// the one its attribution is laid out in: something in it is part of the
/// attribution, and nothing in it is part of the body only. Decided by the
/// elements' spans, never by comparing texts.
fn is_attribution(quote: &QuoteSource, par: &Content) -> bool {
    let Some(attribution) = &quote.attribution else { return false };
    let mut in_body = false;
    let mut in_attribution = false;
    let _ = par.traverse(&mut |element: Content| {
        let span = element.span();
        if span.is_detached() {
            return ControlFlow::Continue(());
        }
        let of_attribution = match attribution {
            AttributionSource::Content(spans) => spans.contains(&span),
            AttributionSource::Label(at) => *at == span,
        };
        if of_attribution {
            in_attribution = true;
        } else if quote.body.contains(&span) {
            in_body = true;
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    });
    !in_body && in_attribution
}

fn should_skip_paragraph<'a>(
    engine: &Engine,
    content: &Content,
    mut skip_ranges: impl Iterator<Item = &'a SourceRange>,
) -> bool {
    source_range(engine, content.span()).is_some_and(|par_range| {
        skip_ranges.any(|outer| range_contains(*outer, par_range))
    })
}

/// Whether a list, a table, a grid or a block quote laid out at `location`
/// becomes a node: not when a node already carries its text — an inline `box`'s
/// paragraph, a heading — or when it is decorative (an artifact), and so
/// is never its owner.
fn admits_block(ancestry: &Ancestry, location: Location) -> bool {
    ancestry.covering_par(location).is_none() && !ancestry.is_absorbed(location)
}

/// Whether the element at `location` is in a slot that [`assemble`] routes.
fn is_routed(ancestry: &Ancestry, location: Location) -> bool {
    ancestry
        .slot_of(location)
        .is_some_and(|slot| ROUTED.contains(&slot.kind()))
}

/// Build CND nodes from introspector queries (post-layout, stable locations).
pub fn convert_from_introspector(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    styles: StyleChain,
    doc_lang: EcoString,
    ancestry: &Ancestry,
    ctx: &mut ConvertContext,
) -> typst_library::diag::SourceResult<()> {
    let skip_ranges = build_skip_ranges(engine, introspector);
    let quote_sources = quote_sources(introspector);
    let doc_selector = doc_selector();
    let mut items: Vec<(Location, Content)> = Vec::new();

    for elem in introspector.query(&FigureElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    // A table in a figure is routed into the figure's body.
    for elem in introspector.query(&TableElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        if !admits_block(ancestry, loc) {
            continue;
        }
        items.push((loc, elem));
    }

    // Every grid is a `layout`, those of templates, show rules and figures
    // included. The introspector does not know a grid: its anchor stands in
    // for it (see `Ancestry::grids`).
    let mut anchors: rustc_hash::FxHashMap<Location, Location> =
        rustc_hash::FxHashMap::default();
    for (loc, elem, anchor) in ancestry.grids() {
        let Some(anchor) = anchor else { continue };
        if !admits_block(ancestry, loc) {
            continue;
        }
        anchors.insert(loc, anchor);
        items.push((loc, elem.clone()));
    }

    // An image is a node of its own: bare in the flow ("a bare image
    // outside any figure is an ImageNode with no wrapper"), or routed into
    // the figure, cell, item, quote or footnote it is laid out in. One
    // inside a heading belongs to its text, one inside a caption to its
    // figure.
    for elem in introspector.query(&ImageElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        if ancestry.is_absorbed(loc) {
            continue;
        }
        if !is_routed(ancestry, loc)
            && let Some(range) = source_range(engine, elem.span())
            && skip_ranges.all().any(|outer| range_contains(*outer, range))
        {
            continue;
        }
        items.push((loc, elem));
    }

    for elem in introspector.query(&HeadingElem::ELEM.select()) {
        let Some(loc) = elem.location() else { continue };
        items.push((loc, elem));
    }

    // Only a block quote is a node (cnd-sdk spec §6.4). An inline quote is
    // text of its paragraph, quotation marks included. The element is
    // materialized, so `block` holds its realized value.
    for elem in introspector.query(&QuoteElem::ELEM.select()) {
        let Some(quote) = elem.to_packed::<QuoteElem>() else { continue };
        if !quote.block.get(StyleChain::default()) {
            continue;
        }
        let Some(loc) = elem.location() else { continue };
        if !admits_block(ancestry, loc) {
            continue;
        }
        items.push((loc, elem));
    }

    // A list in a list item, a term's description, a cell, a quote, a
    // figure or a footnote entry is routed into it by `assemble`.
    for selector in [ListElem::ELEM.select(), EnumElem::ELEM.select()] {
        for elem in introspector.query(&selector) {
            let Some(loc) = elem.location() else { continue };
            if !admits_block(ancestry, loc) {
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
    // The paragraphs block quotes' attributions are laid out in, by quote:
    // the attribution is a text of the quote's own, and these paragraphs
    // carry its edges.
    let mut attributions: Vec<(Location, Content)> = Vec::new();

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
        // A list item's marker, a footnote's number, a heading's body…
        if ancestry.is_absorbed(loc) {
            continue;
        }
        if is_routed(ancestry, loc) {
            // A paragraph of a list item, a term's description, a cell, a
            // quote, a figure's body or a footnote entry is a node of that
            // slot, whatever container is around it.
            if should_skip_paragraph(engine, &elem, skip_ranges.headings.iter()) {
                continue;
            }
            // A block quote's attribution is a text of the quote's own.
            if let Some(SlotKey::QuoteBody(quote)) = ancestry.slot_of(loc)
                && let Some(source) = quote_sources.get(&quote)
                && is_attribution(source, &elem)
            {
                attributions.push((quote, elem.clone()));
                continue;
            }
        } else if should_skip_paragraph(engine, &elem, skip_ranges.all()) {
            continue;
        }
        // A paragraph made only of text that belongs to another node (a
        // heading body re-emitted by a show rule, a boxed block quote) or to
        // no node (a PDF artifact) has nothing of its own left.
        // An inline equation's text is its source, so a paragraph holding
        // only `$z$` is kept.
        let source = |span| extract::source_text(engine, span);
        if extract::is_emptied_by_exclusion(&par.body, &source) {
            continue;
        }
        // A paragraph with no text at all — a `box` holding a shape, say —
        // says nothing.
        if extract::extract_rich(&par.body, &source).value.trim().is_empty() {
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

    reading_order::sort_by_reading_order(
        &mut items,
        introspector,
        &doc_selector,
        &anchors,
    );

    let mut nodes = Vec::with_capacity(items.len());
    for (location, content) in items {
        let site = Site {
            anchor: anchors.get(&location).copied(),
            drawing: content.is::<FigureElem>() && ancestry.is_drawing(location),
            // A text's language is written only where it differs from the
            // document's, which every text inherits (spec §4.1).
            lang: ancestry.lang_of(location).filter(|lang| *lang != doc_lang),
        };
        if let Some(node) = dispatch(engine, introspector, &content, site, styles, ctx)? {
            nodes.push((location, node));
        }
    }
    // The footnote pool exists before its content is routed into it.
    crate::emit::pools::build_footnote_pool(engine, introspector, styles, ancestry, ctx)?;
    ctx.roots = assemble(
        nodes,
        ancestry,
        ROUTED,
        &mut ctx.footnotes,
        &ctx.footnote_entry_to_pool,
    );
    forget_dropped_nodes(ctx);
    absorb_covered_paragraphs(ctx, &absorbed);
    attach_attributions(ctx, &attributions);
    apply_label_langs(ctx, ancestry, &attributions, &doc_lang);
    Ok(())
}

/// Give each caption, term and attribution its language where it differs
/// from the document's, read from the glyphs laid out in its own scope (see
/// `Ancestry::lang_of`): a caption's, a term's label, or the paragraph an
/// attribution is laid out in. Their text is walked as authored, which
/// carries no styles.
fn apply_label_langs(
    ctx: &mut ConvertContext,
    ancestry: &Ancestry,
    attributions: &[(Location, Content)],
    doc_lang: &str,
) {
    let differs = |lang: Option<EcoString>| lang.filter(|lang| lang != doc_lang);
    let mut langs: Vec<(Uuid, TextSlot, EcoString)> = Vec::new();
    let mut located: Vec<(Location, Uuid)> =
        ctx.location_to_id.iter().map(|(loc, id)| (*loc, *id)).collect();
    located.sort_unstable_by_key(|(_, id)| *id);
    for (location, id) in located {
        if let Some(lang) = differs(ancestry.label_lang_of(location)) {
            langs.push((id, TextSlot::Caption, lang));
        }
        for (index, slot) in ancestry.item_slots(location).iter().enumerate() {
            if slot.kind() == SlotKind::TermDescription
                && let Some(lang) = differs(ancestry.label_lang_of(slot.location()))
            {
                langs.push((id, TextSlot::Term(index), lang));
            }
        }
    }
    for (quote, par) in attributions {
        let Some(id) = ctx.location_to_id.get(quote).copied() else { continue };
        if let Some(lang) = differs(par.location().and_then(|loc| ancestry.lang_of(loc)))
        {
            langs.push((id, TextSlot::Attribution, lang));
        }
    }
    for (id, slot, lang) in langs {
        if let Some(text) = ctx.node_mut(id).and_then(|node| node.text_mut(slot)) {
            text.lang = Some(lang.into());
        }
    }
}

/// Give each block quote's attribution the citations and footnotes of the
/// paragraph it is laid out in: that paragraph is the only one realized
/// with their tags. The value differs from the paragraph's (the em dash
/// before it), so the edges carry no span. A label attribution is realized
/// as a citation of that label, which this carries.
fn attach_attributions(ctx: &mut ConvertContext, attributions: &[(Location, Content)]) {
    for (quote, par) in attributions {
        let Some(id) = ctx.location_to_id.get(quote).copied() else { continue };
        let Some(record) = ctx.records.get_mut(&id) else { continue };
        let mut text = realized_edges(par);
        text.slot = TextSlot::Attribution;
        // References and links come from the attribution's own text
        // (`quote::convert`), with the spans this paragraph cannot give.
        text.ref_targets.clear();
        text.link_targets.clear();
        record.add_text(text);
    }
}

/// The edges of realized content, all without a span: its footnote and
/// citation tags, and the references and links its text walk finds.
fn realized_edges(content: &Content) -> TextRecord {
    let mut text = TextRecord::new(TextSlot::Text);
    text.footnote_locs = collect_footnote_locs(content)
        .into_iter()
        .map(|loc| (loc, None))
        .collect();
    text.cite_markers = collect_cite_markers(content);
    let markers = extract::extract_rich(content, &extract::no_source).markers;
    for marker in markers {
        match marker.kind {
            MarkerKind::Ref(label) if !text.ref_targets.contains(&label) => {
                text.ref_targets.push(label);
            }
            MarkerKind::Link(dest) if !text.link_targets.contains(&dest) => {
                text.link_targets.push(dest);
            }
            MarkerKind::MathCite(cite) => text.cite_markers.push(math_cite_marker(&cite)),
            _ => {}
        }
    }
    text
}

/// Forget the records of the nodes `assemble` dropped, so that no label,
/// location or edge resolves to a node the document does not hold.
fn forget_dropped_nodes(ctx: &mut ConvertContext) {
    let mut kept = rustc_hash::FxHashSet::default();
    ctx.walk_all(&mut |node| {
        kept.insert(node.id());
    });
    ctx.records.retain(|id, _| kept.contains(id));
    ctx.location_to_id.retain(|_, id| kept.contains(id));
    ctx.label_to_id.retain(|_, id| kept.contains(id));
}

/// Place each node into the list that holds it, in reading order.
///
/// A node in a slot of a kind in `routed` goes to its slot's owner: a list
/// item's children, a term's description. Every other node takes the
/// heading-stack path: it is a child of the heading it follows, or a root.
/// Each slot's list runs a heading stack of its own, seeded with the path
/// of the headings around its owner, so that a heading in a list item
/// gathers the nodes after it in that item only, and its `heading_path`
/// continues the section it sits in.
///
/// The content of each printed footnote entry in `entry_to_pool` becomes the
/// `children` of its pool entry in `footnotes`. A footnote with no routed
/// content keeps the children it has.
///
/// A node whose slot's owner yields no node — an artifact's list, the entry
/// of a decorative footnote — is dropped with its slot, never re-parented.
fn assemble(
    items: Vec<(Location, CndNode)>,
    ancestry: &Ancestry,
    routed: &[SlotKind],
    footnotes: &mut [crate::model::Footnote],
    entry_to_pool: &rustc_hash::FxHashMap<Location, Uuid>,
) -> Vec<CndNode> {
    let mut flow = Vec::new();
    let mut slots: rustc_hash::FxHashMap<SlotKey, Vec<(Location, CndNode)>> =
        rustc_hash::FxHashMap::default();
    for (location, node) in items {
        match ancestry.slot_of(location) {
            Some(slot) if routed.contains(&slot.kind()) => {
                slots.entry(slot).or_default().push((location, node));
            }
            _ => flow.push((location, node)),
        }
    }
    let mut assembler = Assembler { ancestry, slots };
    let roots = assembler.nodes(flow, &[]);
    let pool_to_entry: rustc_hash::FxHashMap<Uuid, Location> =
        entry_to_pool.iter().map(|(entry, pool)| (*pool, *entry)).collect();
    for note in footnotes {
        let Some(entry) = pool_to_entry.get(&note.id) else { continue };
        if let Some(items) = assembler.slots.remove(&SlotKey::FootnoteBody(*entry)) {
            note.children = assembler.nodes(items, &[]);
        }
    }
    // Whatever is left in `slots` afterwards belongs to an owner that was
    // never placed: it is dropped.
    roots
}

struct Assembler<'a> {
    ancestry: &'a Ancestry,
    slots: rustc_hash::FxHashMap<SlotKey, Vec<(Location, CndNode)>>,
}

impl Assembler<'_> {
    /// Nest `items` under their headings, below the section `seed`.
    fn nodes(
        &mut self,
        items: Vec<(Location, CndNode)>,
        seed: &[String],
    ) -> Vec<CndNode> {
        let mut out = Vec::new();
        let mut stack: Vec<HeadingFrame> = Vec::new();
        for (location, node) in items {
            match node {
                CndNode::Heading(mut heading) => {
                    while stack.last().is_some_and(|frame| frame.level >= heading.level) {
                        let finished = stack.pop().unwrap();
                        attach_heading(finished, &mut stack, &mut out);
                    }
                    let segment = heading.heading_path.pop().unwrap_or_default();
                    let mut path = stack
                        .last()
                        .map_or_else(|| seed.to_vec(), |frame| frame.path.clone());
                    path.push(segment);
                    heading.heading_path = path.clone();
                    stack.push(HeadingFrame {
                        level: heading.level,
                        path,
                        node: heading,
                    });
                }
                mut node => {
                    let path = stack.last().map_or(seed, |frame| frame.path.as_slice());
                    self.fill(location, &mut node, path);
                    match stack.last_mut() {
                        Some(parent) => parent.node.children.push(node),
                        None => out.push(node),
                    }
                }
            }
        }
        while let Some(frame) = stack.pop() {
            attach_heading(frame, &mut stack, &mut out);
        }
        out
    }

    /// Fill the slots of `node`, laid out at `location` in the section
    /// `path`, with the nodes routed into them.
    fn fill(&mut self, location: Location, node: &mut CndNode, path: &[String]) {
        let slots: Vec<(&mut Vec<CndNode>, SlotKey)> = match node {
            CndNode::List(list) => {
                let mut items: Vec<&mut Vec<CndNode>> =
                    list.items.iter_mut().map(|item| &mut item.children).collect();
                self.fill_items(location, &mut items, path);
                return;
            }
            CndNode::Terms(terms) => {
                let mut items: Vec<&mut Vec<CndNode>> =
                    terms.items.iter_mut().map(|item| &mut item.children).collect();
                self.fill_items(location, &mut items, path);
                return;
            }
            CndNode::Table(table) => self.cell_slots(location, &mut table.cells),
            CndNode::Layout(layout) => self.cell_slots(location, &mut layout.cells),
            // A drawing keeps no children: what is laid out in its body is
            // left in its slot, and dropped (its `raw` holds the source).
            CndNode::Figure(figure) if !self.ancestry.is_drawing(location) => {
                vec![(&mut figure.children, SlotKey::FigureBody(location))]
            }
            // With nothing routed, a quote keeps its fallback paragraph (see
            // `quote::convert`).
            CndNode::Quote(quote) => {
                vec![(&mut quote.children, SlotKey::QuoteBody(location))]
            }
            _ => return,
        };
        for (children, key) in slots {
            if let Some(items) = self.slots.remove(&key) {
                *children = self.nodes(items, path);
            }
        }
    }

    /// Fill the items of the list, enum or term list at `location`: item k
    /// takes the k-th body laid out for it.
    ///
    /// Typst lays out one body per item, so the counts agree (the debug
    /// assertion fails loudly in tests if they ever do not). Should they
    /// differ, no text is dropped: bodies beyond the last item go to the
    /// last item, and items beyond the last body stay empty.
    fn fill_items(
        &mut self,
        location: Location,
        items: &mut [&mut Vec<CndNode>],
        path: &[String],
    ) {
        let keys = self.ancestry.item_slots(location).to_vec();
        debug_assert_eq!(items.len(), keys.len(), "one body per item");
        let Some(last) = items.len().checked_sub(1) else { return };
        for (index, key) in keys.into_iter().enumerate() {
            if let Some(routed) = self.slots.remove(&key) {
                let nodes = self.nodes(routed, path);
                items[index.min(last)].extend(nodes);
            }
        }
    }

    /// Pair each cell of the table or grid at `location` with its slot, by
    /// its position.
    fn cell_slots<'c>(
        &self,
        location: Location,
        cells: &'c mut [Cell],
    ) -> Vec<(&'c mut Vec<CndNode>, SlotKey)> {
        let slots: Vec<_> = cells
            .iter_mut()
            .filter_map(|cell| {
                let key = self.ancestry.cell_slot(
                    location,
                    cell.col as usize,
                    cell.row as usize,
                )?;
                Some((&mut cell.children, key))
            })
            .collect();
        debug_assert_eq!(
            slots.len(),
            self.ancestry.cell_slot_count(location),
            "every laid-out cell is a cell of its table"
        );
        slots
    }
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
        let text = unrealized_edges(nested);
        let Some(record) = ctx.records.get_mut(&id) else { continue };
        record.add_text(text);
    }
}

/// The edges `content` shows without a text walk, all without a span: its
/// footnote and citation tags, and its bare references and links (content
/// met unrealized holds those as elements, not tags).
pub fn unrealized_edges(content: &Content) -> TextRecord {
    let mut text = TextRecord::new(TextSlot::Text);
    text.footnote_locs = collect_footnote_locs(content)
        .into_iter()
        .map(|loc| (loc, None))
        .collect();
    text.cite_markers = collect_cite_markers(content);
    text.ref_targets = collect_ref_targets(content);
    text.link_targets = collect_link_targets(content);
    text
}

/// What the frames say about an element that its content does not.
#[derive(Debug, Clone)]
struct Site {
    /// For a grid, the element that stands in for its location (see
    /// `Ancestry::grids`).
    anchor: Option<Location>,
    /// For a figure, whether it is a drawing (see `Ancestry::is_drawing`).
    drawing: bool,
    /// The language of the element's text, when it differs from the
    /// document's (see `Ancestry::lang_of`).
    lang: Option<EcoString>,
}

/// Convert one element into its node, registering the records.
fn dispatch(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    site: Site,
    styles: StyleChain,
    ctx: &mut ConvertContext,
) -> typst_library::diag::SourceResult<Option<CndNode>> {
    let node = if let Some(heading) = content.to_packed::<HeadingElem>() {
        let (node, record) =
            heading::convert(engine, introspector, heading, styles, site.lang)?;
        ctx.register(node.base.id, record);
        CndNode::Heading(node)
    } else if let Some(par) = content.to_packed::<ParElem>() {
        let (node, record) = paragraph::convert(engine, introspector, par, site.lang)?;
        ctx.register(node.base.id, record);
        CndNode::Paragraph(node)
    } else if let Some(quote) = content.to_packed::<QuoteElem>() {
        let (node, records) =
            quote::convert(engine, introspector, quote, styles, site.lang)?;
        ctx.register_records(records);
        CndNode::Quote(node)
    } else if let Some(list) = content.to_packed::<ListElem>() {
        let (node, record) = list::from_list(engine, introspector, list)?;
        ctx.register(node.base.id, record);
        CndNode::List(node)
    } else if let Some(enum_) = content.to_packed::<EnumElem>() {
        let (node, record) = list::from_enum(engine, introspector, enum_, styles)?;
        ctx.register(node.base.id, record);
        CndNode::List(node)
    } else if let Some(terms) = content.to_packed::<TermsElem>() {
        let (node, record) = list::from_terms(engine, introspector, terms)?;
        ctx.register(node.base.id, record);
        CndNode::Terms(node)
    } else if let Some(raw) = content.to_packed::<RawElem>() {
        let (node, record) = code::convert(engine, introspector, raw, styles)?;
        ctx.register(node.base.id, record);
        CndNode::Code(node)
    } else if let Some(equation) = content.to_packed::<EquationElem>() {
        let (node, record) = math::convert(engine, introspector, equation, styles)?;
        ctx.register(node.base.id, record);
        CndNode::Math(node)
    } else if let Some(image) = content.to_packed::<ImageElem>() {
        let (node, record) =
            figure::bare_image(engine, introspector, content, image, styles)?;
        ctx.register(node.base.id, record);
        CndNode::Image(node)
    } else if let Some(figure) = content.to_packed::<FigureElem>() {
        let (node, record) = figure::from_figure(
            engine,
            introspector,
            content,
            figure,
            site.drawing,
            styles,
        )?;
        ctx.register(node.base.id, record);
        CndNode::Figure(node)
    } else if let Some(table_elem) = content.to_packed::<TableElem>() {
        let (node, record) = table::convert(engine, introspector, table_elem, styles)?;
        ctx.register(node.base.id, record);
        CndNode::Table(node)
    } else if let Some(grid) = content.to_packed::<GridElem>() {
        let Some(anchor) = site.anchor else { return Ok(None) };
        let (node, record) =
            table::from_grid(engine, introspector, grid, anchor, styles)?;
        // The anchor is another element's location: it gives the layout a
        // page and state, never its own entry in `location_to_id`.
        ctx.register_records(Records {
            own: Vec::new(),
            synthetic: vec![(node.base.id, record)],
        });
        CndNode::Layout(node)
    } else {
        return Ok(None);
    };
    Ok(Some(node))
}

fn attach_heading(
    frame: HeadingFrame,
    stack: &mut [HeadingFrame],
    out: &mut Vec<CndNode>,
) {
    let node = CndNode::Heading(frame.node);
    match stack.last_mut() {
        Some(parent) => parent.node.children.push(node),
        None => out.push(node),
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
/// (paragraph/heading); it is empty for non-flat nodes, whose marker
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
    let mut ref_markers: Vec<(Label, Option<(i64, i64)>)> = Vec::new();
    let mut link_markers: Vec<(LinkDest, Option<(i64, i64)>)> = Vec::new();
    for marker in markers {
        match &marker.kind {
            MarkerKind::Cite(loc) => {
                if let Some(span) = marker.span {
                    cite_span.entry(*loc).or_insert(span);
                }
            }
            MarkerKind::Footnote(loc) => {
                if let Some(span) = marker.span {
                    footnote_span.entry(*loc).or_insert(span);
                }
            }
            MarkerKind::Ref(label) => {
                ref_markers.push((*label, marker.span));
            }
            MarkerKind::Link(dest) => {
                link_markers.push((dest.clone(), marker.span));
            }
            MarkerKind::MathCite(_) => {}
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
        cite.span = cite.loc.and_then(|loc| cite_span.get(&loc).copied());
    }
    // A citation in an inline equation has no tags: the walk reads it from
    // the equation's body (`extract::math_edges`).
    for marker in markers {
        if let MarkerKind::MathCite(cite) = &marker.kind {
            cite_markers.push(math_cite_marker(cite));
        }
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

/// The marker of a citation written in an inline equation: no location, no
/// span (see `extract::math_edges`).
fn math_cite_marker(cite: &Packed<CiteElem>) -> CiteMarker {
    CiteMarker {
        key: cite.key,
        loc: None,
        form: extract::citation_form_str(cite.form.get(StyleChain::default())),
        supplement: extract::cite_supplement(cite),
        span: None,
    }
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
            let supplement = extract::cite_supplement(cite);
            markers.push(CiteMarker {
                key: cite.key,
                loc: Some(loc),
                form,
                supplement,
                span: None,
            });
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
    let mut apply = |node: &mut CndNode| {
        if let Some(record) = records.get(&node.id()) {
            if let Some(label) = &record.label {
                set_label(node, label.resolve().as_str());
            }
            if !record.state_metadata.is_empty() {
                set_metadata(node, record.state_metadata.clone());
            }
        }
        promote_hints(node);
    };
    crate::model::walk_mut(&mut ctx.roots, &mut apply);
    for note in &mut ctx.footnotes {
        crate::model::walk_mut(&mut note.children, &mut apply);
    }
}

/// The state key under which the authoring package's `cnd.table` holds the
/// author's table kind, around the table it wraps.
const TABLE_KIND_HINT: &str = "table-kind";

/// Turn the authoring hints a node's metadata holds into the fields they
/// stand for, and take them out of the metadata.
///
/// A string under `table-kind` on a table becomes its `kind` (cnd-sdk spec
/// §6.3): the kind comes from the author only, never from a guess. Any
/// other value says nothing. Everything laid out inside the bracketed
/// table sees the hint too (its cells' paragraphs, a figure around it), so
/// the key leaves every node's metadata.
fn promote_hints(node: &mut CndNode) {
    let Some(hint) = node.base_mut().state_metadata.remove(TABLE_KIND_HINT) else {
        return;
    };
    if let CndNode::Table(table) = node
        && let serde_json::Value::String(kind) = hint
    {
        table.kind = Some(kind);
    }
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

/// The source text `span` covers, trimmed: what an author wrote for an
/// element, kept as its `raw` when the model cannot express it.
pub fn source_text(engine: &Engine, span: Span) -> Option<String> {
    let id = span.id()?;
    let range = engine.world.range(span)?;
    let source = engine.world.source(id).ok()?;
    let text = source.text();
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start >= end {
        return None;
    }
    Some(text[start..end].trim().to_string())
}
