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

use std::ops::ControlFlow;

use rustc_hash::{FxHashMap, FxHashSet};
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{Content, Element, NativeElement};
use typst_library::introspection::{Location, Tag};
use typst_library::layout::{Frame, FrameItem};
use typst_library::math::EquationElem;
use typst_library::model::{
    ArtifactElem, BibliographyElem, EnumElem, FigureElem, FootnoteEntry, HeadingElem,
    ListElem, OutlineElem, ParElem, QuoteElem, TableElem, TermsElem,
};
use typst_library::text::RawElem;
use typst_syntax::{FileId, Span};

/// Which of a laid-out element's ancestors matter to the emit pipeline.
#[derive(Debug, Default)]
pub struct Ancestry {
    /// Paragraphs that are part of an enclosing paragraph's own content,
    /// mapped to that paragraph. Emitting both would duplicate the text; the
    /// enclosing one absorbs the nested one's footnote/citation/reference
    /// edges instead.
    covered_by_par: FxHashMap<Location, Location>,
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
    pub fn from_document(paged: &typst_layout::PagedDocument, engine: &Engine) -> Self {
        let mut ancestry = Self::default();
        let mut stack: Vec<Open> = Vec::new();
        for page in paged.pages() {
            walk(&page.frame, engine, &mut stack, &mut ancestry);
        }
        ancestry
    }

    /// The paragraph this one is part of, if any.
    ///
    /// The free function of the same name below states the rule.
    pub fn covering_par(&self, location: Location) -> Option<Location> {
        self.covered_by_par.get(&location).copied()
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

/// Whether a paragraph laid out inside `elem` is a second copy of text the
/// document already carries elsewhere. Layout-only containers — a bare
/// `grid`, a `block`, a `place` — are deliberately absent: they produce no
/// node, so their paragraphs are the only carrier of that text.
///
/// Two kinds qualify. Most are emitted as a node that renders its own
/// content, so a paragraph inside is that content seen twice. The outline is
/// the exception: it is emitted as nothing at all, and every entry repeats a
/// heading that is already a node of its own — navigation furniture.
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
        // Likewise a rendered bibliography entry, whose text the
        // `bibliography` pool carries. With the default title the entries are
        // swallowed by the generated heading's source range; with
        // `title: none` — a hand-written "= References" — nothing catches
        // them and each entry leaks as two paragraphs.
        || elem == BibliographyElem::ELEM
        // Table of contents.
        || elem == OutlineElem::ELEM
}

/// A tag that is currently open while walking the frames.
struct Open {
    location: Location,
    /// The paragraph's own source range, against which a nested paragraph is
    /// tested for being part of it. `None` for non-paragraphs and for a
    /// paragraph with no usable span.
    par_range: Option<Option<SourceRange>>,
    is_artifact: bool,
    is_container: bool,
    /// Glyph runs drawn for this paragraph inside / outside artifact-marked
    /// content. A paragraph all of whose glyphs are drawn inside an artifact
    /// is page furniture, whichever way the two tags happen to nest.
    glyphs_in_artifact: u32,
    glyphs_outside_artifact: u32,
}

fn walk(frame: &Frame, engine: &Engine, stack: &mut Vec<Open>, ancestry: &mut Ancestry) {
    for (_, item) in frame.items() {
        match item {
            // Depth-first keeps the tag stream in document order.
            FrameItem::Group(group) => walk(&group.frame, engine, stack, ancestry),
            FrameItem::Tag(Tag::Start(inner, _)) => {
                let Some(location) = inner.location() else { continue };
                let is_par = inner.elem() == ParElem::ELEM;

                let par_range = is_par.then(|| content_range(engine, inner));
                if let Some(range) = par_range
                    && let Some(parent) = covering_par(stack, range)
                {
                    ancestry.covered_by_par.insert(location, parent);
                }
                if stack.iter().any(|open| open.is_artifact) {
                    ancestry.in_artifact.insert(location);
                }
                if stack.iter().any(|open| open.is_container) {
                    ancestry.in_container.insert(location);
                }

                stack.push(Open {
                    location,
                    par_range,
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
                        if open.par_range.is_some()
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
                for open in stack.iter_mut().filter(|open| open.par_range.is_some()) {
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

/// A half-open byte range in one source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceRange {
    file: FileId,
    start: usize,
    end: usize,
}

fn source_range(engine: &Engine, span: Span) -> Option<SourceRange> {
    let file = span.id()?;
    let range = engine.world.range(span)?;
    Some(SourceRange { file, start: range.start, end: range.end })
}

/// How far `content` reaches in the source.
///
/// A `ParElem`'s own span covers only what it starts with — for
/// `Look at #box[…] please.` it is the seven bytes of `Look at` — so it
/// cannot be asked whether something is written inside the paragraph. The
/// union of the spans of everything the paragraph holds can.
///
/// Spans from other files are ignored: content pulled in from a template or
/// a `#let` defined elsewhere would otherwise stretch the range across a file
/// it says nothing about.
fn content_range(engine: &Engine, content: &Content) -> Option<SourceRange> {
    let mut range = source_range(engine, content.span());
    let file = range.map(|r| r.file).or_else(|| content.span().id())?;
    let _ = content.traverse(&mut |element| {
        if let Some(found) = source_range(engine, element.span())
            && found.file == file
        {
            range = Some(match range {
                Some(cur) => SourceRange {
                    file,
                    start: cur.start.min(found.start),
                    end: cur.end.max(found.end),
                },
                None => found,
            });
        }
        ControlFlow::<()>::Continue(())
    });
    range
}

/// The paragraph `range` is part of, if any.
///
/// Two independent signals must agree, and each guards a different mistake.
///
/// *Nesting in the laid-out frames* — the nearest enclosing open paragraph —
/// is what identifies an inline `box`: its body is realized into a paragraph
/// of its own, drawn inside the line of the paragraph that contains it. This
/// is deliberately not a text comparison: the box body is realized and the
/// enclosing paragraph's copy of it is not, so anything that only acquires
/// text when realized — a `@reference`'s "Section 1", a citation's marker, a
/// footnote's ordinal — appears in one and not the other, and a paragraph
/// holding `#box[see @fig]` would compare unequal and be emitted twice.
///
/// *Source containment* is what keeps a float out. `#place(float: true)[…]`
/// written inside a paragraph is deferred to wherever it fits, and its tags
/// can land inside a *later* paragraph's still-open range — nesting alone
/// would delete it, which is content loss rather than duplication. A float
/// body's source range is never inside the range of the paragraph it lands
/// in: a block-level `place` ends the paragraph it was written in, so the
/// paragraph that ends up enclosing it in the frames always begins after it.
fn covering_par(stack: &[Open], range: Option<SourceRange>) -> Option<Location> {
    let range = range?;
    let nearest = stack.iter().rev().find(|open| open.par_range.is_some())?;
    let parent = nearest.par_range.flatten()?;
    contains(parent, range).then_some(nearest.location)
}

fn contains(outer: SourceRange, inner: SourceRange) -> bool {
    outer.file == inner.file && inner.start >= outer.start && inner.end <= outer.end
}
