use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::HeadingElem;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_with_markers;
use crate::location::placeholder_location;
use crate::model::{HeadingNode, RichText};

/// Convert a realized heading element into a CND heading node.
///
/// Its `heading_path` holds its own segment only: the path of the section
/// it sits in is prepended where it is placed (`convert::assemble`).
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    heading: &Packed<HeadingElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(HeadingNode, NodeRecord)> {
    let level = heading.resolve_level(styles).get() as i32;
    // Typst yields an empty string for an unnumbered heading; the format has
    // no such third state — `numbering` is a resolved value or absent.
    let numbering: EcoString = heading.numbers.clone().unwrap_or_default();
    let (text, markers) = extract_with_markers(&heading.body);
    let segment = if numbering.is_empty() {
        text.clone()
    } else {
        ecow::eco_format!("{numbering} {text}")
    };

    let heading_path = vec![segment.into()];

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = heading.clone().pack();
    let (mut record, edges) =
        convert::make_record(engine, introspector, &packed, &markers)?;
    record.add_text(edges);

    let node = HeadingNode::new(
        id,
        level,
        convert::numbering(Some(numbering), None::<String>),
        RichText::plain(text),
        heading_path,
        location,
    );
    Ok((node, record))
}
