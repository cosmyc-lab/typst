use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::text::RawElem;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::manifest::CodeNode;

pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    raw: &Packed<RawElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(CodeNode, NodeRecord)> {
    let block = raw.block.get(styles);
    let text: EcoString = extract_text(&raw.clone().pack());
    let lang = raw.lang.get_cloned(styles).map(Into::into);

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = raw.clone().pack();
    let record = convert::make_record(engine, introspector, &packed)?;

    let mut node = CodeNode::new(id, text.into(), location);
    node.lang = lang;
    node.block = block;

    Ok((node, record))
}
