use ecow::EcoString;
use typst_library::World;
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain, Synthesize};
use typst_library::introspection::Introspector;
use typst_library::math::EquationElem;
use typst_library::model::Refable;
use typst_syntax::Span;

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

    let text: EcoString = extract_text(&equation.body);
    let block = equation.block.get(styles);
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
    let record = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = MathNode::new(id, text.into(), location);
    node.block = block;
    node.number = numbering.map(Into::into);
    node.raw = raw_typst_from_span(engine, equation.span()).map(RawSource::typst);

    Ok((node, record))
}

fn raw_typst_from_span(engine: &Engine, span: Span) -> Option<String> {
    let id = span.id()?;
    let range = engine.world.range(span)?;
    let source = engine.world.source(id).ok()?;
    let text = source.text();
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start >= end {
        return None;
    }
    Some(text[start..end].trim().to_string())
}
