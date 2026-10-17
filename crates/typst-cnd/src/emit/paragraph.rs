use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::Packed;
use typst_library::introspection::Introspector;
use typst_library::model::ParElem;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::{extract_rich, source_text};
use crate::location::placeholder_location;
use crate::model::{ParagraphNode, RichText};

/// Convert a realized paragraph into a CND paragraph node. `lang` is its
/// language when it differs from the document's.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    par: &Packed<ParElem>,
    lang: Option<EcoString>,
) -> typst_library::diag::SourceResult<(ParagraphNode, NodeRecord)> {
    let extracted = extract_rich(&par.body, &|span| source_text(engine, span));
    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let packed = par.clone().pack();
    let (mut record, mut edges) =
        convert::make_record(engine, introspector, &packed, &extracted.markers)?;
    // A citation or footnote written in a term is the term's: the
    // description paragraph the term is realized in leaves it out.
    let suppressed = &extracted.suppressed;
    edges.footnote_locs.retain(|(loc, _)| !suppressed.contains(loc));
    edges
        .cite_markers
        .retain(|cite| cite.loc.is_none_or(|loc| !suppressed.contains(&loc)));
    record.add_text(edges);

    let mut text = RichText::plain(extracted.value);
    text.lang = lang.map(Into::into);
    text.marks = extracted.marks;
    let node = ParagraphNode::new(id, text, location);

    Ok((node, record))
}
