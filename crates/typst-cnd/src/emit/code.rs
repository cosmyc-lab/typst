use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::text::RawElem;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::model::CodeNode;

pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    raw: &Packed<RawElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(CodeNode, NodeRecord)> {
    let text = extract_text(&raw.clone().pack());
    let lang = raw.lang.get_cloned(styles).map(Into::into);

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = raw.clone().pack();
    // A code block carries no edges (spec §4: only texts hold them).
    let (record, _) = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = CodeNode::new(id, text.into(), location);
    node.lang = lang;

    Ok((node, record))
}
