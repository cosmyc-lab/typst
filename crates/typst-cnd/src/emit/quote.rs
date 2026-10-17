use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{Attribution, QuoteElem};
use typst_library::text::Locale;

use crate::emit::convert::{self, Records, Synthetic};
use crate::emit::extract::{extract_text, extract_with_markers};
use crate::location::placeholder_location;
use crate::model::{QuoteNode, RichText};

/// Convert a quote into a `QuoteNode`. Its body becomes one paragraph child
/// holding the quote text, with the quote's markers and their spans (they
/// index that same text).
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    quote: &Packed<QuoteElem>,
    styles: StyleChain,
    doc_lang: Option<&str>,
) -> typst_library::diag::SourceResult<(QuoteNode, Records)> {
    let (text, markers) = extract_with_markers(&quote.body);
    let attribution = quote
        .attribution
        .get_cloned(styles)
        .map(|attribution| attribution_text(&attribution));
    let lang = doc_lang
        .map(str::to_string)
        .or_else(|| Some(Locale::get_in(styles).rfc_3066().into()));

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = quote.clone().pack();
    let (record, edges) = convert::make_record(engine, introspector, &packed, &markers)?;

    let mut synthetic = Synthetic::new(record.location);
    let mut body = RichText::plain(text);
    body.lang = lang;
    let paragraph = synthetic.paragraph(body);

    let mut node = QuoteNode::new(id, location);
    node.attribution = attribution.map(RichText::plain);
    node.children.push(paragraph);
    let _ = synthetic.attach_to_first_paragraph(&node.children, edges);

    Ok((
        node,
        Records {
            own: vec![(id, record)],
            synthetic: synthetic.records,
        },
    ))
}

fn attribution_text(attribution: &Attribution) -> EcoString {
    match attribution {
        Attribution::Content(content) => extract_text(content),
        Attribution::Label(label) => label.resolve().as_str().into(),
    }
}
