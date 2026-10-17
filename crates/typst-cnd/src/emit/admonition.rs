//! Admonitions, written with the authoring package's `cnd.admonition`
//! (cnd-sdk spec §6.11, ADR 0030).
//!
//! The helper writes no element of its own. It brackets the callout between
//! two labelled `metadata` markers: `<cnd-admonition>` before, whose value
//! holds the `kind` and the `title`, and `<cnd-admonition-end>` after. The
//! title, when given, is printed as the first block of the callout, labelled
//! `<cnd-admonition-title>`. So:
//!
//! - the node is built from the start marker, at its location: the page it
//!   is on, its state, its place in reading order;
//! - its `kind` and `title` are read from the marker's value, as authored —
//!   never from the printed title, whose `strong` is the helper's styling;
//! - its `children` are what is laid out between the two markers
//!   (`SlotKey::AdmonitionBody`, see `ancestry`), the printed title aside:
//!   that one is absorbed.
//!
//! The marker labels name no node: they are never a node's label, and never
//! enter the label index.

use ecow::EcoString;
use typst_library::engine::Engine;
use typst_library::foundations::{Content, Label, Repr, Selector, Value};
use typst_library::introspection::{Introspector, MetadataElem};
use typst_utils::PicoStr;

use crate::emit::convert::{self, NodeRecord};
use crate::emit::extract::{extract_text, label_text, source_text};
use crate::location::placeholder_location;
use crate::model::{AdmonitionNode, RichText, TextSlot};

/// The label of the marker before a callout, which holds its kind and title.
pub const START: &str = "cnd-admonition";
/// The label of the marker after a callout.
pub const END: &str = "cnd-admonition-end";
/// The label of the block a callout's title is printed in.
pub const TITLE: &str = "cnd-admonition-title";

/// The kind a callout has when the author gives none: the helper's default.
const DEFAULT_KIND: &str = "note";

/// Which admonition marker some content is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Start,
    End,
}

/// The admonition marker `content` is, if it is one: a `metadata` element
/// labelled `<cnd-admonition>` or `<cnd-admonition-end>`.
pub fn marker(content: &Content) -> Option<Marker> {
    if !content.is::<MetadataElem>() {
        return None;
    }
    let label = content.label()?;
    match label.resolve().as_str() {
        START => Some(Marker::Start),
        END => Some(Marker::End),
        _ => None,
    }
}

/// Whether `content` is the block a callout's title is printed in.
pub fn is_title(content: &Content) -> bool {
    content.label().is_some_and(|label| label.resolve().as_str() == TITLE)
}

/// Whether `label` is one of the labels the helper writes.
pub fn is_marker_label(label: Label) -> bool {
    matches!(label.resolve().as_str(), START | END | TITLE)
}

/// The selector of every start marker.
pub fn start_selector() -> Option<Selector> {
    Label::new(PicoStr::intern(START)).map(Selector::Label)
}

/// Convert a start marker into an `AdmonitionNode` with no children:
/// `convert::assemble` routes the blocks laid out in its body into it.
/// `lang` is the language of its printed title when it differs from the
/// document's.
pub fn convert(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    content: &Content,
    lang: Option<EcoString>,
) -> typst_library::diag::SourceResult<(AdmonitionNode, NodeRecord)> {
    let value = content.to_packed::<MetadataElem>().map(|marker| &marker.value);
    let field = |name: &str| match value {
        Some(Value::Dict(dict)) => dict.get(name).ok().cloned(),
        _ => None,
    };
    let kind = match field("kind") {
        Some(Value::Str(kind)) => kind.to_string(),
        Some(Value::Content(kind)) => extract_text(&kind).to_string(),
        // The helper's default; a value of another type passes through as
        // written.
        None | Some(Value::None) => DEFAULT_KIND.to_string(),
        Some(other) => other.repr().to_string(),
    };
    let title = match field("title") {
        Some(Value::Content(title)) => {
            Some(label_text(&title, &|span| source_text(engine, span)))
        }
        Some(Value::Str(title)) => Some((
            RichText::plain(title.as_str()),
            convert::TextRecord::new(TextSlot::Title),
        )),
        None | Some(Value::None) => None,
        Some(other) => Some((
            RichText::plain(other.repr().as_str()),
            convert::TextRecord::new(TextSlot::Title),
        )),
    }
    // A blank title says nothing.
    .filter(|(text, _)| !text.value.trim().is_empty());

    // The node's own record: its location and state. The marker's label is
    // the helper's, never the node's.
    let (mut record, _) = convert::make_record(engine, introspector, content, &[])?;
    record.label = None;

    let id = uuid::Uuid::new_v4();
    let mut node = AdmonitionNode::new(id, kind, placeholder_location());
    if let Some((mut text, mut edges)) = title {
        text.lang = lang.map(Into::into);
        edges.slot = TextSlot::Title;
        record.add_text(edges);
        node.title = Some(text);
    }
    Ok((node, record))
}
