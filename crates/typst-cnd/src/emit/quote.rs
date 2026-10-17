use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{Attribution, QuoteElem};

use crate::emit::convert::{self, Records, Synthetic, TextRecord};
use crate::emit::extract::{extract_text, label_text, source_text};
use crate::location::placeholder_location;
use crate::model::{QuoteNode, RichText, TextSlot};

/// Convert a block quote into a `QuoteNode`.
///
/// Its children are the blocks laid out in its body, which `assemble`
/// routes into it. Until then it holds one paragraph built from the body's
/// text, which stays only when nothing is routed: a show rule that boxes
/// the body (`show quote: it => box(it.body)`) lays it out inside a
/// paragraph of the flow, whose text leaves the quote out, so the quote
/// must carry it itself.
///
/// The attribution is a text of the quote's own, walked as authored
/// (`extract::label_text`), which gives its references and links. Its
/// citations and footnotes are those of the paragraph the attribution is
/// laid out in (see `convert`), the only one realized with their tags. The
/// edges of the body belong to the body's paragraphs, never to the quote.
///
/// `lang` is the quote's language when it differs from the document's.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    quote: &Packed<QuoteElem>,
    styles: StyleChain,
    lang: Option<EcoString>,
) -> typst_library::diag::SourceResult<(QuoteNode, Records)> {
    let attribution = quote
        .attribution
        .get_cloned(styles)
        .map(|attribution| attribution_text(engine, &attribution));

    let id = uuid::Uuid::new_v4();
    let packed = quote.clone().pack();
    // The quote's own record: its location, label and state. The edges of
    // its unrealized content are not the quote's.
    let (mut record, _) = convert::make_record(engine, introspector, &packed, &[])?;

    let mut node = QuoteNode::new(id, placeholder_location());
    if let Some((text, mut edges)) = attribution {
        edges.slot = TextSlot::Attribution;
        edges.cite_markers.clear();
        record.add_text(edges);
        node.attribution = Some(text);
    }

    // The fallback paragraph: the body's text, with the edges its
    // unrealized content shows. A body with no text (a shape) has none: an
    // empty paragraph is never a node.
    let mut synthetic = Synthetic::new(record.location);
    let text = extract_text(&quote.body);
    if !text.trim().is_empty() {
        let mut body = RichText::plain(text);
        body.lang = lang.map(Into::into);
        node.children
            .push(synthetic.paragraph_with(body, convert::unrealized_edges(&quote.body)));
    }

    Ok((
        node,
        Records {
            own: vec![(id, record)],
            synthetic: synthetic.records,
        },
    ))
}

/// The attribution's text with its edges: its content's, or a label's name
/// with none (a label attribution is realized as a citation, which the
/// attribution's paragraph carries).
fn attribution_text(
    engine: &Engine,
    attribution: &Attribution,
) -> (RichText, TextRecord) {
    match attribution {
        Attribution::Content(content) => {
            label_text(content, &|span| source_text(engine, span))
        }
        Attribution::Label(label) => (
            RichText::plain(label.resolve().as_str()),
            TextRecord::new(TextSlot::Attribution),
        ),
    }
}
