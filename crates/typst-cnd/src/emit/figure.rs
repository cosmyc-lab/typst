use typst_library::engine::Engine;
use typst_library::foundations::{Content, NativeElement, Packed, Smart, StyleChain, Synthesize};
use typst_library::introspection::Introspector;
use typst_library::loading::DataSource;
use typst_library::model::{FigureElem, FigureKind};
use typst_library::text::RawElem;
use typst_library::visualize::ImageElem;
use uuid::Uuid;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::{code, table};
use crate::location::placeholder_location;
use crate::model::{CndNode, FigureNode, ImageNode, RawSource};

/// Convert a non-table figure (image, code, custom kind, …) into a wrapper
/// `FigureNode` plus whatever children it carries. Table/grid figures are
/// intercepted earlier in `convert::dispatch` and never reach this
/// function (see `table::from_figure`/`table::from_figure_grid`).
pub fn from_figure(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    figure: &Packed<FigureElem>,
    styles: StyleChain,
) -> typst_library::diag::SourceResult<(FigureNode, Vec<(Uuid, NodeRecord)>)> {
    let mut figure = figure.clone();
    figure.synthesize(engine, styles)?;

    let caption = figure
        .caption
        .get_cloned(styles)
        .as_ref()
        .map(convert::caption_text)
        .map(Into::into);
    let fig_number = convert::figure_number(engine, &figure, styles).map(Into::into);
    let record = convert::make_record(engine, introspector, content, &[])?;

    let id = uuid::Uuid::new_v4();
    let location = placeholder_location();
    let mut node = FigureNode::new(id, location);
    node.caption = caption;
    node.number = fig_number;
    node.counter_label =
        convert::figure_counter_label(&figure, styles).map(Into::into);
    node.kind = figure_kind(&figure, styles);

    let mut records = vec![(id, record)];

    if let Some(image) = content.query_first_naive(&ImageElem::ELEM.select()) {
        if let Some(image) = image.to_packed::<ImageElem>() {
            let child_id = Uuid::new_v4();
            let mut child = ImageNode::new(child_id, placeholder_location());
            child.alt = image.alt.get_cloned(styles).map(Into::into);
            child.path = image_path(image);
            records.push((child_id, minimal_record(content)));
            node.children.push(CndNode::Image(child));
        }
    } else if let Some(raw) = content.query_first_naive(&RawElem::ELEM.select()) {
        if let Some(raw) = raw.to_packed::<RawElem>() {
            let (child, child_record) = code::convert(engine, introspector, raw, styles)?;
            let child_id = child.base.id;
            records.push((child_id, child_record));
            node.children.push(CndNode::Code(child));
        }
    }

    node.raw = table::raw_typst_for_label(engine, content.label()).map(RawSource::typst);

    Ok((node, records))
}

/// A record carrying only a location, for a figure's synthetic child node —
/// nothing else is inherited from the wrapper (ADR 0010).
fn minimal_record(content: &Content) -> NodeRecord {
    NodeRecord {
        location: content.location(),
        label: None,
        ref_targets: Vec::new(),
        footnote_locs: Vec::new(),
        cite_markers: Vec::new(),
        ref_markers: Vec::new(),
        state_metadata: std::collections::HashMap::new(),
    }
}

/// Read the figure kind Typst itself already resolved during
/// `synthesize()` — auto-detected via the `Figurable` trait (table, raw,
/// image) or an author-supplied custom string — instead of recomputing a
/// cruder image-only guess. `kind` is always `Smart::Custom` after
/// `synthesize()`; the `None` arm only guards a future Typst change.
pub(crate) fn figure_kind(figure: &Packed<FigureElem>, styles: StyleChain) -> Option<String> {
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
        _ => None,
    }
}
