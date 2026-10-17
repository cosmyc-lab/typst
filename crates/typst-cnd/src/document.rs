use std::sync::Arc;

use ecow::{EcoString, eco_vec};
use typst_layout::{PagedDocument, PagedIntrospector, layout_document};
use typst_library::diag::{SourceResult, error};
use typst_library::engine::Engine;
use typst_library::format::DocumentFormatOptions;
use typst_library::foundations::{Content, Datetime, Output, Smart, StyleChain, Target};
use typst_library::introspection::Introspector;
use typst_library::model::{Document, DocumentInfo};
use typst_library::text::Locale;
use typst_syntax::Span;

use crate::emit::ancestry::Ancestry;
use crate::emit::convert::{self, ConvertContext};
use crate::emit::{pools, refs};
use crate::location::LocationAssigner;
use crate::model::{
    BibEntry, CND_VERSION, Cnd, DocDate, DocMetadata, Footnote, SourceInfo,
};

/// A compiled CND document before JSON serialization.
#[derive(Debug, Clone)]
pub struct CndDocument {
    info: DocumentInfo,
    /// The document's language (see [`doc_lang`]).
    lang: EcoString,
    nodes: Vec<crate::model::CndNode>,
    bibliography: Vec<BibEntry>,
    footnotes: Vec<Footnote>,
    introspector: Arc<PagedIntrospector>,
    /// Format options set by the document (upstream #8496). CND exports none
    /// of these formats, but `Document` requires them and they are carried
    /// through unchanged from the paged layout this document is built on.
    options: DocumentFormatOptions,
    /// The paged layout the document was read from, kept so the same pages
    /// can be exported (a PDF, a page image) without a second layout.
    paged: PagedDocument,
}

impl CndDocument {
    pub fn info(&self) -> &DocumentInfo {
        &self.info
    }

    /// The document's language, as an RFC 3066 tag (`doc.lang`).
    pub fn lang(&self) -> &str {
        &self.lang
    }

    pub fn nodes(&self) -> &[crate::model::CndNode] {
        &self.nodes
    }

    /// Bibliography pool (proposal 0004).
    pub fn bibliography(&self) -> &[BibEntry] {
        &self.bibliography
    }

    /// Footnote pool (proposal 0004).
    pub fn footnotes(&self) -> &[Footnote] {
        &self.footnotes
    }

    pub fn introspector(&self) -> &Arc<PagedIntrospector> {
        &self.introspector
    }

    /// The paged layout this document was read from.
    pub fn paged(&self) -> &PagedDocument {
        &self.paged
    }
}

impl Document for CndDocument {
    fn info(&self) -> &DocumentInfo {
        &self.info
    }

    fn options(&self) -> &DocumentFormatOptions {
        &self.options
    }
}

impl Output for CndDocument {
    fn target() -> Target {
        Target::Paged
    }

    fn create(
        engine: &mut Engine,
        content: &Content,
        styles: StyleChain,
    ) -> SourceResult<Self> {
        let paged = layout_document(engine, content, styles)?;
        let introspector = paged.introspector().clone();
        let info = paged.info().clone();
        let lang = doc_lang(&info, styles);
        let options = paged.options().clone();

        // Containment is read off the laid-out frames: `Introspector::query`
        // returns a flat list, but the emit pipeline must know which located
        // elements sit inside a paragraph or inside page furniture.
        let ancestry = Ancestry::from_document(&paged, engine);

        let mut ctx = realize_and_convert(
            engine,
            introspector.as_ref(),
            content,
            styles,
            lang.clone(),
            &ancestry,
        )?;
        ctx.artifact_locations = ancestry.artifact_locations().clone();
        refs::rebuild_label_index(&mut ctx, introspector.as_ref());
        convert::apply_metadata(&mut ctx);

        // Bibliography pool must precede cross-reference resolution so a
        // `@key` citation (a RefElem) is kept out of the `refs` index.
        pools::build_bibliography_pool(engine, introspector.as_ref(), &mut ctx)?;
        refs::resolve_refs(&mut ctx, introspector.as_ref(), content);

        // `links` (ADR 0024, cnd-sdk): a label/location `LinkElem` folds
        // into the same `refs` edges above; needs `ctx.label_to_id`/
        // `ctx.location_to_id`, already built by `rebuild_label_index`.
        refs::resolve_links(&mut ctx, introspector.as_ref());

        // Out-of-tree typed edges (proposal 0004): footnote + citation
        // markers were captured per node during conversion (as introspection
        // tags, since the realized flow keeps only rendered markers); resolve
        // them to FootnoteRef/CiteRef edges against the pools.
        pools::resolve_footnotes(engine, introspector.as_ref(), &ancestry, &mut ctx)?;
        pools::resolve_cite_edges(&mut ctx, &ancestry);

        let mut footnotes = ctx.footnotes;
        let bibliography = ctx.bibliography;
        let mut nodes = ctx.roots;
        let mut assigner = LocationAssigner::new(introspector.clone(), ctx.records);
        assigner.assign_all(&mut nodes, &mut footnotes);

        Ok(Self {
            info,
            lang,
            nodes,
            bibliography,
            footnotes,
            introspector,
            options,
            paged,
        })
    }

    fn introspector(&self) -> &dyn Introspector {
        self.introspector.as_ref()
    }
}

fn realize_and_convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    _content: &Content,
    styles: StyleChain,
    doc_lang: EcoString,
    ancestry: &Ancestry,
) -> SourceResult<ConvertContext> {
    let mut ctx = ConvertContext::default();
    crate::emit::convert::convert_from_introspector(
        engine,
        introspector,
        styles,
        doc_lang,
        ancestry,
        &mut ctx,
    )?;
    Ok(ctx)
}

/// The document's language: the locale the author set for it (Typst reads
/// it from the text language set before any content), or else the text
/// locale at the document's root, which is Typst's default (`en`) when
/// nothing sets one. Every text inherits it (spec §4.1).
fn doc_lang(info: &DocumentInfo, styles: StyleChain) -> EcoString {
    match info.locale {
        Smart::Custom(locale) => locale.rfc_3066(),
        Smart::Auto => Locale::get_in(styles).rfc_3066(),
    }
}

/// Produce a CND document from content (used internally by `Output::create`).
pub fn cnd_document(
    engine: &mut Engine,
    content: &Content,
    styles: StyleChain,
) -> SourceResult<CndDocument> {
    CndDocument::create(engine, content, styles)
}

/// Build a CND from a compiled document.
pub fn cnd_from_document(
    document: &CndDocument,
    source: SourceInfo,
    built_at: String,
) -> Cnd {
    Cnd {
        id: None,
        cnd_version: CND_VERSION.into(),
        built_at,
        source: Some(source),
        doc: doc_metadata_from_info(document.info(), document.lang()),
        nodes: document.nodes().to_vec(),
        bibliography: document.bibliography().to_vec(),
        footnotes: document.footnotes().to_vec(),
    }
}

fn doc_metadata_from_info(info: &DocumentInfo, lang: &str) -> DocMetadata {
    DocMetadata {
        title: info
            .title
            .clone()
            .unwrap_or_else(|| EcoString::from("Untitled"))
            .into(),
        authors: info.author.iter().map(|a| a.clone().into()).collect(),
        date: doc_date_from_info(info),
        keywords: info.keywords.iter().map(|k| k.clone().into()).collect(),
        description: info.description.clone().map(Into::into),
        lang: Some(lang.into()),
        status: None,
        version: None,
        extra: std::collections::BTreeMap::default(),
    }
}

fn doc_date_from_info(info: &DocumentInfo) -> Option<DocDate> {
    match info.date {
        Smart::Custom(Some(dt)) => Some(datetime_to_doc_date(dt)),
        Smart::Auto | Smart::Custom(None) => None,
    }
}

fn datetime_to_doc_date(dt: Datetime) -> DocDate {
    DocDate {
        year: dt.year().unwrap_or(1970),
        month: dt.month().map(i32::from),
        day: dt.day().map(i32::from),
    }
}

/// Serialize a CND to pretty JSON.
pub fn cnd_to_json(cnd: &Cnd) -> SourceResult<String> {
    serde_json::to_string_pretty(cnd).map_err(|err| {
        eco_vec![error!(Span::detached(), "failed to serialize CND: {err}")]
    })
}
