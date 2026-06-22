use typst_library::engine::Engine;
use typst_library::foundations::{Content, NativeElement, Packed, StyleChain, Synthesize};
use typst_library::introspection::Introspector;
use typst_library::loading::DataSource;
use typst_library::model::FigureElem;
use typst_library::visualize::ImageElem;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::table;
use crate::location::placeholder_location;
use crate::manifest::FigureNode;

/// Convert a non-table figure (image, rect, …).
pub fn from_figure(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    figure: &Packed<FigureElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(FigureNode, NodeRecord)> {
    let mut figure = figure.clone();
    figure.synthesize(engine, styles)?;

    let caption = figure
        .caption
        .get_cloned(styles)
        .as_ref()
        .map(convert::caption_text)
        .map(Into::into);
    let fig_number = convert::figure_number(engine, &figure, styles).map(Into::into);
    let record = convert::make_record(engine, introspector, content)?;

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let mut node = FigureNode::new(id, location);
    node.caption = caption;
    node.fig_number = fig_number;
    node.kind = figure_kind(content);
    if let Some(image) = content.query_first_naive(&ImageElem::ELEM.select()) {
        if let Some(image) = image.to_packed::<ImageElem>() {
            node.alt = image.alt.get_cloned(styles).map(Into::into);
            node.path = image_path(image);
        }
    }
    node.raw_typst = table::raw_typst_for_label(engine, content.label());

    Ok((node, record))
}

fn figure_kind(content: &Content) -> Option<String> {
    if content.query_first_naive(&ImageElem::ELEM.select()).is_some() {
        return Some("image".into());
    }
    Some("other".into())
}

fn image_path(image: &Packed<ImageElem>) -> Option<String> {
    match &image.source.source {
        DataSource::Path(path) => path
            .resolve_if_some(image.span().id())
            .ok()
            .map(|resolved| resolved.vpath().get_without_slash().to_string()),
        _ => None,
    }
}
