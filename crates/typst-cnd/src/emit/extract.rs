use std::ops::{ControlFlow, Range};

use ecow::EcoString;
use typst_library::World;
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, Label, NativeElement, Packed, PlainText, Smart, StyleChain, Value,
};
use typst_library::introspection::{Location, Tag, TagElem};
use typst_library::math::{AttachElem, EquationElem, FracElem, RootElem};
use typst_library::model::{
    ArtifactElem, CitationForm, CiteElem, Destination, EmphElem, EnumElem, FigureElem,
    FootnoteElem, HeadingElem, LinkElem, LinkMarker, LinkTarget, ListElem, PdfMarkerTag,
    PdfMarkerTagKind, QuoteElem, RefElem, StrongElem, Supplement, TableElem, TermsElem,
};
use typst_library::text::{
    HighlightElem, LinebreakElem, RawElem, StrikeElem, SubElem, SuperElem, UnderlineElem,
};
use typst_library::visualize::ImageElem;
use typst_syntax::{FileId, Span};

use crate::rich_text::{
    CODE, EMPH, HIGHLIGHT, MATH, Mark, RawSource, STRIKE, STRONG, SUB, SUP, UNDERLINE,
    canonical_marks, trim_rebase,
};

/// Extract plain text from content without duplicating inline code.
///
/// Unlike `Content::plain_text()`, this stops at elements that implement
/// `PlainText` and does not recurse into their synthesized children. This
/// prevents `RawElem` text from appearing three times (once from `RawElem`,
/// once from the synthesized `RawLine`, once from the `TextElem` inside
/// `RawLine.body`).
///
/// With no source at hand, an inline equation contributes its linear form
/// ([`math_linear`]).
pub fn extract_text(content: &Content) -> EcoString {
    extract_rich(content, &no_source).value.into()
}

/// The Typst source a span covers: its file, its byte range there and the
/// text verbatim.
#[derive(Debug, Clone)]
pub struct SpanSource {
    pub file: FileId,
    pub range: Range<usize>,
    pub text: String,
}

/// How a walk reads the source of a span.
pub type SourceLookup<'a> = &'a dyn Fn(Span) -> Option<SpanSource>;

/// A source lookup that never finds anything.
pub fn no_source(_: Span) -> Option<SpanSource> {
    None
}

/// The Typst source `span` covers, read from the engine's world. `None` for
/// a detached span or a file the world cannot give back.
pub fn source_text(engine: &Engine, span: Span) -> Option<SpanSource> {
    let file = span.id()?;
    let source = engine.world.source(file).ok()?;
    let range = source.find(span)?.range();
    let text = source.text().get(range.clone())?.to_owned();
    Some(SpanSource { file, range, text })
}

/// The source of this inline equation instance, without its `$` delimiters
/// and trimmed.
///
/// `None` when the source does not describe this instance: an equation
/// built in code (no `$…$`), or one whose body holds content written
/// outside its own source range. That is an equation in a function's body
/// filled with an argument (`#let sq(a) = $#a^2$`, called as `#sq[y]`):
/// its source is `#a^2`, while the page shows `y^2`.
fn instance_source(
    equation: &Content,
    body: &Content,
    lookup: SourceLookup<'_>,
) -> Option<String> {
    let own = lookup(equation.span())?;
    let inner = own.text.strip_prefix('$')?.strip_suffix('$')?.trim().to_owned();
    let foreign = body.traverse(&mut |element| {
        let span = element.span();
        if span.is_detached() {
            return ControlFlow::Continue(());
        }
        match lookup(span) {
            Some(src)
                if src.file == own.file
                    && own.range.start <= src.range.start
                    && src.range.end <= own.range.end =>
            {
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Break(()),
        }
    });
    foreign.is_continue().then_some(inner)
}

/// The text an inline equation contributes to a text and the `raw` of its
/// `math` mark: its source when that source is this instance's
/// ([`instance_source`]), its linear form ([`math_linear`]) with no `raw`
/// otherwise. The formula never vanishes.
fn equation_text(
    equation: &Content,
    body: &Content,
    lookup: Option<SourceLookup<'_>>,
) -> (String, Option<RawSource>) {
    match lookup.and_then(|lookup| instance_source(equation, body, lookup)) {
        Some(text) => (text.clone(), Some(RawSource::typst(text))),
        None => (math_linear(body), None),
    }
}

/// A linear, readable form of an unrealized equation body, for an equation
/// whose source is not this instance's: its text and symbols in order, an
/// attachment as `base_b^t`, a fraction as `num/denom` and a root as
/// `√x`, any part longer than one character in parentheses (`y^2`,
/// `(a+b)/2`). It is not Typst source, so a mark over it carries no `raw`.
pub fn math_linear(body: &Content) -> String {
    let mut out = String::new();
    linearize(body, &mut out);
    out.trim().to_owned()
}

fn linearize(content: &Content, out: &mut String) {
    let materialized = StyleChain::default();
    if let Some(attach) = content.to_packed::<AttachElem>() {
        let part = |field: Option<Content>| field.as_ref().map(linear_part);
        let tl = part(attach.tl.get_cloned(materialized));
        let bl = part(attach.bl.get_cloned(materialized));
        let t = part(
            attach
                .t
                .get_cloned(materialized)
                .or(attach.tr.get_cloned(materialized)),
        );
        let b = part(
            attach
                .b
                .get_cloned(materialized)
                .or(attach.br.get_cloned(materialized)),
        );
        for (mark, part) in [('_', bl), ('^', tl)] {
            if let Some(part) = part {
                out.push(mark);
                out.push_str(&part);
            }
        }
        linearize(&attach.base, out);
        for (mark, part) in [('_', b), ('^', t)] {
            if let Some(part) = part {
                out.push(mark);
                out.push_str(&part);
            }
        }
        return;
    }
    if let Some(frac) = content.to_packed::<FracElem>() {
        out.push_str(&linear_part(&frac.num));
        out.push('/');
        out.push_str(&linear_part(&frac.denom));
        return;
    }
    if let Some(root) = content.to_packed::<RootElem>() {
        out.push('√');
        out.push_str(&linear_part(&root.radicand));
        return;
    }
    if let Some(textable) = content.with::<dyn PlainText>() {
        let mut text = EcoString::new();
        textable.plain_text(&mut text);
        out.push_str(&text);
        return;
    }
    for (_, value) in content.fields() {
        linearize_value(value, out);
    }
}

fn linearize_value(value: Value, out: &mut String) {
    match value {
        Value::Content(c) => linearize(&c, out),
        Value::Array(arr) => {
            for v in arr {
                linearize_value(v, out);
            }
        }
        _ => {}
    }
}

/// One operand of an attachment, fraction or root, in parentheses when it
/// is longer than one character.
fn linear_part(content: &Content) -> String {
    let text = math_linear(content);
    if text.chars().count() > 1 { format!("({text})") } else { text }
}

/// Whether the text realized inside `content` was all excluded — every word
/// of it sits inside an element emitted as its own node or inside a PDF
/// artifact — leaving nothing of its own.
///
/// Content that was empty to begin with is not "emptied": with no exclusion
/// at all this is `false`, so a paragraph that never held text keeps
/// whatever handling it had before exclusion existed. An inline equation
/// counts as text: its source, read through `source`.
pub fn is_emptied_by_exclusion(content: &Content, source: SourceLookup<'_>) -> bool {
    let mut out = EcoString::new();
    let mut ctx = MarkerCtx { source: Some(source), ..MarkerCtx::default() };
    walk(content, &mut out, &mut ctx);
    ctx.excluded_any && out.trim().is_empty()
}

/// The mark kind an element's realized text carries (spec §4.1), if any.
/// An inline raw is `code` and an inline equation `math`; their block forms
/// are nodes of their own.
fn mark_kind(inner: &Content) -> Option<&'static str> {
    let materialized = StyleChain::default();
    let kind = if inner.is::<StrongElem>() {
        STRONG
    } else if inner.is::<EmphElem>() {
        EMPH
    } else if inner.is::<UnderlineElem>() {
        UNDERLINE
    } else if inner.is::<StrikeElem>() {
        STRIKE
    } else if inner.is::<HighlightElem>() {
        HIGHLIGHT
    } else if inner.is::<SuperElem>() {
        SUP
    } else if inner.is::<SubElem>() {
        SUB
    } else if let Some(raw) = inner.to_packed::<RawElem>() {
        if raw.block.get(materialized) {
            return None;
        }
        CODE
    } else if let Some(equation) = inner.to_packed::<EquationElem>() {
        if equation.block.get(materialized) {
            return None;
        }
        MATH
    } else {
        return None;
    };
    Some(kind)
}

/// Whether the text realized between this element's `Tag::Start` and
/// `Tag::End` belongs somewhere other than the enclosing node's text: to a
/// node the exporter emits on its own (heading, list, enum, terms, figure,
/// table, block quote, raw block, block equation), to a text of its own (a
/// term, which a term's description paragraph opens with), or to no node at
/// all (a PDF artifact, which Typst's accessibility model defines as
/// decoration; a term's separator is one).
///
/// Realization can wrap such an element's tags inside a paragraph — a show
/// rule that boxes a quote, or re-emits a heading's body in fresh markup —
/// and the paragraph would then repeat that element's text. Matching the
/// element itself, rather than comparing texts, leaves alone a paragraph that
/// merely starts with the same words.
///
/// The element carried by a `Tag::Start` was materialized during
/// realization: every settable field, `block` included, already holds the
/// value its style chain gave it at that point (so `#set quote(block: true)`
/// counts). Reading the field with an empty chain therefore reads that
/// realized value, not a default.
fn is_excluded_scope(inner: &Content) -> bool {
    let elem = inner.elem();
    if elem == ArtifactElem::ELEM
        || elem == HeadingElem::ELEM
        || elem == ListElem::ELEM
        || elem == EnumElem::ELEM
        || elem == TermsElem::ELEM
        || elem == FigureElem::ELEM
        || elem == TableElem::ELEM
    {
        return true;
    }
    if let Some(marker) = inner.to_packed::<PdfMarkerTag>() {
        return marker.kind == PdfMarkerTagKind::TermsItemLabel;
    }
    let materialized = StyleChain::default();
    if let Some(quote) = inner.to_packed::<QuoteElem>() {
        return quote.block.get(materialized);
    }
    if let Some(raw) = inner.to_packed::<RawElem>() {
        return raw.block.get(materialized);
    }
    if let Some(equation) = inner.to_packed::<EquationElem>() {
        return equation.block.get(materialized);
    }
    false
}

/// Whether `inner` is the number a printed footnote entry starts with: a
/// `Label` marker that no footnote marker's tags hold. The marker in the
/// text renders its number inside a `Label` marker too, but inside the
/// footnote's own tags, and that number is the edge's span. A custom
/// `show footnote.entry` rule that drops the number leaves no `Label`
/// marker, and so nothing is excluded.
fn is_entry_number(inner: &Content, ctx: &MarkerCtx<'_>) -> bool {
    ctx.footnotes.is_empty()
        && inner
            .to_packed::<PdfMarkerTag>()
            .is_some_and(|marker| marker.kind == PdfMarkerTagKind::Label)
}

/// A reference/citation/footnote marker located within a node's rendered
/// text: its kind (carrying the resolution payload) and the `[start, end)`
/// Unicode code-point offsets of its rendered marker in that text.
///
/// The span is `None` when it reaches partway into a `code` or `math` mark
/// (spec §4.1 rule 4): such a span cannot be expressed on the text.
#[derive(Debug, Clone)]
pub struct ExtractedMarker {
    pub kind: MarkerKind,
    pub span: Option<(i64, i64)>,
}

/// A node's text as realized: the plain value, its canonical marks
/// (spec §4.1) and the reference/citation/footnote/link markers in it, every
/// offset in Unicode code points of `value`.
#[derive(Debug, Clone, Default)]
pub struct Extracted {
    pub value: String,
    pub marks: Vec<Mark>,
    pub markers: Vec<ExtractedMarker>,
    /// The citation and footnote markers written in a term, which a term's
    /// description paragraph opens with: they are edges of the term
    /// (`label_text`), never of the paragraph, which must leave them out of
    /// the edges it collects by location.
    pub suppressed: Vec<Location>,
}

#[derive(Debug, Clone)]
pub enum MarkerKind {
    /// Cross-reference (`@label`); payload is the target label.
    Ref(Label),
    /// Citation; payload is the marker's own location. The citation's key /
    /// form / supplement are captured separately (universally, for every
    /// node) by `convert::collect_cite_markers`; this marker only carries
    /// the span, matched to that cite by location.
    Cite(Location),
    /// Footnote; payload is the marker's own location.
    Footnote(Location),
    /// `LinkElem` (ADR 0024, cnd-sdk); payload is the narrowed destination
    /// (see [`LinkDest`]). The marker's own `Tag::Start`/`Tag::End` bracket
    /// the whole link body, so this is captured like `Cite`/`Footnote`
    /// (`open_frame`) — never through the ref/`LinkMarker` pending
    /// mechanism, which is a different marker's rendered text.
    Link(LinkDest),
}

/// A `LinkElem`'s destination, narrowed to what a CND can durably point at
/// (the three-row mapping in ADR 0024): a URL (a `links` edge), or a
/// label/location naming another node in this document (a `refs` edge,
/// with a custom body). A page/point position has no label and is dropped
/// by [`link_dest`] before it ever becomes a marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkDest {
    Url(EcoString),
    Label(Label),
    Location(Location),
}

/// Map a `LinkElem`'s raw (unresolved) `dest` field to what this fork
/// carries forward. `dest` is a `LinkTarget`, not a resolved `Destination`:
/// resolution (`LinkTarget::resolve_early`) happens inside `LinkElem`'s show
/// rule, not on the tag this reads from, so a `#link(<label>)[..]` is seen
/// here as `LinkTarget::Label` directly, not yet turned into a `Location`.
pub(crate) fn link_dest(target: &LinkTarget) -> Option<LinkDest> {
    match target {
        LinkTarget::Label(label) => Some(LinkDest::Label(*label)),
        LinkTarget::Dest(Destination::Url(url)) => {
            Some(LinkDest::Url(url.clone().into_inner()))
        }
        LinkTarget::Dest(Destination::Location(loc)) => Some(LinkDest::Location(*loc)),
        // Not durable: a page/point position carries no label to survive a
        // rebuild, and is dropped rather than emitted (design's row 3).
        LinkTarget::Dest(Destination::Position(_)) => None,
    }
}

/// Extract plain text and, in the same walk, its inline marks and the
/// code-point spans of any reference/citation/footnote/link markers it
/// contains.
///
/// The realized flow no longer carries the `RefElem`/`CiteElem`/
/// `FootnoteElem` itself, but leaves an introspection `TagElem` pair
/// (`Tag::Start(inner)` … rendered marker text … `Tag::End(loc)`) around
/// the marker's rendered text. Because those tags are interleaved in the
/// exact content tree this walk visits, offsets are computed *as the text
/// is built* — no substring search, correct by construction. Spans are
/// therefore offsets into the returned text, which is the value a flat
/// node uses verbatim for its `text` field.
///
/// Marks come from the same tags: the elements [`mark_kind`] names open a
/// mark at their `Tag::Start` and close it at their `Tag::End`. An inline
/// equation's text is its Typst source, read through `source`, with the
/// realized formula suppressed; its mark carries that source as `raw`
/// (spec §4.1). Marks and spans come back canonical.
pub fn extract_rich(content: &Content, source: SourceLookup<'_>) -> Extracted {
    let mut out = EcoString::new();
    let mut ctx = MarkerCtx { source: Some(source), ..MarkerCtx::default() };
    walk(content, &mut out, &mut ctx);

    // A `LinkElem` whose `Tag::Start` opens within this walk but whose
    // matching `Tag::End` does not — left open here — would otherwise be
    // silently discarded: the edge disappears entirely instead of getting
    // the null span the design calls for when a marker "does not close
    // inside [the] node's rendered text". Flush any still-open `Link` frame
    // with a degenerate `[start, start)` span; `refs::normalize_link_span`
    // turns that into `text_span: None`.
    //
    // This is `Link`-only on purpose: cite/footnote frames are NOT flushed
    // here. Cite/footnote already have their own universal,
    // `TagElem`-unwrapping collectors (`convert::collect_cite_markers`/
    // `collect_footnote_locs`) that capture their locations regardless of
    // node flatness — flushing them here too would duplicate that path and
    // is an unrequested behavior change beyond this task's scope.
    //
    // What this flush does and does not reach (verified against real
    // compiles, not derived from reading alone): it rescues a link whose
    // `Tag::Start` is *interior* to the enclosing paragraph group's trigger
    // range — e.g. `Leading text #link(url)[body that continues onto a
    // second paragraph.]`, where "Leading text" and the link open in the
    // same group. It does **not** reach a *paragraph-initial* link, whose
    // `Tag::Start` sits immediately before any group's trigger range: per
    // `typst-realize/src/lib.rs::finish_grouping`, `PAR` grouping keeps tags
    // (`tags: true`) and includes a tag when it is interior to the trigger
    // range, or its partner is within the range or in the immediately
    // adjacent run of tags — a paragraph-initial link's `Tag::End` (on the
    // far side of a paragraph break) satisfies neither, so both tags stay
    // in the outer flow and never enter any node's walk at all. A
    // `#block`/`#image` body is a separate case with the same visible
    // effect for a different reason: `BlockElem`/`ImageElem` are
    // `Interrupt` for `PAR`, so no paragraph group forms around the link at
    // the flow level in the first place, and the block's own inner
    // paragraph is realized without ever seeing the link's tags.
    let MarkerCtx {
        open,
        mut done,
        open_marks,
        mut marks,
        pos,
        suppressed,
        ..
    } = ctx;
    for frame in open {
        if matches!(frame.kind, MarkerKind::Link(_)) {
            done.push(ExtractedMarker {
                kind: frame.kind,
                span: Some((frame.start, frame.start)),
            });
        }
    }
    // A mark whose element does not end inside this text covers the rest of
    // it.
    for frame in open_marks {
        marks.push(Mark {
            start: frame.start,
            end: pos,
            kind: frame.kind.into(),
            raw: frame.raw,
        });
    }
    let mut spans: Vec<&mut Option<(i64, i64)>> =
        done.iter_mut().map(|marker| &mut marker.span).collect();
    let marks = canonical_marks(pos, marks, &mut spans);
    Extracted {
        value: out.into(),
        marks,
        markers: done,
        suppressed,
    }
}

#[derive(Default)]
struct MarkerCtx<'a> {
    /// Where an inline equation's source is read from.
    source: Option<SourceLookup<'a>>,
    /// Code-point position in `out` so far.
    pos: i64,
    /// Open mark frames, by their element's location.
    open_marks: Vec<MarkFrame>,
    /// Closed marks, in closing order.
    marks: Vec<Mark>,
    /// Open inline equations whose source stands in for their realized
    /// text, which is suppressed.
    math: Vec<Location>,
    /// Open cite/footnote frames — those tags bracket their own rendered
    /// marker text — keyed by the marker element's own location.
    open: Vec<OpenFrame>,
    /// Zero-width `RefElem` tags awaiting the `LinkMarker` that renders the
    /// reference text. A ref tag is a point marker; its rendered supplement
    /// + number is wrapped in a following `LinkMarker` starting at the same
    ///   position (this fork's realization — cites/footnotes differ, their own
    ///   tag brackets the text).
    pending_refs: Vec<PendingRef>,
    /// `LinkMarker` frames currently bracketing a pending ref's text.
    link_for_ref: Vec<LinkFrame>,
    done: Vec<ExtractedMarker>,
    /// Open scopes (by the element's location) whose realized text is
    /// excluded, see [`is_excluded_scope`].
    excluded: Vec<Location>,
    /// Open PDF artifacts (by location). A reference, link, footnote or
    /// citation marker inside one is decorative and records no marker.
    artifacts: Vec<Location>,
    /// Open footnote markers (by location), whatever they sit in. A
    /// footnote's number is a `Label` marker inside its own marker's tags;
    /// a `Label` marker with no footnote open is the number a printed
    /// footnote entry starts with, which is not part of the note's text.
    footnotes: Vec<Location>,
    /// Open terms (by location), realized at the start of their
    /// description's first paragraph.
    term_labels: Vec<Location>,
    /// The marker elements met inside an open term (see
    /// [`Extracted::suppressed`]).
    suppressed: Vec<Location>,
    /// Whether the content walked is as authored ([`label_text`]): its
    /// marks, references, citations and links are bare elements.
    bare: bool,
    /// The references met as authored, in order.
    bare_refs: Vec<Packed<RefElem>>,
    /// The citations met as authored, in order.
    bare_cites: Vec<Packed<CiteElem>>,
    /// Whether any text scope was excluded during this walk.
    excluded_any: bool,
    /// An excluded scope just closed: the next text drops its leading
    /// whitespace if `out` already ends with some (or is still empty), so
    /// that `Before #pdf.artifact[x] after.` reads "Before after.".
    collapse_space: bool,
}

struct MarkFrame {
    loc: Location,
    kind: &'static str,
    start: i64,
    raw: Option<RawSource>,
}

struct OpenFrame {
    marker_loc: Location,
    kind: MarkerKind,
    start: i64,
}

struct PendingRef {
    label: Label,
    start: i64,
}

struct LinkFrame {
    marker_loc: Location,
    label: Label,
    start: i64,
}

fn walk(content: &Content, out: &mut EcoString, ctx: &mut MarkerCtx<'_>) {
    // A `TagElem`'s `tag` is `#[internal]`, so `fields()` never enters it —
    // match the tag element itself and unpack it here.
    if let Some(tag) = content.to_packed::<TagElem>() {
        match &tag.tag {
            Tag::Start(inner, _) => {
                if (is_excluded_scope(inner) || is_entry_number(inner, ctx))
                    && let Some(loc) = inner.location()
                {
                    ctx.excluded.push(loc);
                    ctx.excluded_any = true;
                }
                if inner.elem() == ArtifactElem::ELEM
                    && let Some(loc) = inner.location()
                {
                    ctx.artifacts.push(loc);
                }
                if inner.elem() == FootnoteElem::ELEM
                    && let Some(loc) = inner.location()
                {
                    ctx.footnotes.push(loc);
                }
                if inner
                    .to_packed::<PdfMarkerTag>()
                    .is_some_and(|marker| marker.kind == PdfMarkerTagKind::TermsItemLabel)
                    && let Some(loc) = inner.location()
                {
                    ctx.term_labels.push(loc);
                }
                open_tag(inner, ctx);
                open_mark(inner, out, ctx);
            }
            Tag::End(..) => {
                let loc = tag.tag.location();
                if let Some(index) = ctx.math.iter().rposition(|open| *open == loc) {
                    ctx.math.remove(index);
                }
                if let Some(index) = ctx.open_marks.iter().rposition(|f| f.loc == loc) {
                    let frame = ctx.open_marks.remove(index);
                    ctx.marks.push(Mark {
                        start: frame.start,
                        end: ctx.pos,
                        kind: frame.kind.into(),
                        raw: frame.raw,
                    });
                }
                if let Some(index) = ctx.artifacts.iter().rposition(|open| *open == loc) {
                    ctx.artifacts.remove(index);
                }
                if let Some(index) = ctx.footnotes.iter().rposition(|open| *open == loc) {
                    ctx.footnotes.remove(index);
                }
                if let Some(index) = ctx.term_labels.iter().rposition(|open| *open == loc)
                {
                    ctx.term_labels.remove(index);
                }
                if let Some(index) = ctx.excluded.iter().rposition(|open| *open == loc) {
                    ctx.excluded.remove(index);
                    if ctx.excluded.is_empty() {
                        ctx.collapse_space = true;
                    }
                }
                close_tag(ctx, loc);
            }
        }
        return;
    }

    // An artifact met unrealized (a list item's or a table cell's body) has
    // no tags to scope it: drop it whole, it is decoration.
    if content.elem() == ArtifactElem::ELEM {
        ctx.excluded_any = true;
        ctx.collapse_space = true;
        return;
    }

    // An image in a `box` is met unrealized, in the box's body. It is a node
    // of its own: a paragraph holding only such images (two side by side)
    // has nothing left.
    if content.elem() == ImageElem::ELEM {
        ctx.excluded_any = true;
        return;
    }

    // Inside an excluded scope, or an inline equation whose source stands in
    // for it: emit no text, but keep walking so that the tags nested in it
    // still balance.
    if !ctx.excluded.is_empty() || !ctx.math.is_empty() {
        if content.with::<dyn PlainText>().is_some()
            || content.to_packed::<LinebreakElem>().is_some()
        {
            return;
        }
        for (_, value) in content.fields() {
            walk_value(value, out, ctx);
        }
        return;
    }

    if ctx.bare && walk_bare(content, out, ctx) {
        return;
    }

    // A bare (unrealized) footnote or citation — as found inside content
    // that is text-extracted without realization, e.g. list-item or
    // table-cell bodies — must not leak its note body / supplement into the
    // node's text. In realized flow these appear as tags (handled above),
    // not bare elements.
    if content.to_packed::<FootnoteElem>().is_some()
        || content.to_packed::<CiteElem>().is_some()
    {
        return;
    }

    // A hard line break (`\\`) renders as a new line but carries no text, so
    // `PlainText` yields nothing for it and the words on either side would be
    // run together ("Plateforme de calcul \\ de coût" -> "calculde coût").
    // A node's `text` is one flat string, so the break becomes a space.
    if content.to_packed::<LinebreakElem>().is_some() {
        if !out.is_empty() && !out.ends_with(char::is_whitespace) {
            out.push(' ');
            ctx.pos += 1;
        }
        return;
    }

    if let Some(textable) = content.with::<dyn PlainText>() {
        push_plain(textable, out, ctx);
        return;
    }

    for (_, value) in content.fields() {
        walk_value(value, out, ctx);
    }
}

/// Open a mark frame for an element that carries one ([`mark_kind`]).
///
/// No frame opens inside an excluded scope, nor while a reference,
/// citation or footnote marker is open: the superscript Typst puts on a
/// footnote number is presentation, not the author's (a link's body is the
/// author's, and keeps its marks). An inline equation also writes its
/// source here, and suppresses its realized text until it closes.
fn open_mark(inner: &Content, out: &mut EcoString, ctx: &mut MarkerCtx<'_>) {
    let Some(kind) = mark_kind(inner) else { return };
    let Some(loc) = inner.location() else { return };
    if !ctx.excluded.is_empty() || !ctx.math.is_empty() {
        return;
    }
    let in_marker = !ctx.link_for_ref.is_empty()
        || ctx.open.iter().any(|frame| {
            matches!(frame.kind, MarkerKind::Cite(_) | MarkerKind::Footnote(_))
        });
    let start = ctx.pos;
    let mut raw = None;
    if let Some(equation) = inner.to_packed::<EquationElem>() {
        raw = push_equation(inner, &equation.body, out, ctx);
        ctx.math.push(loc);
    }
    if !in_marker {
        ctx.open_marks.push(MarkFrame { loc, kind, start, raw });
    }
}

/// Append an inline equation's text ([`equation_text`]) and return the
/// `raw` of its mark.
fn push_equation(
    equation: &Content,
    body: &Content,
    out: &mut EcoString,
    ctx: &mut MarkerCtx<'_>,
) -> Option<RawSource> {
    let (text, raw) = equation_text(equation, body, ctx.source);
    out.push_str(&text);
    ctx.pos += text.chars().count() as i64;
    if !text.is_empty() {
        ctx.collapse_space = false;
    }
    raw
}

/// Append an element's plain text, dropping its leading whitespace after an
/// excluded scope (see [`MarkerCtx::collapse_space`]).
fn push_plain(textable: &dyn PlainText, out: &mut EcoString, ctx: &mut MarkerCtx<'_>) {
    let before = out.len();
    textable.plain_text(out);
    if ctx.collapse_space && out.len() > before {
        if out[..before].chars().next_back().is_none_or(char::is_whitespace) {
            let added = out[before..].trim_start().to_string();
            out.truncate(before);
            out.push_str(&added);
        }
        if out.len() > before {
            ctx.collapse_space = false;
        }
    }
    ctx.pos += out[before..].chars().count() as i64;
}

/// Walk an element met as authored, in a label text ([`label_text`]):
/// whether it was handled here.
///
/// - A reference or a citation renders text the authored content does not
///   hold: it adds nothing to the value, and is kept for an edge with no
///   span.
/// - A link is an edge whose span covers its body.
/// - An element of the mark table ([`mark_kind`]) is a mark over its body;
///   an inline equation's text is its source, as in realized text.
fn walk_bare(content: &Content, out: &mut EcoString, ctx: &mut MarkerCtx<'_>) -> bool {
    if let Some(reference) = content.to_packed::<RefElem>() {
        ctx.bare_refs.push(reference.clone());
        return true;
    }
    if let Some(cite) = content.to_packed::<CiteElem>() {
        ctx.bare_cites.push(cite.clone());
        return true;
    }
    if let Some(link) = content.to_packed::<LinkElem>() {
        let start = ctx.pos;
        walk(&link.body, out, ctx);
        if let Some(dest) = link_dest(&link.dest) {
            ctx.done.push(ExtractedMarker {
                kind: MarkerKind::Link(dest),
                span: Some((start, ctx.pos)),
            });
        }
        return true;
    }
    let Some(kind) = mark_kind(content) else { return false };
    let start = ctx.pos;
    let mut raw = None;
    if let Some(equation) = content.to_packed::<EquationElem>() {
        raw = push_equation(content, &equation.body, out, ctx);
    } else if let Some(textable) = content.with::<dyn PlainText>() {
        push_plain(textable, out, ctx);
    } else {
        for (_, value) in content.fields() {
            walk_value(value, out, ctx);
        }
    }
    ctx.marks.push(Mark { start, end: ctx.pos, kind: kind.into(), raw });
    true
}

/// The rich text of a label slot — a heading, a caption, a term, a quote's
/// attribution, a bibliography entry — walked from the content the author
/// wrote, which realization has not touched.
///
/// Its marks come from the elements of the mark table ([`mark_kind`]),
/// inline math as its source. Its edges come back in a `TextRecord` (slot
/// `Text`, for the caller to set):
///
/// - a link gives a `links` or `refs` edge whose span covers its body;
/// - a reference gives a `refs` edge, and a citation a `cites` edge, both
///   with no span: their rendered text is not in the authored content. A
///   `@key` is a reference as authored; resolution sends it to `refs` or
///   `cites` by whether the key is a bibliography entry;
/// - a footnote gives no edge: it has no location to resolve.
///
/// The value is trimmed, its marks and spans re-based on it. Presentation a
/// show rule adds at realization (the `strong` of the terms rule) is not
/// authored, and never becomes a mark.
pub fn label_text(
    content: &Content,
    source: SourceLookup<'_>,
) -> (crate::model::RichText, crate::emit::convert::TextRecord) {
    use crate::emit::convert::{CiteMarker, TextRecord};
    use crate::model::TextSlot;

    let mut out = EcoString::new();
    let mut ctx = MarkerCtx {
        source: Some(source),
        bare: true,
        ..MarkerCtx::default()
    };
    walk(content, &mut out, &mut ctx);

    let MarkerCtx { mut done, marks, bare_refs, bare_cites, .. } = ctx;
    let (value, marks) = {
        let mut spans: Vec<&mut Option<(i64, i64)>> =
            done.iter_mut().map(|marker| &mut marker.span).collect();
        let (value, marks) = trim_rebase(&out, marks, &mut spans);
        let len = value.chars().count() as i64;
        (value, canonical_marks(len, marks, &mut spans))
    };

    let mut record = TextRecord::new(TextSlot::Text);
    for marker in done {
        if let MarkerKind::Link(dest) = marker.kind {
            record.link_markers.push((dest, marker.span));
        }
    }
    let materialized = StyleChain::default();
    for reference in &bare_refs {
        if !record.ref_targets.contains(&reference.target) {
            record.ref_targets.push(reference.target);
        }
        let supplement = match reference.supplement.get_ref(materialized) {
            Smart::Custom(Some(Supplement::Content(content))) => {
                Some(extract_text(content).into())
            }
            _ => None,
        };
        record.cite_markers.push(CiteMarker {
            key: reference.target,
            loc: None,
            form: citation_form_str(Some(CitationForm::Normal)),
            supplement,
            span: None,
        });
    }
    for cite in &bare_cites {
        record.cite_markers.push(CiteMarker {
            key: cite.key,
            loc: None,
            form: citation_form_str(cite.form.get(materialized)),
            supplement: cite
                .supplement
                .get_cloned(materialized)
                .map(|content| extract_text(&content).into()),
            span: None,
        });
    }

    let mut text = crate::model::RichText::plain(value);
    text.marks = marks;
    (text, record)
}

fn open_tag(inner: &Content, ctx: &mut MarkerCtx<'_>) {
    let Some(marker_loc) = inner.location() else { return };
    // Decorative content yields no edge (cnd-sdk spec §2): no marker for a
    // reference, link, footnote or citation written inside an artifact.
    if !ctx.artifacts.is_empty() {
        return;
    }
    // A marker written in a term is an edge of the term, not of the
    // description paragraph the term is realized in.
    if !ctx.term_labels.is_empty() {
        if inner.elem() == CiteElem::ELEM || inner.elem() == FootnoteElem::ELEM {
            ctx.suppressed.push(marker_loc);
        }
        return;
    }
    let pos = ctx.pos;

    if let Some(reference) = inner.to_packed::<RefElem>() {
        // Point marker — defer until its LinkMarker renders the text.
        ctx.pending_refs
            .push(PendingRef { label: reference.target, start: pos });
        return;
    }
    if inner.to_packed::<LinkMarker>().is_some() {
        // Pair with the pending ref that starts here, if any.
        if let Some(index) = ctx.pending_refs.iter().position(|r| r.start == pos) {
            let pending = ctx.pending_refs.remove(index);
            ctx.link_for_ref.push(LinkFrame {
                marker_loc,
                label: pending.label,
                start: pending.start,
            });
        }
        return;
    }
    if let Some(frame) = open_frame(inner, pos) {
        ctx.open.push(frame);
    }
}

fn close_tag(ctx: &mut MarkerCtx<'_>, loc: Location) {
    // A cite/footnote tag closes its own bracketed span.
    if let Some(index) = ctx.open.iter().position(|frame| frame.marker_loc == loc) {
        let frame = ctx.open.remove(index);
        ctx.done.push(ExtractedMarker {
            kind: frame.kind,
            span: Some((frame.start, ctx.pos)),
        });
        return;
    }
    // A LinkMarker closing finishes the pending ref it wraps.
    if let Some(index) = ctx.link_for_ref.iter().position(|frame| frame.marker_loc == loc)
    {
        let frame = ctx.link_for_ref.remove(index);
        ctx.done.push(ExtractedMarker {
            kind: MarkerKind::Ref(frame.label),
            span: Some((frame.start, ctx.pos)),
        });
    }
}

/// Visit every element of `content`, depth-first, except what sits inside
/// a PDF artifact: decorative content (cnd-sdk spec §2) contributes no
/// reference or link. For content met unrealized — a list item's or a table
/// cell's body, an absorbed paragraph, the document's own content — where an
/// artifact still nests its body. (Realized content brackets an artifact
/// with tags instead; [`extract_rich`] handles that.)
pub fn traverse_kept(content: &Content, f: &mut impl FnMut(&Content)) {
    if content.elem() == ArtifactElem::ELEM {
        return;
    }
    f(content);
    for (_, value) in content.fields() {
        traverse_kept_value(value, f);
    }
}

fn traverse_kept_value(value: Value, f: &mut impl FnMut(&Content)) {
    match value {
        Value::Content(c) => traverse_kept(&c, f),
        Value::Array(arr) => {
            for v in arr {
                traverse_kept_value(v, f);
            }
        }
        _ => {}
    }
}

fn walk_value(value: Value, out: &mut EcoString, ctx: &mut MarkerCtx<'_>) {
    match value {
        Value::Content(c) => walk(&c, out, ctx),
        Value::Array(arr) => {
            for v in arr {
                walk_value(v, out, ctx);
            }
        }
        _ => {}
    }
}

/// Open a cite/footnote/link marker frame for a `Tag::Start`'s inner
/// element — these tags bracket their own rendered marker text. Refs and
/// (a ref's own rendering) `LinkMarker`s are handled separately in
/// [`open_tag`], since a ref is a zero-width point marker.
fn open_frame(inner: &Content, start: i64) -> Option<OpenFrame> {
    let marker_loc = inner.location()?;
    if inner.to_packed::<CiteElem>().is_some() {
        // Cite-group limitation (v1): Typst realizes a `@a @b` group under
        // the first cite's tag, so the first cite's span covers the whole
        // rendered group ("[1], [2]") and later members get a zero-width
        // span at the group's end. Typst does not expose per-member
        // sub-spans; emit what the tags give us.
        return Some(OpenFrame {
            marker_loc,
            kind: MarkerKind::Cite(marker_loc),
            start,
        });
    }
    if inner.to_packed::<FootnoteElem>().is_some() {
        return Some(OpenFrame {
            marker_loc,
            kind: MarkerKind::Footnote(marker_loc),
            start,
        });
    }
    if let Some(link) = inner.to_packed::<LinkElem>() {
        // `LinkElem` is `Locatable`, so its own `Tag::Start`/`Tag::End`
        // already bracket the realized `LinkMarker` (and, in turn, the
        // link's body) at this same location — unlike a ref, which is a
        // zero-width point marker that must wait for a *different*
        // element's `LinkMarker` to learn its rendered span. A destination
        // that carries no label (a page/point position) yields no frame at
        // all here, so it never gets a marker or a span.
        let dest = link_dest(&link.dest)?;
        return Some(OpenFrame { marker_loc, kind: MarkerKind::Link(dest), start });
    }
    None
}

/// Map a Typst citation form to its lowercase schema string. A suppressed
/// citation (`form: none`) maps to `"none"` (proposal 0004).
pub fn citation_form_str(form: Option<CitationForm>) -> Option<String> {
    Some(
        match form {
            None => "none",
            Some(CitationForm::Normal) => "normal",
            Some(CitationForm::Prose) => "prose",
            Some(CitationForm::Full) => "full",
            Some(CitationForm::Author) => "author",
            Some(CitationForm::Year) => "year",
        }
        .to_string(),
    )
}
