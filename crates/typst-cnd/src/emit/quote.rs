use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{Attribution, QuoteElem};
use typst_library::text::Locale;

use crate::emit::convert::{self, NodeRecord};
use crate::location::placeholder_location;
use crate::manifest::QuoteNode;

pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    quote: &Packed<QuoteElem>,
    styles: StyleChain,
    doc_lang: Option<&str>,
) -> typst_library::diag::SourceResult<(QuoteNode, NodeRecord)> {
    let text: EcoString = quote.body.plain_text();
    let attribution = quote
        .attribution
        .get_cloned(styles)
        .map(|attribution| attribution_text(&attribution));
    let block = quote.block.get(styles);
    let lang = doc_lang
        .map(str::to_string)
        .or_else(|| Some(Locale::get_in(styles).rfc_3066().into()));

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = quote.clone().pack();
    let record = convert::make_record(engine, introspector, &packed)?;

    let mut node = QuoteNode::new(id, text.into(), location);
    node.attribution = attribution.map(Into::into);
    node.block = block;
    node.lang = lang;

    Ok((node, record))
}

fn attribution_text(attribution: &Attribution) -> EcoString {
    match attribution {
        Attribution::Content(content) => content.plain_text(),
        Attribution::Label(label) => label.resolve().as_str().into(),
    }
}
