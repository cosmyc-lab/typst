//! Out-of-tree pool collection and typed-edge resolution (proposal 0004).
//!
//! Footnotes and bibliography entries are referenced *from* the reading
//! flow but sit outside it, so they are top-level pool entries reached
//! through the `cites`/`footnotes` link families rather than nodes. This
//! module builds the footnote pool and resolves each node's footnote
//! markers (captured as introspection tag locations during conversion) to
//! `FootnoteRef` edges pointing at the pool.
//!
//! Conformance note — spans: the `span` field on `FootnoteRef`/`CiteRef`
//! is a deferred conformance level. Edges resolve to real pool entries,
//! but `span` is always `None` today; the schema allows null spans. Adding
//! positioned spans requires threading codepoint offsets through the text
//! extractor and is tracked as follow-up work.

use typst_library::engine::Engine;
use typst_library::foundations::{NativeElement, StyleChain};
use typst_library::introspection::{Counter, Introspector};
use typst_library::model::FootnoteElem;
use uuid::Uuid;

use crate::emit::convert::ConvertContext;
use crate::emit::extract::extract_text;
use crate::emit::refs::find_node_mut;
use crate::manifest::{Footnote, FootnoteRef};

/// Build the footnote pool into `ctx.footnotes` and the marker-location →
/// pool-id map into `ctx.footnote_loc_to_id`, then attach a `FootnoteRef`
/// to every node that carries a footnote marker.
///
/// Only declaration footnotes become pool entries; a `#footnote(<label>)`
/// re-reference reuses the original's pool id, so several markers point at
/// one entry. Pool order is the introspector's document order; each entry's
/// `label` is the rendered footnote ordinal and `text` is the flat body.
pub fn resolve_footnotes(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    styles: StyleChain,
    ctx: &mut ConvertContext,
) -> typst_library::diag::SourceResult<()> {
    let counter = Counter::of(FootnoteElem::ELEM);
    let footnotes: Vec<_> = introspector
        .query(&FootnoteElem::ELEM.select())
        .into_iter()
        .filter_map(|elem| elem.to_packed::<FootnoteElem>().cloned())
        .collect();

    // Pass 1 — declarations become pool entries, keyed by their own
    // (declaration) location.
    for footnote in &footnotes {
        if footnote.is_ref() {
            continue;
        }
        let Some(own_loc) = footnote.location() else {
            continue;
        };
        let id = *ctx.footnote_loc_to_id.entry(own_loc).or_insert_with(Uuid::new_v4);
        if !ctx.footnotes.iter().any(|f| f.id == id) {
            let text = footnote
                .body_content()
                .map(extract_text)
                .unwrap_or_default()
                .into();
            let numbering = footnote.numbering.get_ref(styles);
            let label = counter
                .display_at(engine, own_loc, styles, numbering, footnote.span())
                .ok()
                .map(|num| extract_text(&num).into())
                .unwrap_or_default();
            ctx.footnotes.push(Footnote { id, label, text });
        }
    }

    // Pass 2 — every marker (declaration or re-reference) maps its own
    // location to a pool id. A node records the marker location it holds
    // (a tag), so both marker kinds resolve to the same entry.
    for footnote in &footnotes {
        let Some(own_loc) = footnote.location() else {
            continue;
        };
        let Ok(decl_loc) = footnote.declaration_location(engine) else {
            continue;
        };
        if let Some(id) = ctx.footnote_loc_to_id.get(&decl_loc).copied() {
            ctx.footnote_loc_to_id.insert(own_loc, id);
        }
    }

    resolve_footnote_edges(ctx);
    Ok(())
}

/// Attach a `FootnoteRef` to each node for every footnote marker location
/// captured in its record. A miss (location not a footnote) is dropped.
fn resolve_footnote_edges(ctx: &mut ConvertContext) {
    let labels: rustc_hash::FxHashMap<Uuid, String> = ctx
        .footnotes
        .iter()
        .map(|f| (f.id, f.label.clone()))
        .collect();

    let mut edges: Vec<(Uuid, Uuid)> = Vec::new();
    for (node_id, record) in &ctx.records {
        for marker_loc in &record.footnote_locs {
            if let Some(pool_id) = ctx.footnote_loc_to_id.get(marker_loc).copied() {
                edges.push((*node_id, pool_id));
            }
        }
    }

    for (node_id, pool_id) in edges {
        let label = labels.get(&pool_id).cloned();
        if let Some(node) = find_node_mut(&mut ctx.roots, node_id) {
            let footnotes = node.footnotes_mut();
            if !footnotes.iter().any(|reference| reference.id == pool_id) {
                footnotes.push(FootnoteRef { id: pool_id, label, span: None });
            }
        }
    }
}
