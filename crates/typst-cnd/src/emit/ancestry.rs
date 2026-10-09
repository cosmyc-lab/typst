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
//! exact ancestry. Only whether a nested paragraph is part of the enclosing
//! one needs a second signal; [`covering_par`] states it.
//!
//! [`Feature::CndSemantics`]: typst_library::Feature::CndSemantics

use std::cell::OnceCell;
use std::ops::ControlFlow;

use rustc_hash::{FxHashMap, FxHashSet};
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, Element, NativeElement, SequenceElem, StyleChain,
};
use typst_library::introspection::{Location, Tag};
use typst_library::layout::{Frame, FrameItem, PlaceElem};
use typst_library::math::EquationElem;
use typst_library::model::{
    ArtifactElem, BibliographyElem, EnumElem, FigureElem, FootnoteEntry, HeadingElem,
    ListElem, OutlineElem, ParElem, ParbreakElem, QuoteElem, TableElem, TermsElem,
};
use typst_library::text::RawElem;
use typst_syntax::{FileId, Span};

use crate::emit::extract;

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
    /// Elements whose text is carried by an enclosing node, so that a list
    /// among them is not a node of its own. See [`list_scope`].
    list_carried: FxHashSet<Location>,
}

impl Ancestry {
    /// Reconstruct ancestry from a laid-out document's frames.
    pub fn from_document(paged: &typst_layout::PagedDocument, engine: &Engine) -> Self {
        let mut ancestry = Self::default();
        let mut found = Vec::new();
        for page in paged.pages() {
            floats(&page.frame, engine, &mut found);
        }
        let mut stack: Vec<Open> = Vec::new();
        for page in paged.pages() {
            walk(&page.frame, engine, &found, &mut stack, &mut ancestry);
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

    /// Whether a list at `location` has its text carried by an enclosing
    /// node — a list, an enum, a definition list, a quote, or the footnote
    /// pool — and so is not a node of its own.
    pub fn is_list_carried(&self, location: Location) -> bool {
        self.list_carried.contains(&location)
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

/// What an element does to the text of a list laid out inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListScope {
    /// Nothing: the decision is up to an element further out.
    Transparent,
    /// It carries the list's text: a list or an enum (as an item's
    /// children), a definition list (a description's text), a quote (its
    /// text), a footnote entry (the `footnotes` pool).
    Carrier,
    /// It ends the search, and the list is a node of its own. A figure keeps
    /// only its caption, so a list inside it is that text's only carrier. A
    /// float is laid out wherever it fits — possibly while a long list,
    /// whose tags are still open, spans the page — so the elements open
    /// around it in the frames are not what it was written in.
    Barrier,
}

/// The nearest enclosing element that is not transparent decides whether a
/// list is carried. Asking only "is any list-like element open?" would let a
/// float placed beside an open list lose the list inside it.
fn list_scope(inner: &Content) -> ListScope {
    let elem = inner.elem();
    if elem == ListElem::ELEM
        || elem == EnumElem::ELEM
        || elem == TermsElem::ELEM
        || elem == QuoteElem::ELEM
        || elem == FootnoteEntry::ELEM
    {
        ListScope::Carrier
    } else if elem == FigureElem::ELEM
        // A `place` that does not float is laid out where it is written,
        // inside its real ancestors.
        || is_float(inner)
    {
        ListScope::Barrier
    } else {
        ListScope::Transparent
    }
}

/// Whether `inner` is a `place` that floats. Materialized, so `float` holds
/// its realized value (see `extract::is_excluded_scope`).
fn is_float(inner: &Content) -> bool {
    inner
        .to_packed::<PlaceElem>()
        .is_some_and(|place| place.float.get(StyleChain::default()))
}

/// What identifies a float's body once it is drawn away from its tags.
struct Float {
    /// Where the `place` is written. A paragraph whose own range lies inside
    /// it is that float's content, whatever text it realizes to.
    range: Option<SourceRange>,
    /// The body's text, for a paragraph with no range of its own (a string
    /// passed through `upper`, `text`…), which only its words can identify.
    text: String,
}

impl Float {
    fn holds(&self, par: &OpenPar) -> bool {
        par.range
            .zip(self.range)
            .is_some_and(|(inner, outer)| contains(outer, inner))
            || occurs_as_words(&self.text, par.text())
    }
}

/// Collect every float of the document.
///
/// A separate pass, because a float is drawn away from its tags: they open
/// and close where it is written, and its body lands wherever it fits —
/// possibly before them, at the top of the page.
fn floats(frame: &Frame, engine: &Engine, out: &mut Vec<Float>) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => floats(&group.frame, engine, out),
            FrameItem::Tag(Tag::Start(inner, _)) => {
                if is_float(inner)
                    && let Some(place) = inner.to_packed::<PlaceElem>()
                {
                    out.push(Float {
                        range: content_range(engine, inner),
                        text: paragraphs_text(&place.body),
                    });
                }
            }
            _ => {}
        }
    }
}

/// The normalized text of `body`, with a space where a paragraph break
/// separates two of its paragraphs — `extract_text` yields nothing for the
/// break, which would glue the last word of one to the first of the next.
fn paragraphs_text(body: &Content) -> String {
    let Some(sequence) = body.to_packed::<SequenceElem>() else {
        return normalized_text(body);
    };
    sequence
        .children
        .split(|child| child.is::<ParbreakElem>())
        .map(|run| normalized_text(&Content::sequence(run.iter().cloned())))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A tag that is currently open while walking the frames.
struct Open {
    location: Location,
    /// What a nested paragraph is tested against for being part of this one.
    /// `None` for non-paragraphs.
    par: Option<OpenPar>,
    is_artifact: bool,
    is_container: bool,
    list_scope: ListScope,
    /// Glyph runs drawn for this paragraph inside / outside artifact-marked
    /// content. A paragraph all of whose glyphs are drawn inside an artifact
    /// is page furniture, whichever way the two tags happen to nest.
    glyphs_in_artifact: u32,
    glyphs_outside_artifact: u32,
}

/// An open paragraph, as [`covering_par`] needs it.
struct OpenPar {
    /// The paragraph's own source range. `None` when it has no usable span.
    range: Option<SourceRange>,
    /// The paragraph itself, whose text is only extracted when the source
    /// cannot settle the question.
    content: Content,
    /// Its normalized text, extracted at most once.
    text: OnceCell<String>,
    /// Whether it is (part of) a float's body (see [`Float::holds`]).
    float_body: bool,
}

impl OpenPar {
    fn text(&self) -> &str {
        self.text.get_or_init(|| normalized_text(&self.content))
    }
}

fn walk(
    frame: &Frame,
    engine: &Engine,
    floats: &[Float],
    stack: &mut Vec<Open>,
    ancestry: &mut Ancestry,
) {
    for (_, item) in frame.items() {
        match item {
            // Depth-first keeps the tag stream in document order.
            FrameItem::Group(group) => {
                walk(&group.frame, engine, floats, stack, ancestry);
            }
            FrameItem::Tag(Tag::Start(inner, _)) => {
                let Some(location) = inner.location() else { continue };
                let is_par = inner.elem() == ParElem::ELEM;

                let par = is_par.then(|| {
                    let mut par = OpenPar {
                        range: content_range(engine, inner),
                        content: inner.clone(),
                        text: OnceCell::new(),
                        float_body: false,
                    };
                    par.float_body = floats.iter().any(|float| float.holds(&par));
                    par
                });
                if let Some(par) = &par
                    && let Some(parent) = covering_par(stack, par)
                {
                    ancestry.covered_by_par.insert(location, parent);
                }
                if stack.iter().any(|open| open.is_artifact) {
                    ancestry.in_artifact.insert(location);
                }
                if stack.iter().any(|open| open.is_container) {
                    ancestry.in_container.insert(location);
                }
                if stack
                    .iter()
                    .rev()
                    .map(|open| open.list_scope)
                    .find(|scope| *scope != ListScope::Transparent)
                    == Some(ListScope::Carrier)
                {
                    ancestry.list_carried.insert(location);
                }

                stack.push(Open {
                    location,
                    par,
                    is_artifact: inner.elem() == ArtifactElem::ELEM,
                    is_container: is_emitted_container(inner.elem()),
                    list_scope: list_scope(inner),
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
                        if open.par.is_some()
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
                for open in stack.iter_mut().filter(|open| open.par.is_some()) {
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

/// The paragraph `par` is part of, if any.
///
/// *Nesting in the laid-out frames* — the nearest enclosing open paragraph —
/// is what identifies an inline `box`: its body is realized into a paragraph
/// of its own, drawn inside the line of the paragraph that contains it.
/// Nesting alone is not enough, though: `#place(float: true)[…]` written
/// inside a paragraph is deferred to wherever it fits, and its tags can land
/// inside a *later* paragraph that is still open — taking that for nesting
/// would delete the float, which is content loss rather than duplication.
/// So a second signal must confirm it, and which one depends on where the
/// nested paragraph's spans come from.
///
/// *Source containment*, when both paragraphs have a range in the same file.
/// It is deliberately preferred over a text comparison: the box body is
/// realized and the enclosing paragraph's copy of it is not, so anything
/// that only acquires text when realized — a `@reference`'s "Section 1", a
/// citation's marker, a footnote's ordinal — appears in one and not the
/// other. A float body's range is never inside the range of the paragraph it
/// lands in: a block-level `place` ends the paragraph it was written in, so
/// the paragraph that ends up enclosing it in the frames always begins after
/// it.
///
/// *Text occurrence* otherwise — when the nested body's spans are detached
/// (a string passed through `text`, `upper`, `strong`…) or come from another
/// file (a helper imported from a template). The source says nothing about
/// such a body, so it counts as part of the enclosing paragraph only if its
/// text occurs in that one paragraph's text. A body that does not — a
/// reference resolved inside a helper, a package's drawing whose labels are
/// not in any paragraph of the author's — is kept as a paragraph of its own:
/// a duplicate costs less than a loss. The text must occur as words in the
/// enclosing paragraph's, so that a floated "Draft" is not found inside
/// "Drafting".
///
/// A float settles it before the text is compared. Its tags open and close
/// where it is written, but its body is drawn wherever it fits — possibly
/// inside a later paragraph that spans the page, and that may well repeat
/// the float's sentence. Nothing in the frames around the body says it is a
/// float, so a paragraph that is a float's body — by its source range, or by
/// its words when it has no range (see [`floats`]) — is never taken for a box
/// in another paragraph, whichever signal would otherwise apply; at worst a
/// box that happens to say the same as a float is kept twice.
fn covering_par(stack: &[Open], par: &OpenPar) -> Option<Location> {
    let nearest = stack.iter().rev().find(|open| open.par.is_some())?;
    let parent = nearest.par.as_ref()?;
    // A float body is not a box of the paragraph it lands in. A box laid out
    // inside a float body is part of that float too, so when the enclosing
    // paragraph is a float body as well, the usual signals decide.
    if par.float_body && !parent.float_body {
        return None;
    }
    let covered = match (parent.range, par.range) {
        (Some(outer), Some(inner)) if outer.file == inner.file => contains(outer, inner),
        _ => occurs_as_words(parent.text(), par.text()),
    };
    covered.then_some(nearest.location)
}

/// Whether `needle` occurs in `haystack` with no letter or digit glued to
/// either end — at whitespace, punctuation, or the ends of the text.
fn occurs_as_words(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        !before.is_some_and(char::is_alphanumeric)
            && !after.is_some_and(char::is_alphanumeric)
    })
}

fn contains(outer: SourceRange, inner: SourceRange) -> bool {
    outer.file == inner.file && inner.start >= outer.start && inner.end <= outer.end
}

/// A paragraph's text with its whitespace runs collapsed, so that a line
/// break inside the box and a space in the enclosing line compare equal.
fn normalized_text(content: &Content) -> String {
    extract::extract_text(content)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
