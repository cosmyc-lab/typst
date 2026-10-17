use typst_library::foundations::Label;
use typst_library::introspection::Introspector;
use uuid::Uuid;

use crate::emit::convert::ConvertContext;
use crate::emit::extract::LinkDest;
use crate::model::{LinkRef, NodeRef, TextSlot};

/// `(source node, source text, target node, the label the marker names —
/// none for a location destination —, the marker's text span)`.
type RefEdge = (Uuid, TextSlot, Uuid, Option<String>, Option<(i64, i64)>);
/// A `links` edge before it reaches the node: (node id, text, href, span).
/// Sibling of `RefEdge` above — same reason, and clippy's `type_complexity`
/// is denied by CI's `-Dwarnings`.
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
            // `ref_markers` first: these carry spans, and `set_ref` dedups
            // first-wins per target, so the spanned edge must be pushed
            // before a span-less `ref_targets` edge for the same target.
            let spanned = text.ref_markers.iter().map(|(label, span)| (*label, *span));
            let spanless = text.ref_targets.iter().map(|label| (*label, None));
            for (label, span) in spanned.chain(spanless) {
                // A `@key` citation is a RefElem too, but resolves in the
                // bibliography pool, not the node tree: keep it out of
                // `refs`.
                if ctx.bib_key_to_id.contains_key(&label) {
                    continue;
                }
                if let Some(target_id) = resolve_label(label, ctx, introspector) {
                    edges.push((
                        source_id,
                        text.slot,
                        target_id,
                        Some(label.resolve().as_str().into()),
                        span,
                    ));
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
fn set_refs(ctx: &mut ConvertContext, edges: Vec<RefEdge>) {
    let labels = ctx.node_labels();
    let refs: Vec<(Uuid, TextSlot, NodeRef)> = edges
        .into_iter()
        .filter_map(|(source, slot, target, marker_label, span)| {
            let label = labels.get(&target)?.clone();
            if marker_label.is_some_and(|marker| marker != label) {
                return None;
            }
            let text_span = span.map(|(s, e)| vec![s, e]);
            Some((source, slot, NodeRef { label, text_span }))
        })
        .collect();
    ctx.edit_texts(refs, |text, reference| {
        // Dedup by target label (one edge per target per node); the first
        // occurrence — the primary, span-carrying path — wins. Labels are
        // globally unique (ADR 0017), so this is the same dedup the target
        // id used to give.
        if !text.refs.iter().any(|known| known.label == reference.label) {
            text.refs.push(reference);
        }
    });
}

/// Resolve `LinkElem` markers (ADR 0024, cnd-sdk) into `links` and `refs`
/// edges: a URL destination becomes a `links` edge — `href` has no
/// resolution domain, so it is pushed directly, never dropped. A
/// label/location destination *is* a cross-reference (with a custom body)
/// and is folded into the same `set_ref` dedup-by-label machinery as
/// `resolve_refs`, so a node naming the same target both ways (`@fig-1`
/// and `#link(<fig-1>)[...]`) still yields one `refs` edge.
///
/// Call after `resolve_refs`: label/location resolution needs
/// `ctx.label_to_id`/`ctx.location_to_id`, already built by
/// `rebuild_label_index` and unaffected by `resolve_refs` itself.
pub fn resolve_links(ctx: &mut ConvertContext, introspector: &dyn Introspector) {
    // (source, href, span) for a `links` edge.
    let mut links: Vec<LinkEdge> = Vec::new();
    // Same shape as `RefEdge` in `resolve_refs`, resolved through the same
    // `set_ref` dedup below.
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
                    *span,
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
                    None,
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

fn collect_link_edge(
    ctx: &ConvertContext,
    introspector: &dyn Introspector,
    (source_id, slot): (Uuid, TextSlot),
    dest: &LinkDest,
    span: Option<(i64, i64)>,
    links: &mut Vec<LinkEdge>,
    ref_edges: &mut Vec<RefEdge>,
) {
    match dest {
        LinkDest::Url(href) => links.push((source_id, slot, href.to_string(), span)),
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
            if let Some(target_id) = resolve_label(*label, ctx, introspector) {
                ref_edges.push((
                    source_id,
                    slot,
                    target_id,
                    Some(label.resolve().as_str().into()),
                    span,
                ));
            }
        }
        LinkDest::Location(loc) => {
            if let Some(target_id) = ctx.location_to_id.get(loc).copied() {
                ref_edges.push((source_id, slot, target_id, None, span));
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
