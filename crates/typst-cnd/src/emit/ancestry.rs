//! Containment relationships between laid-out elements, read off the frames.
//!
//! The emit pipeline works from `Introspector::query`, which returns a flat
//! list of located elements with no parent/child information. That is fine as
//! long as every queried element is a standalone node, but it is not: with
//! [`Feature::CndSemantics`] enabled every fully-inline fragment body becomes
//! a real `ParElem`, including the body of an inline `box` sitting *inside*
//! another paragraph. Emitting both would duplicate the box's text.
//!
//! The laid-out frames carry the missing structure. Introspection tags appear
//! in them as balanced `Tag::Start` / `Tag::End` pairs, and visiting the frame
//! tree depth-first yields them in document order, so a stack reconstructs
//! exact ancestry — no source-range or text-prefix heuristics.
//!
//! [`Feature::CndSemantics`]: typst_library::Feature::CndSemantics

use ecow::EcoString;
use rustc_hash::FxHashSet;
use typst_library::foundations::{Content, Element, NativeElement};
use typst_library::introspection::{Location, Tag};
use typst_library::layout::{Frame, FrameItem};
use typst_library::math::EquationElem;
use typst_library::model::{
    EnumElem, FigureElem, FootnoteEntry, HeadingElem, ListElem, ParElem, QuoteElem,
    TableElem, TermsElem,
};
use typst_library::pdf::ArtifactElem;
use typst_library::text::RawElem;

use crate::emit::extract;

/// Which of a laid-out element's ancestors matter to the emit pipeline.
#[derive(Debug, Default)]
pub struct Ancestry {
    /// Paragraphs whose text an enclosing paragraph already carries.
    covered_by_par: FxHashSet<Location>,
    /// Elements that sit inside an `ArtifactElem` — page furniture (headers,
    /// footers, page decorations), which Typst's accessibility model already
    /// marks as "not document content".
    in_artifact: FxHashSet<Location>,
    /// Elements that sit inside a container the emit pipeline turns into a
    /// node of its own — a list, a table, a code block, an equation. Such a
    /// node renders its own children, so a paragraph found inside one is that
    /// container's content seen a second time.
    in_container: FxHashSet<Location>,
}

impl Ancestry {
    /// Reconstruct ancestry from a laid-out document's frames.
    pub fn from_document(paged: &typst_layout::PagedDocument) -> Self {
        let mut ancestry = Self::default();
        let mut stack: Vec<Open> = Vec::new();
        for page in paged.pages() {
            walk(&page.frame, &mut stack, &mut ancestry);
        }
        ancestry
    }

    /// Whether an enclosing paragraph's own text already contains this
    /// paragraph's text, which makes emitting it a duplicate.
    ///
    /// This is the body of an inline `box`: the box is part of the enclosing
    /// paragraph's content, so that paragraph renders the text itself. It is
    /// deliberately *not* every paragraph with a paragraph ancestor — content
    /// reached through a `context` block is realized separately and is **not**
    /// part of the enclosing paragraph's text, so skipping it would drop it
    /// from the document entirely.
    pub fn is_covered_by_par(&self, location: Location) -> bool {
        self.covered_by_par.contains(&location)
    }

    /// Whether `location` sits inside page furniture marked as a PDF artifact.
    pub fn is_artifact(&self, location: Location) -> bool {
        self.in_artifact.contains(&location)
    }

    /// Whether `location` sits inside a container that is emitted as its own
    /// node — a list item's marker and body, a table cell, the lines of a code
    /// block, an equation's numbering.
    pub fn is_in_emitted_container(&self, location: Location) -> bool {
        self.in_container.contains(&location)
    }
}

/// Whether `elem` is emitted as a node that renders its own content, making
/// any paragraph laid out inside it a duplicate. Layout-only containers — a
/// bare `grid`, a `block`, a `place` — are deliberately absent: they produce
/// no node, so their paragraphs are the only carrier of that text.
fn is_emitted_container(elem: Element) -> bool {
    elem == HeadingElem::ELEM
        || elem == QuoteElem::ELEM
        || elem == ListElem::ELEM
        || elem == EnumElem::ELEM
        || elem == TermsElem::ELEM
        || elem == RawElem::ELEM
        || elem == EquationElem::ELEM
        || elem == FigureElem::ELEM
        || elem == TableElem::ELEM
        // A footnote's rendered entry at the bottom of the page: its text is
        // carried by the `footnotes` pool, which every marker refers to.
        || elem == FootnoteEntry::ELEM
}

/// A tag that is currently open while walking the frames.
struct Open {
    location: Location,
    /// The paragraph's own text, kept so a nested paragraph can be tested for
    /// being already covered by it. `None` for non-paragraphs.
    par_text: Option<EcoString>,
    is_artifact: bool,
    is_container: bool,
    /// Glyph runs drawn for this paragraph inside / outside artifact-marked
    /// content. A paragraph all of whose glyphs are drawn inside an artifact
    /// is page furniture, whichever way the two tags happen to nest.
    glyphs_in_artifact: u32,
    glyphs_outside_artifact: u32,
}

fn walk(frame: &Frame, stack: &mut Vec<Open>, ancestry: &mut Ancestry) {
    for (_, item) in frame.items() {
        match item {
            // Depth-first keeps the tag stream in document order.
            FrameItem::Group(group) => walk(&group.frame, stack, ancestry),
            FrameItem::Tag(Tag::Start(inner, _)) => {
                let Some(location) = inner.location() else { continue };
                let is_par = inner.elem() == ParElem::ELEM;

                if is_par && covered_by_enclosing_par(stack, inner) {
                    ancestry.covered_by_par.insert(location);
                }
                if stack.iter().any(|open| open.is_artifact) {
                    ancestry.in_artifact.insert(location);
                }
                if stack.iter().any(|open| open.is_container) {
                    ancestry.in_container.insert(location);
                }

                stack.push(Open {
                    location,
                    par_text: is_par.then(|| extract::extract_text(inner)),
                    is_artifact: inner.elem() == ArtifactElem::ELEM,
                    is_container: is_emitted_container(inner.elem()),
                    glyphs_in_artifact: 0,
                    glyphs_outside_artifact: 0,
                });
            }
            FrameItem::Tag(Tag::End(location, ..)) => {
                // Pop to the matching frame. Tags are balanced in practice;
                // popping by search rather than blindly keeps a stray end tag
                // from unwinding unrelated ancestors.
                if let Some(index) =
                    stack.iter().rposition(|open| open.location == *location)
                {
                    for open in stack.drain(index..) {
                        if open.par_text.is_some()
                            && open.glyphs_outside_artifact == 0
                            && open.glyphs_in_artifact > 0
                        {
                            ancestry.in_artifact.insert(open.location);
                        }
                    }
                }
            }
            FrameItem::Text(_) => {
                let in_artifact = stack.iter().any(|open| open.is_artifact);
                for open in stack.iter_mut().filter(|open| open.par_text.is_some()) {
                    if in_artifact {
                        open.glyphs_in_artifact += 1;
                    } else {
                        open.glyphs_outside_artifact += 1;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Whether the *nearest* enclosing paragraph's text already contains
/// `inner`'s text.
///
/// Only the nearest one is consulted: an outer paragraph further up may
/// contain the same words by coincidence, and dropping a paragraph on that
/// basis would lose content outright.
fn covered_by_enclosing_par(stack: &[Open], inner: &Content) -> bool {
    let text = collapse_whitespace(&extract::extract_text(inner));
    if text.is_empty() {
        return false;
    }
    let Some(nearest) = stack.iter().rev().find_map(|open| open.par_text.as_deref())
    else {
        return false;
    };
    collapse_whitespace(nearest).contains(&text)
}

/// Whitespace-insensitive form, so that a line break introduced by layout in
/// one of the two texts does not defeat the containment test.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
