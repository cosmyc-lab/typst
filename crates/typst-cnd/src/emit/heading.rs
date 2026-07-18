use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::HeadingElem;

use crate::emit::convert::{self, HeadingFrame, NodeRecord};
use crate::emit::extract::extract_with_markers;
use crate::location::placeholder_location;
use crate::manifest::HeadingNode;

/// Convert a realized heading element into a CND heading node.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    heading: &Packed<HeadingElem>,
    styles: StyleChain,
    stack: &[HeadingFrame],
) -> typst_library::diag::SourceResult<(HeadingNode, NodeRecord)> {
    let level = heading.resolve_level(styles).get() as i32;
    let numbering: EcoString = heading.numbers.clone().unwrap_or_default();
    let (text, markers) = extract_with_markers(&heading.body);
    let segment = if numbering.is_empty() {
        text.clone()
    } else {
        ecow::eco_format!("{numbering} {text}")
    };

    let mut heading_path: Vec<String> = stack
        .iter()
        .map(|frame| frame.path.last().cloned().unwrap_or_default())
        .collect();
    heading_path.push(segment.into());

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = heading.clone().pack();
    let record = convert::make_record(engine, introspector, &packed, &markers)?;

    let node = HeadingNode::new(id, level, numbering.into(), text.into(), heading_path, location);
    Ok((node, record))
}
