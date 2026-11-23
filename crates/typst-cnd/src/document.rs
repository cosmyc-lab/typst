use std::collections::BTreeMap;
use std::sync::Arc;

use ecow::{EcoString, eco_vec};
use typst_layout::{PagedDocument, PagedIntrospector, layout_document};
use typst_library::diag::{SourceResult, error};
use typst_library::engine::Engine;
use typst_library::format::DocumentFormatOptions;
use typst_library::foundations::{
    Content, Datetime, Label, Output, Selector, SequenceElem, Smart, StyleChain,
    StyledElem, Target, Value,
};
use typst_library::introspection::{
    CounterUpdateElem, Introspector, MetadataElem, StateUpdateElem,
};
use typst_library::model::{Document, DocumentInfo, ParbreakElem};
use typst_library::text::{Locale, SpaceElem, TextElem};
use typst_syntax::Span;
use typst_utils::PicoStr;

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
        let options = paged.options().clone();

        // Containment is read off the laid-out frames: `Introspector::query`
        // returns a flat list, but the emit pipeline must know which located
        // elements sit inside a paragraph or inside page furniture.
        let ancestry = Ancestry::from_document(&paged, engine);
        let lang = doc_lang(&info, content, styles, &ancestry);

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
        refs::resolve_refs(&mut ctx, introspector.as_ref());

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

/// The document's language, which every text inherits (spec §4.1): the
/// locale the author set for the whole document (see [`author_locale`]),
/// else the language most of its glyphs are set in (see
/// `Ancestry::majority_lang`), else the text locale at the document's root,
/// Typst's default (`en`).
fn doc_lang(
    info: &DocumentInfo,
    content: &Content,
    styles: StyleChain,
    ancestry: &Ancestry,
) -> EcoString {
    author_locale(info, content, styles, ancestry)
        .map(|locale| locale.rfc_3066())
        .or_else(|| ancestry.majority_lang())
        .unwrap_or_else(|| Locale::get_in(styles).rfc_3066())
}

/// The locale the author set for the whole document, before any content.
///
/// Typst's `DocumentInfo::locale` is the first text language set at the
/// top level, but that may as well be an inline `text(lang:)` on a single
/// foreign word in the middle of the document. It is the author's when the
/// set rules at the top of the document set it (see [`leading_locale`]), or
/// when the document's first text is mostly set in it: a template applied
/// with `show:` sets it before the content it lays out, inside a function
/// the walk of the authored content cannot look into. A document opening on
/// a foreign phrase inside an otherwise English paragraph is not in that
/// phrase's language.
fn author_locale(
    info: &DocumentInfo,
    content: &Content,
    styles: StyleChain,
    ancestry: &Ancestry,
) -> Option<Locale> {
    leading_locale(content, styles).or_else(|| match info.locale {
        Smart::Custom(locale)
            if ancestry.first_text_lang().as_deref() == Some(&*locale.rfc_3066()) =>
        {
            Some(locale)
        }
        _ => None,
    })
}

/// The text language the outer styles set, or that set rules set before
/// any content.
///
/// A set rule styles everything after it in its scope, so at the top of a
/// document it is a styled element that is the only visible part of its
/// sequence; an inline `text(lang:)` styles its body alone, which has
/// siblings. The walk descends through such sole children, gathering their
/// styles, and stops at the first content: the text language set by then,
/// if any, is the author's. A show rule is not applied here: what a
/// template sets is not found.
fn leading_locale(content: &Content, styles: StyleChain) -> Option<Locale> {
    if let Some(styled) = content.to_packed::<StyledElem>() {
        return leading_locale(&styled.child, styles.chain(&styled.styles));
    }
    if let Some(sequence) = content.to_packed::<SequenceElem>() {
        let mut visible = sequence.children.iter().filter(|child| !is_invisible(child));
        if let (Some(only), None) = (visible.next(), visible.next()) {
            return leading_locale(only, styles);
        }
    }
    styles.has(TextElem::lang).then(|| Locale::get_in(styles))
}

/// Whether `content` lays out nothing that reads as document content:
/// white space between set rules, or an element that only records
/// something (metadata, a counter or state update).
fn is_invisible(content: &Content) -> bool {
    content.is::<SpaceElem>()
        || content.is::<ParbreakElem>()
        || content.is::<MetadataElem>()
        || content.is::<CounterUpdateElem>()
        || content.is::<StateUpdateElem>()
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
        doc: doc_metadata_from_info(
            document.info(),
            document.lang(),
            doc_markers(document.introspector().as_ref()),
        ),
        nodes: document.nodes().to_vec(),
        bibliography: document.bibliography().to_vec(),
        footnotes: document.footnotes().to_vec(),
    }
}

/// The label of the marker `cnd.document` writes.
const DOCUMENT_MARKER: &str = "cnd-document";

/// The `DocMetadata` fields an `extra` key may not repeat (spec §3).
const RESERVED_KEYS: [&str; 9] = [
    "title",
    "authors",
    "date",
    "keywords",
    "description",
    "lang",
    "status",
    "version",
    "extra",
];

/// What the author declared with `cnd.document`: `status`, `version` and
/// `extra`, read off the `<cnd-document>` markers in document order.
///
/// A later marker overrides an earlier one key by key. A non-string
/// `extra` value is dropped, and so is a key naming a `DocMetadata` field
/// (spec §3: the field, not `extra`, holds it).
pub fn doc_markers(
    introspector: &dyn Introspector,
) -> (Option<String>, Option<String>, BTreeMap<String, String>) {
    let mut status = None;
    let mut version = None;
    let mut extra = BTreeMap::new();
    let Some(label) = Label::new(PicoStr::intern(DOCUMENT_MARKER)) else {
        return (status, version, extra);
    };
    for elem in introspector.query(&Selector::Label(label)) {
        let Ok(Value::Dict(marker)) = elem.get_by_name("value") else { continue };
        if let Some(Value::Str(value)) = marker.get("status").ok().cloned() {
            status = Some(value.to_string());
        }
        if let Some(Value::Str(value)) = marker.get("version").ok().cloned() {
            version = Some(value.to_string());
        }
        if let Some(Value::Dict(pairs)) = marker.get("extra").ok().cloned() {
            for (key, value) in &pairs {
                if let Value::Str(value) = value
                    && !RESERVED_KEYS.contains(&key.as_str())
                {
                    extra.insert(key.to_string(), value.to_string());
                }
            }
        }
    }
    (status, version, extra)
}

fn doc_metadata_from_info(
    info: &DocumentInfo,
    lang: &str,
    (status, version, extra): (Option<String>, Option<String>, BTreeMap<String, String>),
) -> DocMetadata {
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
        status,
        version,
        extra,
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
