//! Out-of-tree pool collection and typed-edge resolution (proposal 0004).
//!
//! Footnotes and bibliography entries are referenced *from* the reading
//! flow but sit outside it, so they are top-level pool entries reached
//! through the `cites`/`footnotes` link families rather than nodes. This
//! module builds the footnote pool and resolves each node's footnote
//! markers (captured as introspection tag locations during conversion) to
//! `FootnoteRef` edges pointing at the pool.
//!
//! Text spans (ADR 0013): `FootnoteRef`/`CiteRef` carry the marker's
//! `[start, end)` codepoint offsets in the node's rendered text, computed
//! during extraction (see `extract::extract_with_markers`). Spans are
//! populated for flat-text nodes (paragraph/heading/quote); on other node
//! types, and for a suppressed `form: "none"` citation, `text_span` is
//! `None` (the schema allows null spans).

use typst_library::engine::Engine;
use typst_library::foundations::{NativeElement, StyleChain};
use typst_library::introspection::{Counter, Introspector};
use typst_library::model::{BibliographyElem, FootnoteElem, RenderedEntry, Works};
use uuid::Uuid;

use crate::emit::convert::ConvertContext;
use crate::emit::extract::extract_text;
use crate::emit::refs::find_node_mut;
use crate::manifest::{BibEntry, CiteRef, Footnote, FootnoteRef};

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

/// Build the bibliography pool into `ctx.bibliography` and the `@key` →
/// pool-id map into `ctx.bib_key_to_id` (proposal 0004).
///
/// Only cited entries appear (matching the displayed bibliography, unless
/// the author set `full: true`). Each entry carries the rendered reference
/// string, a curated typed subset, and the full hayagriva source entry as
/// structured JSON (`raw`). Must run before cross-reference resolution so
/// citation keys can be kept out of the `refs` index.
pub fn build_bibliography_pool(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    ctx: &mut ConvertContext,
) -> typst_library::diag::SourceResult<()> {
    let bib_elems: Vec<_> = introspector
        .query(&BibliographyElem::ELEM.select())
        .into_iter()
        .filter_map(|elem| elem.to_packed::<BibliographyElem>().cloned())
        .collect();

    for bib_elem in &bib_elems {
        let Some(loc) = bib_elem.location() else {
            continue;
        };
        let span = bib_elem.span();
        let works = Works::generate(engine, span)?;
        // On an intermediate (not-yet-converged) introspection pass the
        // bibliography may not be stably locatable; skip and let a later
        // pass fill the pool. A genuinely broken bibliography would already
        // have errored during layout, before this runs.
        let Ok(rendered) = works.bibliography(loc, span) else {
            continue;
        };
        let database = &bib_elem.sources.derived;

        for entry in &rendered.entries {
            // Recover the source key and hayagriva entry (rendered order may
            // differ from source order — match by the key we retained).
            let key_str = entry.key.as_str();
            let Some((label, source)) = database
                .iter()
                .find(|(label, _)| label.resolve().as_str() == key_str)
            else {
                continue;
            };
            // First bibliography wins if a key appears in several.
            if ctx.bib_key_to_id.contains_key(&label) {
                continue;
            }

            let id = Uuid::new_v4();
            let raw = serde_json::to_value(source).unwrap_or_default();
            let bib_entry = BibEntry {
                id,
                label: label.resolve().to_string(),
                rendered: rendered_string(entry),
                type_: raw.get("type").and_then(|v| v.as_str()).map(str::to_string),
                authors: source
                    .authors()
                    .map(|persons| {
                        persons.iter().map(|p| p.name_first(false, true)).collect()
                    })
                    .unwrap_or_default(),
                title: source.title().map(|t| t.value.to_str().to_string()),
                year: source.date_any().map(|d| d.year),
                container: source
                    .parents()
                    .first()
                    .and_then(|parent| parent.title())
                    .map(|t| t.value.to_str().to_string()),
                doi: source.doi().map(str::to_string),
                url: source.url_any().map(|u| u.value.to_string()),
                raw,
            };
            ctx.bib_key_to_id.insert(label, id);
            ctx.bibliography.push(bib_entry);
        }
    }
    Ok(())
}

/// Rendered reference string for a bibliography entry: the optional prefix
/// (e.g. the `[1]` label) joined with the entry body.
fn rendered_string(entry: &RenderedEntry) -> String {
    let body = extract_text(&entry.body);
    match &entry.prefix {
        Some(prefix) => {
            let prefix = extract_text(prefix);
            if prefix.is_empty() {
                body.to_string()
            } else {
                format!("{prefix} {body}")
            }
        }
        None => body.to_string(),
    }
}

/// Resolve every node's citation markers to `CiteRef` edges pointing at the
/// bibliography pool. A key with no pool entry is dropped silently. The
/// marker's text span is carried through, except for a suppressed
/// (`form: "none"`) citation, which renders no text and so has no span
/// (ADR 0013 / proposal 0004).
pub fn resolve_cite_edges(ctx: &mut ConvertContext) {
    let mut edges: Vec<(Uuid, CiteRef)> = Vec::new();
    for (node_id, record) in &ctx.records {
        for marker in &record.cite_markers {
            if let Some(bib_id) = ctx.bib_key_to_id.get(&marker.key).copied() {
                let suppressed = marker.form.as_deref() == Some("none");
                let text_span = if suppressed {
                    None
                } else {
                    marker.span.map(|(s, e)| vec![s, e])
                };
                edges.push((
                    *node_id,
                    CiteRef {
                        id: bib_id,
                        label: Some(marker.key.resolve().to_string()),
                        text_span,
                        form: marker.form.clone(),
                        supplement: marker.supplement.clone(),
                    },
                ));
            }
        }
    }

    for (node_id, cite) in edges {
        if let Some(node) = find_node_mut(&mut ctx.roots, node_id) {
            let cites = node.cites_mut();
            // Keep meaningful variants (same work, different supplement/form/
            // position) but drop exact duplicates.
            if !cites.contains(&cite) {
                cites.push(cite);
            }
        }
    }
}

/// Attach a `FootnoteRef` to each node for every footnote marker captured
/// in its record, carrying the marker's text span. A miss (location not a
/// footnote) is dropped. Dedup is by the full edge (id + span), so two
/// markers of the same note in one node become two positioned entries.
fn resolve_footnote_edges(ctx: &mut ConvertContext) {
    let labels: rustc_hash::FxHashMap<Uuid, String> = ctx
        .footnotes
        .iter()
        .map(|f| (f.id, f.label.clone()))
        .collect();

    let mut edges: Vec<(Uuid, Uuid, Option<(i64, i64)>)> = Vec::new();
    for (node_id, record) in &ctx.records {
        for (marker_loc, span) in &record.footnote_locs {
            if let Some(pool_id) = ctx.footnote_loc_to_id.get(marker_loc).copied() {
                edges.push((*node_id, pool_id, *span));
            }
        }
    }

    for (node_id, pool_id, span) in edges {
        let label = labels.get(&pool_id).cloned();
        let reference = FootnoteRef {
            id: pool_id,
            label,
            text_span: span.map(|(s, e)| vec![s, e]),
        };
        if let Some(node) = find_node_mut(&mut ctx.roots, node_id) {
            let footnotes = node.footnotes_mut();
            if !footnotes.contains(&reference) {
                footnotes.push(reference);
            }
        }
    }
}
