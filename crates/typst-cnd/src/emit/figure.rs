use typst_library::engine::Engine;
use typst_library::foundations::{Content, Packed, Smart, StyleChain, Synthesize};
use typst_library::introspection::Introspector;
use typst_library::loading::DataSource;
use typst_library::model::{FigureElem, FigureKind};
use typst_library::visualize::ImageElem;
use uuid::Uuid;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::{label_text, source_text};
use crate::location::placeholder_location;
use crate::model::{FigureNode, ImageNode, RawSource};
use crate::rich_text::RichText;

/// Convert a figure into its wrapper node: kind, caption and numbering. Its
/// `children` are left empty: `convert::assemble` fills them with the
/// blocks laid out in its body, each a node of its own (an image, a code
/// block, a table, a layout, paragraphs, a nested figure…).
///
/// A `drawing` (see `Ancestry::is_drawing`) keeps no children: its `raw`
/// holds the source of its body instead, and the edges written anywhere in
/// the figure go to its caption. Any other figure's caption holds the edges
/// written in the caption only: those of its body sit on the body's texts.
pub fn from_figure(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    figure: &Packed<FigureElem>,
    drawing: bool,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(FigureNode, NodeRecord)> {
    let mut figure = figure.clone();
    figure.synthesize(engine, styles)?;

    // The caption is its body, without the supplement or the number: a
    // label text, walked as authored.
    let caption = figure
        .caption
        .get_cloned(styles)
        .map(|caption| label_text(&caption.body, &|span| source_text(engine, span)));
    let fig_number = convert::figure_number(engine, &figure, styles);
    let (mut record, _) = convert::make_record(engine, introspector, content, &[])?;

    let mut node = FigureNode::new(Uuid::new_v4(), placeholder_location());
    node.numbering =
        convert::numbering(fig_number, convert::figure_counter_label(&figure, styles));
    node.kind = figure_kind(&figure, styles);
    // Typst's `alt` is the text equivalent of the figure's visual, which is
    // what the description is (cnd-sdk spec §6.7). An image's own `alt`
    // stays on the image. A blank `alt` describes nothing.
    node.description = figure
        .alt
        .get_cloned(styles)
        .filter(|alt| !alt.trim().is_empty())
        .map(RichText::plain);
    if drawing {
        node.raw = convert::source_text(engine, figure.body.span()).map(RawSource::typst);
    }

    if let Some((text, mut edges)) = caption {
        // A drawing has no texts in its body: what is written there is the
        // caption's.
        if drawing {
            edges.merge(convert::unrealized_edges(&figure.body));
        }
        edges.slot = crate::model::TextSlot::Caption;
        record.add_text(edges);
        node.caption = Some(text);
    }
    Ok((node, record))
}

/// Convert an `#image(...)` into an `ImageNode`: bare outside any figure —
/// the "bare image outside any figure is an `ImageNode` with no wrapper"
/// shape the CND model allows — or a figure's child, which
/// `convert::assemble` routes into its body. The image owns its location;
/// the figure around it owns its own.
pub fn bare_image(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    image: &Packed<ImageElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(ImageNode, NodeRecord)> {
    // An image carries no edges (spec §4: only texts hold them).
    let (record, _) = convert::make_record(engine, introspector, content, &[])?;
    let id = Uuid::new_v4();
    let mut node = ImageNode::new(id, placeholder_location());
    node.alt = image.alt.get_cloned(styles).map(Into::into);
    node.path = image_path(image);
    Ok((node, record))
}

/// Read the figure kind Typst itself already resolved during
/// `synthesize()` — auto-detected via the `Figurable` trait (table, raw,
/// image) or an author-supplied custom string — instead of recomputing a
/// cruder image-only guess. `kind` is always `Smart::Custom` after
/// `synthesize()`; the `None` arm only guards a future Typst change.
pub(crate) fn figure_kind(
    figure: &Packed<FigureElem>,
    styles: StyleChain,
) -> Option<String> {
    match figure.kind.get_cloned(styles) {
        Smart::Custom(FigureKind::Elem(func)) => Some(func.name().to_string()),
        Smart::Custom(FigureKind::Name(name)) => Some(name.to_string()),
        Smart::Auto => None,
    }
}

fn image_path(image: &Packed<ImageElem>) -> Option<String> {
    match &image.source.source {
        DataSource::Path(path) => path
            .resolve_if_some(image.span().id())
            .ok()
            .map(|resolved| resolved.vpath().get_without_slash().to_string()),
        DataSource::Bytes(_) => None,
    }
}
