use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::HeadingElem;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::{label_text, source_text};
use crate::location::placeholder_location;
use crate::model::HeadingNode;

/// Convert a realized heading element into a CND heading node. `lang` is
/// its language when it differs from the document's.
///
/// Its `heading_path` holds its own segment only: the path of the section
/// it sits in is prepended where it is placed (`convert::assemble`).
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    heading: &Packed<HeadingElem>,
    styles: StyleChain,
    lang: Option<EcoString>,
) -> typst_library::diag::SourceResult<(HeadingNode, NodeRecord)> {
    let level = heading.resolve_level(styles).get() as i32;
    // Typst yields an empty string for an unnumbered heading; the format has
    // no such third state — `numbering` is a resolved value or absent.
    let numbering: EcoString = heading.numbers.clone().unwrap_or_default();
    // A heading's body is met as authored: a label text.
    let (mut text, edges) = label_text(&heading.body, &|span| source_text(engine, span));
    text.lang = lang.map(Into::into);
    let segment = if numbering.is_empty() {
        text.value.clone()
    } else {
        format!("{numbering} {}", text.value)
    };

    let heading_path = vec![segment];

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = heading.clone().pack();
    // The heading's own record; its text's edges are the label text's.
    let (mut record, _) = convert::make_record(engine, introspector, &packed, &[])?;
    record.add_text(edges);

    let node = HeadingNode::new(
        id,
        level,
        convert::numbering(Some(numbering), None::<String>),
        text,
        heading_path,
        location,
    );
    Ok((node, record))
}
