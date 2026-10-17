use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain, Synthesize};
use typst_library::introspection::Introspector;
use typst_library::math::EquationElem;
use typst_library::model::Refable;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::extract_text;
use crate::location::placeholder_location;
use crate::model::{MathNode, RawSource};

pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    equation: &Packed<EquationElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(MathNode, NodeRecord)> {
    let mut equation = equation.clone();
    equation.synthesize(engine, styles)?;

    let text = extract_text(&equation.body);
    let numbering: Option<EcoString> = equation.numbering().and_then(|numbering| {
        let location = equation.location()?;
        equation
            .counter()
            .display_at(engine, location, styles, numbering, equation.span())
            .ok()
            .map(|display| extract_text(&display))
    });

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = equation.clone().pack();
    // An equation carries no edges (spec §4: only texts hold them).
    let (record, _) = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = MathNode::new(id, text.into(), location);
    node.numbering = convert::numbering(numbering, None::<String>);
    node.raw = convert::source_text(engine, equation.span()).map(RawSource::typst);

    Ok((node, record))
}
