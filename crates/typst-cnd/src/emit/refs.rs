use typst_library::foundations::Label;
use typst_library::introspection::Introspector;
use uuid::Uuid;

use crate::emit::convert::ConvertContext;
use crate::emit::extract::LinkDest;
use crate::model::{LinkRef, NodeRef, TextSlot};

/// A `refs` edge before it reaches its source text.
struct RefEdge {
    source: Uuid,
    slot: TextSlot,
    target: Uuid,
    /// The label the marker names; none for a location destination.
    marker_label: Option<String>,
    /// The marker's text span.
    span: Option<(i64, i64)>,
    /// Whether the edge comes from a marker the text walk met, one per
    /// occurrence, rather than from a target a traverse of the content
    /// found, which may be such a marker met again (see [`set_refs`]).
    occurrence: bool,
}
/// A `links` edge before it reaches the node: (node id, text, href, span).
/// An alias, since clippy's `type_complexity` is denied by `-Dwarnings`.
type LinkEdge = (Uuid, TextSlot, String, Option<(i64, i64)>);

/// Resolve cross-references and populate the forward-only `refs` list on
/// each source text (ADR 0008). The reverse index is derived on demand by
/// the SDK (`Cnd.incoming`), never materialized here.
///
/// An edge sits only on the text its marker was written in: a marker
/// realized in the text gives its span (ADR 0013), and a label text (a
/// caption, a term) or an attribution, walked as authored, gives a
/// span-less edge. Nothing is guessed from the section a reference sits
/// in, and an edge lands only on the node that carries the label it names.
pub fn resolve_refs(ctx: &mut ConvertContext, introspector: &dyn Introspector) {
    let mut edges: Vec<RefEdge> = Vec::new();
    for (source_id, record) in ctx.records_sorted() {
        for text in &record.texts {
            // `ref_markers` first: a `ref_targets` edge is kept only for a
            // target no marker of the text names (see `set_refs`).
            let markers =
                text.ref_markers.iter().map(|(label, span)| (*label, *span, true));
            let targets = text.ref_targets.iter().map(|label| (*label, None, false));
            for (label, span, occurrence) in markers.chain(targets) {
                // A `@key` citation is a RefElem too, but resolves in the
                // bibliography pool, not the node tree: keep it out of
                // `refs`.
                if ctx.bib_key_to_id.contains_key(&label) {
                    continue;
                }
                if let Some(target) = resolve_label(label, ctx, introspector) {
                    edges.push(RefEdge {
                        source: source_id,
                        slot: text.slot,
                        target,
                        marker_label: Some(label.resolve().as_str().into()),
                        span,
                        occurrence,
                    });
                }
            }
        }
    }
    set_refs(ctx, edges);
}

/// Put each edge on its source text, named by its target's label (ADR
/// 0017). An edge from a marker lands only when the target carries exactly
/// the label the marker names. One from a location destination names the
/// target by the label it carries, and is dropped when it carries none:
/// there is nothing to name it by.
///
/// A text holds one edge per marker written in it, each with its own span,
/// so a target named twice is two edges. A marker counted twice gives the
/// same edge twice, kept once. A target found by a traverse of the content
/// rather than by the text walk carries no span and may be a marker the
/// walk already met: it is kept only when no edge of the text names that
/// target yet.
fn set_refs(ctx: &mut ConvertContext, edges: Vec<RefEdge>) {
    let labels = ctx.node_labels();
    let refs: Vec<(Uuid, TextSlot, (NodeRef, bool))> = edges
        .into_iter()
        .filter_map(|edge| {
            let label = labels.get(&edge.target)?.clone();
            if edge.marker_label.is_some_and(|marker| marker != label) {
                return None;
            }
            let text_span = edge.span.map(|(s, e)| vec![s, e]);
            Some((
                edge.source,
                edge.slot,
                (NodeRef { label, text_span }, edge.occurrence),
            ))
        })
        .collect();
    ctx.edit_texts(refs, |text, (reference, occurrence)| {
        let known = if occurrence {
            text.refs.contains(&reference)
        } else {
            text.refs.iter().any(|known| known.label == reference.label)
        };
        if !known {
            text.refs.push(reference);
        }
    });
}

/// Resolve `LinkElem` markers (ADR 0024, cnd-sdk) into `links` and `refs`
/// edges: a URL destination becomes a `links` edge — `href` has no
/// resolution domain, so it is pushed directly, never dropped. A
/// label/location destination *is* a cross-reference (with a custom body)
/// and goes through the same `set_refs` as `resolve_refs`: a text naming
/// one target both ways (`@fig-1` and `#link(<fig-1>)[...]`) holds one
/// `refs` edge per marker.
///
/// Call after `resolve_refs`: label/location resolution needs
/// `ctx.label_to_id`/`ctx.location_to_id`, already built by
/// `rebuild_label_index` and unaffected by `resolve_refs` itself.
pub fn resolve_links(ctx: &mut ConvertContext, introspector: &dyn Introspector) {
    // (source, href, span) for a `links` edge.
    let mut links: Vec<LinkEdge> = Vec::new();
    // Resolved through the same `set_refs` as in `resolve_refs`.
    let mut ref_edges: Vec<RefEdge> = Vec::new();

    for (source_id, record) in ctx.records_sorted() {
        for text in &record.texts {
            let source = (source_id, text.slot);
            for (dest, span) in &text.link_markers {
                collect_link_edge(
                    ctx,
                    introspector,
                    source,
                    dest,
                    (*span, true),
                    &mut links,
                    &mut ref_edges,
                );
            }
            for dest in &text.link_targets {
                collect_link_edge(
                    ctx,
                    introspector,
                    source,
                    dest,
                    (None, false),
                    &mut links,
                    &mut ref_edges,
                );
            }
        }
    }

    let links = links
        .into_iter()
        .map(|(source_id, slot, href, span)| {
            (source_id, slot, LinkRef { href, text_span: normalize_link_span(span) })
        })
        .collect();
    ctx.edit_texts(links, |text, link| text.links.push(link));

    set_refs(ctx, ref_edges);
}

/// Collect the edge of one link destination: `span` is the marker's, and
/// `occurrence` whether the text walk met it (see [`RefEdge::occurrence`]).
fn collect_link_edge(
    ctx: &ConvertContext,
    introspector: &dyn Introspector,
    (source, slot): (Uuid, TextSlot),
    dest: &LinkDest,
    (span, occurrence): (Option<(i64, i64)>, bool),
    links: &mut Vec<LinkEdge>,
    ref_edges: &mut Vec<RefEdge>,
) {
    match dest {
        LinkDest::Url(href) => links.push((source, slot, href.to_string(), span)),
        LinkDest::Label(label) => {
            // Mirrors `resolve_refs`'s bib-key guard: a `@key` citation is a
            // `RefElem` that resolves in the bibliography pool, not the node
            // tree, and a `#link(<key>)[..]` naming the same label should be
            // kept out of `refs` for the same reason. `resolve_label`
            // presumably already returns `None` for a bib key (nothing in
            // `ctx.records`/the introspector carries it as a node label),
            // but the guard is cheap and keeps this path an exact mirror of
            // `resolve_refs` rather than an inconsistent near-copy.
            if ctx.bib_key_to_id.contains_key(label) {
                return;
            }
            if let Some(target) = resolve_label(*label, ctx, introspector) {
                ref_edges.push(RefEdge {
                    source,
                    slot,
                    target,
                    marker_label: Some(label.resolve().as_str().into()),
                    span,
                    occurrence,
                });
            }
        }
        LinkDest::Location(loc) => {
            if let Some(target) = ctx.location_to_id.get(loc).copied() {
                ref_edges.push(RefEdge {
                    source,
                    slot,
                    target,
                    marker_label: None,
                    span,
                    occurrence,
                });
            }
        }
    }
}

/// A marker that opened and closed at the same code-point position rendered
/// no text (e.g. an image link's body) — null the span rather than emit a
/// degenerate `[n, n]` (design's null-span rule).
fn normalize_link_span(span: Option<(i64, i64)>) -> Option<Vec<i64>> {
    match span {
        Some((start, end)) if start != end => Some(vec![start, end]),
        _ => None,
    }
}

/// The node that carries `label`: the one built from the element the label
/// is attached to, and no other. A label whose element yields no node — on
/// decorative content, in a drawing's body, on a node `assemble` dropped —
/// names nothing in the CND, and a reference to it has no edge.
fn resolve_label(
    label: Label,
    ctx: &ConvertContext,
    introspector: &dyn Introspector,
) -> Option<Uuid> {
    if let Some(id) = ctx.label_to_id.get(&label).copied() {
        return Some(id);
    }
    let location = introspector.query_label(label).ok()?.location()?;
    // A label on decorative content names nothing (cnd-sdk spec §2).
    if ctx.artifact_locations.contains(&location) {
        return None;
    }
    ctx.location_to_id.get(&location).copied()
}

/// Build label lookup from records and labelled introspector elements.
pub fn rebuild_label_index(ctx: &mut ConvertContext, introspector: &dyn Introspector) {
    let labelled: Vec<(Label, Uuid)> = ctx
        .records_sorted()
        .into_iter()
        .filter_map(|(id, record)| record.label.map(|label| (label, id)))
        .collect();
    ctx.label_to_id.clear();
    ctx.label_to_id.extend(labelled);

    for elem in introspector.query_labelled() {
        let Some(label) = elem.label() else { continue };
        // The authoring package's admonition markers name no node.
        if crate::emit::admonition::is_marker_label(label) {
            continue;
        }
        let Some(id) = resolve_label(label, ctx, introspector) else { continue };
        ctx.label_to_id.insert(label, id);
        if let Some(record) = ctx.records.get_mut(&id) {
            record.label = Some(label);
        }
    }
}
