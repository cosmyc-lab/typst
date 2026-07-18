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
use crate::emit::refs;
use crate::location::LocationAssigner;
use crate::manifest::{BibEntry, CND_VERSION, CndManifest, DocDate, DocMetadata, Footnote};

/// A compiled CND document before JSON serialization.
#[derive(Debug, Clone)]
pub struct CndDocument {
    info: DocumentInfo,
    nodes: Vec<crate::manifest::CndNode>,
    bibliography: Vec<BibEntry>,
    footnotes: Vec<Footnote>,
    introspector: Arc<PagedIntrospector>,
}

impl CndDocument {
    pub fn info(&self) -> &DocumentInfo {
        &self.info
    }

    pub fn nodes(&self) -> &[crate::manifest::CndNode] {
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
        refs::resolve_refs(&mut ctx, introspector.as_ref(), content);

        let mut nodes = ctx.roots;
        let mut assigner = LocationAssigner::new(introspector.clone(), ctx.records);
        assigner.assign_all(&mut nodes);

        Ok(Self {
            info,
            nodes,
            bibliography: Vec::new(),
            footnotes: Vec::new(),
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

/// Build a manifest JSON model from a compiled document.
pub fn manifest_from_document(
    document: &CndDocument,
    doc_hash: String,
    compiled_at: String,
) -> CndManifest {
    CndManifest {
        id: None,
        cnd_version: CND_VERSION.into(),
        doc_hash,
        compiled_at,
        doc: doc_metadata_from_info(document.info()),
        nodes: document.nodes().to_vec(),
        // Pools are populated by a later increment (footnote/bibliography
        // collection); always-present empty vecs satisfy the 0.2.0 contract.
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

/// Serialize a manifest to pretty JSON.
pub fn manifest_to_json(manifest: &CndManifest) -> SourceResult<String> {
    serde_json::to_string_pretty(manifest).map_err(|err| {
        eco_vec![error!(
            Span::detached(),
            "failed to serialize CND manifest: {err}"
        )]
    })
}
