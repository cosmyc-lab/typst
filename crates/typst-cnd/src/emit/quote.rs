use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{Attribution, QuoteElem};
use typst_library::text::Locale;

use crate::emit::convert::{self, Records, Synthetic};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::model::{QuoteNode, RichText};

/// Convert a block quote into a `QuoteNode`.
///
/// Its children are the blocks laid out in its body, which `assemble`
/// routes into it. Until then it holds one paragraph built from the body's
/// text, which stays only when nothing is routed: a show rule that boxes
/// the body (`show quote: it => box(it.body)`) lays it out inside a
/// paragraph of the flow, whose text leaves the quote out, so the quote
/// must carry it itself.
///
/// The attribution is a text of the quote's own. Its edges are those of the
/// paragraph the attribution is laid out in (see `convert`); the edges of
/// the body belong to the body's paragraphs, never to the quote.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    quote: &Packed<QuoteElem>,
    styles: StyleChain,
    doc_lang: Option<&str>,
) -> typst_library::diag::SourceResult<(QuoteNode, Records)> {
    let attribution = quote
        .attribution
        .get_cloned(styles)
        .map(|attribution| attribution_text(&attribution));
    let lang = doc_lang
        .map(str::to_string)
        .or_else(|| Some(Locale::get_in(styles).rfc_3066().into()));

    let id = uuid::Uuid::new_v4();
    let packed = quote.clone().pack();
    // The quote's own record: its location, label and state. The edges of
    // its unrealized content are not the quote's.
    let (record, _) = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = QuoteNode::new(id, placeholder_location());
    node.attribution = attribution.map(RichText::plain);

    // The fallback paragraph: the body's text, with the edges its
    // unrealized content shows. A body with no text (a shape) has none: an
    // empty paragraph is never a node.
    let mut synthetic = Synthetic::new(record.location);
    let text = extract_text(&quote.body);
    if !text.trim().is_empty() {
        let mut body = RichText::plain(text);
        body.lang = lang;
        node.children
            .push(synthetic.paragraph_with(body, convert::unrealized_edges(&quote.body)));
    }

    Ok((
        node,
        Records {
            own: vec![(id, record)],
            synthetic: synthetic.records,
        },
    ))
}

/// The attribution's plain text: its content's, or a label's name.
fn attribution_text(attribution: &Attribution) -> EcoString {
    match attribution {
        Attribution::Content(content) => extract_text(content),
        Attribution::Label(label) => label.resolve().as_str().into(),
    }
}
