use std::sync::Arc;

use ecow::eco_vec;
use typst_layout::{PagedIntrospector, layout_document};
use typst_library::diag::{SourceResult, error};
use typst_library::engine::Engine;
use typst_library::foundations::{Content, Datetime, Output, Smart, StyleChain, Target};
use typst_library::introspection::Introspector;
use typst_library::model::{Document, DocumentInfo};
use typst_syntax::Span;

use crate::emit::convert::{self, ConvertContext};
use crate::emit::{pools, refs};
use crate::location::LocationAssigner;
use crate::model::{BibEntry, CND_VERSION, Cnd, DocDate, DocMetadata, Footnote, SourceInfo};

/// A compiled CND document before JSON serialization.
#[derive(Debug, Clone)]
pub struct CndDocument {
    info: DocumentInfo,
    nodes: Vec<crate::model::CndNode>,
    bibliography: Vec<BibEntry>,
    footnotes: Vec<Footnote>,
    introspector: Arc<PagedIntrospector>,
}

impl CndDocument {
    pub fn info(&self) -> &DocumentInfo {
        &self.info
    }

    pub fn nodes(&self) -> &[crate::model::CndNode] {
        &self.nodes
    }

    /// Bibliography pool (proposal 0004). Empty until pool collection lands.
    pub fn bibliography(&self) -> &[BibEntry] {
        &self.bibliography
    }

    /// Footnote pool (proposal 0004). Empty until pool collection lands.
    pub fn footnotes(&self) -> &[Footnote] {
        &self.footnotes
    }

    pub fn introspector(&self) -> &Arc<PagedIntrospector> {
        &self.introspector
    }
}

impl Document for CndDocument {
    fn info(&self) -> &DocumentInfo {
        &self.info
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

        let mut ctx = realize_and_convert(
            engine,
            introspector.as_ref(),
            content,
            styles,
            &info,
        )?;
        refs::rebuild_label_index(&mut ctx, introspector.as_ref());
        convert::apply_metadata(&mut ctx);

        // Bibliography pool must precede cross-reference resolution so a
        // `@key` citation (a RefElem) is kept out of the `refs` index.
        pools::build_bibliography_pool(engine, introspector.as_ref(), &mut ctx)?;
        refs::resolve_refs(&mut ctx, introspector.as_ref(), content);

        // Out-of-tree typed edges (proposal 0004): footnote + citation
        // markers were captured per node during conversion (as introspection
        // tags, since the realized flow keeps only rendered markers); resolve
        // them to FootnoteRef/CiteRef edges against the pools.
        pools::resolve_footnotes(engine, introspector.as_ref(), styles, &mut ctx)?;
        pools::resolve_cite_edges(&mut ctx);

        let footnotes = ctx.footnotes.clone();
        let bibliography = ctx.bibliography.clone();
        let mut nodes = ctx.roots;
        let mut assigner = LocationAssigner::new(introspector.clone(), ctx.records);
        assigner.assign_all(&mut nodes);

        Ok(Self {
            info,
            nodes,
            bibliography,
            footnotes,
            introspector,
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
    info: &DocumentInfo,
) -> SourceResult<ConvertContext> {
    let mut ctx = ConvertContext::default();
    crate::emit::convert::convert_from_introspector(
        engine,
        introspector,
        styles,
        doc_lang_from_info(info),
        &mut ctx,
    )?;
    Ok(ctx)
}

fn doc_lang_from_info(info: &DocumentInfo) -> Option<ecow::EcoString> {
    match info.locale {
        Smart::Custom(locale) => Some(locale.rfc_3066()),
        Smart::Auto => None,
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
        doc: doc_metadata_from_info(document.info()),
        nodes: document.nodes().to_vec(),
        // Pools are populated by a later increment (footnote/bibliography
        // collection); always-present empty vecs satisfy the contract.
        bibliography: document.bibliography().to_vec(),
        footnotes: document.footnotes().to_vec(),
    }
}

fn doc_metadata_from_info(info: &DocumentInfo) -> DocMetadata {
    use ecow::EcoString;
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
        lang: match info.locale {
            Smart::Custom(locale) => Some(locale.rfc_3066().into()),
            Smart::Auto => None,
        },
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
        eco_vec![error!(
            Span::detached(),
            "failed to serialize CND: {err}"
        )]
    })
}
