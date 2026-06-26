use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::ParElem;
use typst_library::text::Locale;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::manifest::ParagraphNode;

/// Convert a realized paragraph into a CND paragraph node.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    par: &Packed<ParElem>,
    styles: StyleChain,
    doc_lang: Option<&str>,
) -> typst_library::diag::SourceResult<(ParagraphNode, NodeRecord)> {
    let text = extract_text(&par.body).into();
    let lang = doc_lang
        .map(str::to_string)
        .or_else(|| Some(Locale::get_in(styles).rfc_3066().into()));
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = par.clone().pack();
    let record = convert::make_record(engine, introspector, &packed)?;

    let mut node = ParagraphNode::new(id, text, location);
    node.lang = lang;

    Ok((node, record))
}
