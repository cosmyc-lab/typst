use ecow::EcoString;
use typst_library::foundations::{Content, Label, PlainText, Value};
use typst_library::introspection::{Location, Tag, TagElem};
use typst_library::model::{
    CitationForm, CiteElem, Destination, FootnoteElem, LinkElem, LinkMarker, LinkTarget,
    RefElem,
};
use typst_library::text::LinebreakElem;

/// Extract plain text from content without duplicating inline code.
///
/// Unlike `Content::plain_text()`, this stops at elements that implement
/// `PlainText` and does not recurse into their synthesized children. This
/// prevents `RawElem` text from appearing three times (once from `RawElem`,
/// once from the synthesized `RawLine`, once from the `TextElem` inside
/// `RawLine.body`).
pub fn extract_text(content: &Content) -> EcoString {
    extract_with_markers(content).0
}

/// A reference/citation/footnote marker located within a node's rendered
/// text: its kind (carrying the resolution payload) and the `[start, end)`
/// Unicode code-point offsets of its rendered marker in that text.
#[derive(Debug, Clone)]
pub struct ExtractedMarker {
    pub kind: MarkerKind,
    pub start: i64,
    pub end: i64,
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

/// Extract plain text and, in the same walk, the code-point spans of any
/// reference/citation/footnote markers it contains.
///
/// The realized flow no longer carries the `RefElem`/`CiteElem`/
/// `FootnoteElem` itself, but leaves an introspection `TagElem` pair
/// (`Tag::Start(inner)` … rendered marker text … `Tag::End(loc)`) around
/// the marker's rendered text. Because those tags are interleaved in the
/// exact content tree this walk visits, offsets are computed *as the text
/// is built* — no substring search, correct by construction. Spans are
/// therefore offsets into the returned text, which is the value a flat
/// node uses verbatim for its `text` field.
pub fn extract_with_markers(content: &Content) -> (EcoString, Vec<ExtractedMarker>) {
    let mut out = EcoString::new();
    let mut ctx = MarkerCtx::default();
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
    let MarkerCtx { open, mut done, .. } = ctx;
    for frame in open {
        if matches!(frame.kind, MarkerKind::Link(_)) {
            done.push(ExtractedMarker {
                kind: frame.kind,
                start: frame.start,
                end: frame.start,
            });
        }
    }
    (out, done)
}

#[derive(Default)]
struct MarkerCtx {
    /// Code-point position in `out` so far.
    pos: i64,
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

fn walk(content: &Content, out: &mut EcoString, ctx: &mut MarkerCtx) {
    // A `TagElem`'s `tag` is `#[internal]`, so `fields()` never enters it —
    // match the tag element itself and unpack it here.
    if let Some(tag) = content.to_packed::<TagElem>() {
        match &tag.tag {
            Tag::Start(inner, _) => open_tag(inner, ctx),
            Tag::End(..) => close_tag(ctx, tag.tag.location()),
        }
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
        let before = out.len();
        textable.plain_text(out);
        ctx.pos += out[before..].chars().count() as i64;
        return;
    }

    for (_, value) in content.fields() {
        walk_value(value, out, ctx);
    }
}

fn open_tag(inner: &Content, ctx: &mut MarkerCtx) {
    let Some(marker_loc) = inner.location() else { return };
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

fn close_tag(ctx: &mut MarkerCtx, loc: Location) {
    // A cite/footnote tag closes its own bracketed span.
    if let Some(index) = ctx.open.iter().position(|frame| frame.marker_loc == loc) {
        let frame = ctx.open.remove(index);
        ctx.done.push(ExtractedMarker {
            kind: frame.kind,
            start: frame.start,
            end: ctx.pos,
        });
        return;
    }
    // A LinkMarker closing finishes the pending ref it wraps.
    if let Some(index) = ctx.link_for_ref.iter().position(|frame| frame.marker_loc == loc)
    {
        let frame = ctx.link_for_ref.remove(index);
        ctx.done.push(ExtractedMarker {
            kind: MarkerKind::Ref(frame.label),
            start: frame.start,
            end: ctx.pos,
        });
    }
}

fn walk_value(value: Value, out: &mut EcoString, ctx: &mut MarkerCtx) {
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
