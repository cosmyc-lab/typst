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
//! The same walk finds each element's *slot*: the body of the innermost
//! container it is laid out in (a list item, a term's description, a table
//! cell, a quote, a figure, a footnote entry), see [`Ancestry::slot_of`].
//! The tags that open them:
//!
//! - a list or enum item's body: `PdfMarkerTag` of kind `ListItemBody`. The
//!   item elements' own tags are no help: the first item's wraps the whole
//!   list, the others are empty. For a tight item whose body is a single
//!   paragraph the nesting is inverted (`par > ListItemBody > text`): that
//!   paragraph is the item's.
//! - a term's description: `PdfMarkerTag` of kind `TermsItemBody`, which also
//!   holds the term (`TermsItemLabel`) and the separator (an artifact);
//! - a table or grid cell: the `TableCell` / `GridCell` tag, which carries
//!   the cell's resolved `x` and `y`. A header or footer that repeats on
//!   each page is tagged again, with new locations: only the first copy of
//!   each position is the cell's slot, so the repeated copies' content has
//!   no owner and is dropped. A row laid out across several pages (a cell
//!   too tall for one) is drawn differently: each cell's tags open and close
//!   with nothing between them, and the cell's content follows them, laid
//!   out in the table itself, on every page the row spans. That content
//!   belongs to the row's cell whose column it is drawn in. Only a body
//!   row counts: a repeated copy of a header or footer cell, or the first
//!   copy of a footer cell, drawn between two pages of the row, is never
//!   taken for it even when it holds nothing;
//! - a block quote, a figure, a footnote entry: the element's own tag. An
//!   inline quote opens no slot: it is text of the paragraph around it. A
//!   footnote entry is printed at the bottom of the page; its number is a
//!   `Label` marker inside its first paragraph, which that paragraph's text
//!   leaves out (see `extract`). A note is drawn in a frame of its own,
//!   whose logical parent is the footnote; a note too long for its page
//!   continues in another such frame on the next page. What is open in a
//!   note's frames is the note's own, kept across pages apart from the
//!   flow (see [`NoteStacks`]);
//! - an admonition's body: no element. The authoring package brackets it
//!   between two `metadata` markers (see `emit::admonition`), whose tags
//!   open and close back to back, so the slot is a *bracket*: it opens at
//!   the start marker and stays open, across pages, until the end marker.
//!   Brackets nest like any slot, and a container that closes closes the
//!   brackets opened inside it. The printed title is absorbed: the node
//!   carries it.
//!
//! [`Feature::CndSemantics`]: typst_library::Feature::CndSemantics

use std::cell::OnceCell;
use std::ops::ControlFlow;

use ecow::EcoString;
use rustc_hash::{FxHashMap, FxHashSet};
use typst_library::WorldExt;
use typst_library::engine::Engine;
use typst_library::foundations::{
    Content, NativeElement, SequenceElem, Smart, StyleChain,
};
use typst_library::introspection::{Introspector, Location, Tag};
use typst_library::layout::{Frame, FrameItem, GridCell, GridElem, GroupItem, PlaceElem};
use typst_library::math::EquationElem;
use typst_library::model::{
    ArtifactElem, BibliographyElem, EnumElem, FigureCaption, FigureElem, FootnoteEntry,
    HeadingElem, ListElem, OutlineElem, ParElem, ParbreakElem, PdfMarkerTag,
    PdfMarkerTagKind, QuoteElem, TableCell, TableCellKind, TableElem, TermsElem,
};
use typst_library::text::{Locale, RawElem};
use typst_library::visualize::ImageElem;
use typst_syntax::{FileId, Span};

use crate::emit::{admonition, extract};

/// The body of a container, which holds the nodes laid out in it. Each is
/// keyed by the location of the tag that opens it (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKey {
    /// A list or enum item's body (`ListItemBody`).
    Item(Location),
    /// A term's description (`TermsItemBody`).
    TermDescription(Location),
    /// A table or grid cell.
    Cell(Location),
    /// A quote's body.
    QuoteBody(Location),
    /// A figure's body.
    FigureBody(Location),
    /// A footnote's entry, printed at the bottom of the page.
    FootnoteBody(Location),
    /// An admonition's body, bracketed by the start marker at this location
    /// and the end marker after it.
    AdmonitionBody(Location),
}

/// What kind of container a [`SlotKey`] is the body of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotKind {
    Item,
    TermDescription,
    Cell,
    QuoteBody,
    FigureBody,
    FootnoteBody,
    AdmonitionBody,
}

impl SlotKey {
    pub fn kind(&self) -> SlotKind {
        match self {
            Self::Item(_) => SlotKind::Item,
            Self::TermDescription(_) => SlotKind::TermDescription,
            Self::Cell(_) => SlotKind::Cell,
            Self::QuoteBody(_) => SlotKind::QuoteBody,
            Self::FigureBody(_) => SlotKind::FigureBody,
            Self::FootnoteBody(_) => SlotKind::FootnoteBody,
            Self::AdmonitionBody(_) => SlotKind::AdmonitionBody,
        }
    }

    /// The location of the tag that opens this slot.
    pub fn location(&self) -> Location {
        match self {
            Self::Item(location)
            | Self::TermDescription(location)
            | Self::Cell(location)
            | Self::QuoteBody(location)
            | Self::FigureBody(location)
            | Self::FootnoteBody(location)
            | Self::AdmonitionBody(location) => *location,
        }
    }

    /// The slot that the tag `inner` opens, if any.
    fn opened_by(inner: &Content, location: Location) -> Option<Self> {
        let elem = inner.elem();
        if let Some(marker) = inner.to_packed::<PdfMarkerTag>() {
            match marker.kind {
                PdfMarkerTagKind::ListItemBody => Some(Self::Item(location)),
                PdfMarkerTagKind::TermsItemBody => Some(Self::TermDescription(location)),
                _ => None,
            }
        } else if elem == TableCell::ELEM || elem == GridCell::ELEM {
            Some(Self::Cell(location))
        } else if let Some(quote) = inner.to_packed::<QuoteElem>() {
            // Only a block quote is a node. An inline one is text of its
            // paragraph: what is laid out in it stays in the slot around it.
            // The element is materialized, so `block` holds its realized
            // value (see `extract::is_excluded_scope`).
            quote
                .block
                .get(StyleChain::default())
                .then_some(Self::QuoteBody(location))
        } else if elem == FigureElem::ELEM {
            Some(Self::FigureBody(location))
        } else if elem == FootnoteEntry::ELEM {
            Some(Self::FootnoteBody(location))
        } else {
            None
        }
    }
}

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
    /// Each element's innermost slot, see [`Ancestry::slot_of`].
    slots: FxHashMap<Location, SlotKey>,
    /// Each list's, enum's or term list's item slots, in order.
    item_slots: FxHashMap<Location, Vec<SlotKey>>,
    /// Each table's or grid's cell slots, by the cell's `(x, y)` position.
    cell_slots: FxHashMap<Location, FxHashMap<(usize, usize), SlotKey>>,
    /// While walking: each table's or grid's last row whose cells' tags
    /// closed with nothing between them — a row laid out across pages —
    /// with those cells and their locations (see [`innermost_slot`]).
    fragment_cells: FxHashMap<Location, (usize, Vec<(SlotKey, Location)>)>,
    /// Elements laid out in such a row, with the row's cells: which one
    /// holds each is settled by position once the walk is done.
    pending_fragments: Vec<(Location, Vec<(SlotKey, Location)>)>,
    /// Elements laid out inside something that already carries their text,
    /// see [`Ancestry::is_absorbed`].
    absorbed: FxHashSet<Location>,
    /// Every grid, in layout order (see [`Ancestry::grids`]).
    grids: Vec<(Location, Content)>,
    /// The locations in `grids`.
    grid_locations: FxHashSet<Location>,
    /// Each grid's anchor (see [`Ancestry::grids`]).
    grid_anchors: FxHashMap<Location, Location>,
    /// The figures whose body has an element laid out in it that may become
    /// a node (see [`Ancestry::is_drawing`]).
    figures_with_nodes: FxHashSet<Location>,
    /// The figures whose body has such an element inside a `place` that
    /// does not float, written in the body: a positioned drawing.
    figures_with_placed_nodes: FxHashSet<Location>,
    /// The figures whose body holds decorative content (an artifact): such
    /// a body is never kept as `raw`, which would carry its words.
    figures_with_artifacts: FxHashSet<Location>,
    /// While walking: the grids laid out in a figure's body that have no
    /// anchor yet, with that figure and whether they sit in a `place`.
    grids_in_figures: FxHashMap<Location, (Location, bool)>,
    /// The footnotes that have a printed entry, by the footnote's location.
    printed_notes: FxHashSet<Location>,
    /// What is open in each note's frames (see [`NoteStacks`]).
    note_stacks: NoteStacks,
    /// For each element made of text (see [`is_text_node`]), the locales
    /// its glyphs are set in, with how many glyphs each, in the order met.
    locales: FxHashMap<Location, Vec<(Locale, usize)>>,
    /// The scope a label text is laid out in, by the container that owns
    /// it: a figure's caption by the figure, a term by its description's
    /// slot (see [`Ancestry::label_lang_of`]).
    label_scopes: FxHashMap<Location, Location>,
    /// The start markers that an end marker closed: the admonitions.
    admonitions: FxHashSet<Location>,
    /// The start markers laid out inside a paragraph, with that paragraph.
    admonitions_in_par: FxHashMap<Location, Location>,
    /// The start markers whose printed title an enclosing paragraph
    /// renders.
    covered_titles: FxHashSet<Location>,
}

impl Ancestry {
    /// Reconstruct ancestry from a laid-out document's frames.
    pub fn from_document(paged: &typst_layout::PagedDocument, engine: &Engine) -> Self {
        let mut ancestry = Self::default();
        let mut found = Vec::new();
        // Tags stay open across pages, as in `walk`.
        let mut open = Vec::new();
        for page in paged.pages() {
            floats(&page.frame, engine, &mut open, &mut found);
        }
        let floats = Floats::new(found);
        let places = FloatPlaces::of(paged);
        for elem in paged.introspector().query(&FootnoteEntry::ELEM.select()) {
            let Some(entry) = elem.to_packed::<FootnoteEntry>() else { continue };
            if let Some(note) = entry.note.location() {
                ancestry.printed_notes.insert(note);
            }
        }
        let mut stack: Vec<Open> = Vec::new();
        for page in paged.pages() {
            walk(
                &page.frame,
                engine,
                &floats,
                &places,
                DrawScope::FLOW,
                &mut stack,
                &mut ancestry,
            );
        }
        ancestry.settle_fragments(paged.introspector().as_ref());
        ancestry.settle_brackets();
        ancestry
    }

    /// Give what was laid out after a start marker that no end marker
    /// closed — one written by hand, or cut off by the container it was
    /// written in — the slot that marker itself sits in: such a marker
    /// brackets nothing (see [`Ancestry::is_admonition`]).
    fn settle_brackets(&mut self) {
        let unpaired = |slot: &SlotKey| {
            slot.kind() == SlotKind::AdmonitionBody
                && !self.admonitions.contains(&slot.location())
        };
        let stray: Vec<(Location, SlotKey)> = self
            .slots
            .iter()
            .filter(|(_, slot)| unpaired(slot))
            .map(|(location, slot)| (*location, *slot))
            .collect();
        // Each is settled against the slots as the walk left them, so the
        // order they come in does not matter.
        let mut settled = Vec::with_capacity(stray.len());
        for (location, mut slot) in stray {
            // Up through every unpaired marker it sits in; markers are
            // finitely nested, the bound only guards a cycle.
            let mut outer = None;
            for _ in 0..64 {
                match self.slots.get(&slot.location()) {
                    Some(next) if unpaired(next) => slot = *next,
                    found => {
                        outer = found.copied();
                        break;
                    }
                }
            }
            settled.push((location, outer));
        }
        for (location, outer) in settled {
            match outer {
                Some(slot) => self.slots.insert(location, slot),
                None => self.slots.remove(&location),
            };
        }
    }

    /// Whether the start marker at `location` opens an admonition: an end
    /// marker closed it.
    pub fn is_admonition(&self, location: Location) -> bool {
        self.admonitions.contains(&location)
    }

    /// Whether the admonition at `location` is laid out in an inline `box`
    /// whose paragraph renders it, title included: its start marker sits
    /// in a paragraph, and its printed title, if any, is part of one. Its
    /// body then routes nothing (see `convert`).
    pub fn is_boxed_admonition(&self, location: Location, has_title: bool) -> bool {
        self.admonitions_in_par.contains_key(&location)
            && (!has_title || self.covered_titles.contains(&location))
    }

    /// Give each element laid out in a row that spans pages the cell whose
    /// column it is drawn in: the cell starting furthest right at or before
    /// the element. A grid, which the introspector does not place, is
    /// placed by its anchor (see [`Ancestry::grids`]).
    fn settle_fragments(&mut self, introspector: &dyn Introspector) {
        let x = |location: Location| {
            introspector
                .position(location)
                .map(|position| position.as_paged_or_default().point.x)
        };
        for (location, cells) in std::mem::take(&mut self.pending_fragments) {
            // A slot set after the fragment was queued is not a cell's: a
            // tight item's paragraph belongs to its item, whatever column
            // the item is drawn in.
            let still_cell = self
                .slots
                .get(&location)
                .is_some_and(|slot| cells.iter().any(|(cell, _)| cell == slot));
            if !still_cell {
                continue;
            }
            let Some(own) =
                x(self.grid_anchors.get(&location).copied().unwrap_or(location))
            else {
                continue;
            };
            let column = cells
                .iter()
                .filter_map(|(slot, cell)| Some((*slot, x(*cell)?)))
                .filter(|(_, start)| start.to_pt() <= own.to_pt() + 0.01)
                .max_by(|(_, a), (_, b)| a.to_pt().total_cmp(&b.to_pt()));
            if let Some((slot, _)) = column {
                self.slots.insert(location, slot);
            }
        }
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

    /// Every location inside a PDF artifact.
    pub fn artifact_locations(&self) -> &FxHashSet<Location> {
        &self.in_artifact
    }

    /// The innermost slot enclosing the element at `location`: the body of
    /// the container whose node holds it.
    ///
    /// Only a container the element is written in counts (see
    /// [`Open::encloses`]): a float drawn inside a container it is not
    /// written in does not join it. A float's body drawn before the slot it
    /// is written in opens — on an earlier page, while the list is open
    /// around it — takes the slot open where its `place` is written.
    pub fn slot_of(&self, location: Location) -> Option<SlotKey> {
        self.slots.get(&location).copied()
    }

    /// The item slots of the list, enum or term list at `location`, in the
    /// order its items are laid out.
    pub fn item_slots(&self, location: Location) -> &[SlotKey] {
        self.item_slots.get(&location).map_or(&[], Vec::as_slice)
    }

    /// The slot of the cell at column `x`, row `y` of the table or grid at
    /// `owner`: the first laid-out copy of that cell.
    pub fn cell_slot(&self, owner: Location, x: usize, y: usize) -> Option<SlotKey> {
        self.cell_slots.get(&owner)?.get(&(x, y)).copied()
    }

    /// How many distinct cells of the table or grid at `owner` were laid
    /// out.
    pub fn cell_slot_count(&self, owner: Location) -> usize {
        self.cell_slots.get(&owner).map_or(0, FxHashMap::len)
    }

    /// Every grid of the document, in layout order, with its anchor: the
    /// first element laid out inside it that the introspector knows and
    /// that may become a node (see [`is_node_kind`]).
    ///
    /// A grid is not `Locatable`, so unless it is labelled the introspector
    /// does not know it: it gives the grid no page, no state and no place
    /// in reading order. Its anchor stands in for it there. A grid whose
    /// anchor is `None` holds nothing that becomes a node.
    pub fn grids(&self) -> impl Iterator<Item = (Location, &Content, Option<Location>)> {
        self.grids.iter().map(|(location, grid)| {
            (*location, grid, self.grid_anchors.get(location).copied())
        })
    }

    /// Whether the element at `location` is laid out inside something that
    /// carries its text already, or that carries none: a list item's marker
    /// (`ListItemLabel`), a term (`TermsItemLabel`), a footnote's number (a
    /// `Label` marker), a heading, a code block, an equation, the outline,
    /// the bibliography, or an artifact. A paragraph found there is never a
    /// node.
    pub fn is_absorbed(&self, location: Location) -> bool {
        self.absorbed.contains(&location) || self.is_artifact(location)
    }

    /// Note that an element that may become a node is laid out in the body
    /// of the figure at `figure`, inside a `place` that does not float,
    /// written in the body, when `placed`.
    fn note_figure_node(&mut self, figure: Location, placed: bool) {
        self.figures_with_nodes.insert(figure);
        if placed {
            self.figures_with_placed_nodes.insert(figure);
        }
    }

    /// The language of the text the element at `location` lays out: the
    /// locale most of its glyphs are set in, the first met on a tie. `None`
    /// for an element that lays out no glyph, or that is not made of text.
    ///
    /// Realization keeps no style chain with a paragraph, so the locale is
    /// read where layout recorded it, on each run of glyphs.
    ///
    /// This differs on purpose from the PDF export's tagging, where a
    /// group takes the language of its first child. A CND text has one
    /// `lang` for the whole of it, and a foreign phrase inside it is not
    /// modeled (spec §4.1): the language most of the text is in describes
    /// it, where the first run would let a paragraph opening on a quoted
    /// foreign word take that word's language. The tie-break keeps the
    /// result deterministic.
    pub fn lang_of(&self, location: Location) -> Option<EcoString> {
        let mut best: Option<(Locale, usize)> = None;
        for &(locale, count) in self.locales.get(&location)? {
            if best.is_none_or(|(_, most)| count > most) {
                best = Some((locale, count));
            }
        }
        best.map(|(locale, _)| locale.rfc_3066())
    }

    /// The language of the label text owned by `owner` (see
    /// [`Ancestry::lang_of`]): the caption of the figure at `owner`, or the
    /// term of the description slot at `owner`.
    pub fn label_lang_of(&self, owner: Location) -> Option<EcoString> {
        self.lang_of(*self.label_scopes.get(&owner)?)
    }

    /// Whether the figure at `figure` is a drawing (cnd-sdk spec §6.7): its
    /// body lays out no element that may become a node (a `rect`, a line, a
    /// canvas of shapes; an empty paragraph does not count), or an element
    /// laid out in it sits inside a `place` that does not float, written in
    /// the body — a diagram, or a helper that positions labels. Its content
    /// is then its source, not the scattered pieces lifted out of it.
    ///
    /// A body holding decorative content and no node is not a drawing: its
    /// source would carry the decoration's words.
    pub fn is_drawing(&self, figure: Location) -> bool {
        self.figures_with_placed_nodes.contains(&figure)
            || (!self.figures_with_nodes.contains(&figure)
                && !self.figures_with_artifacts.contains(&figure))
    }
}

/// Whether a paragraph laid out inside `inner` is absorbed (see
/// [`Ancestry::is_absorbed`]). Artifacts are left to [`DrawScope`]: what is
/// open around a float's body has nothing to do with it.
fn absorbs(inner: &Content) -> bool {
    if let Some(marker) = inner.to_packed::<PdfMarkerTag>() {
        return matches!(
            marker.kind,
            PdfMarkerTagKind::ListItemLabel
                | PdfMarkerTagKind::TermsItemLabel
                | PdfMarkerTagKind::Label
        );
    }
    let elem = inner.elem();
    // An admonition's printed title, which its node carries.
    admonition::is_title(inner)
        || elem == HeadingElem::ELEM
        || elem == RawElem::ELEM
        || elem == EquationElem::ELEM
        || elem == OutlineElem::ELEM
        || elem == BibliographyElem::ELEM
        // A figure's caption, which the figure node carries.
        || elem == FigureCaption::ELEM
}

/// Whether `inner` owns item slots: a list, an enum or a term list.
fn owns_items(inner: &Content) -> bool {
    let elem = inner.elem();
    elem == ListElem::ELEM || elem == EnumElem::ELEM || elem == TermsElem::ELEM
}

/// Whether `inner` is of a kind that the emit pipeline may turn into a node
/// (the elements `convert` queries, and an admonition's start marker).
fn is_node_kind(inner: &Content) -> bool {
    let elem = inner.elem();
    admonition::marker(inner) == Some(admonition::Marker::Start)
        || elem == ParElem::ELEM
        || elem == HeadingElem::ELEM
        || elem == TableElem::ELEM
        || elem == FigureElem::ELEM
        || elem == QuoteElem::ELEM
        || elem == RawElem::ELEM
        || elem == EquationElem::ELEM
        || elem == ListElem::ELEM
        || elem == EnumElem::ELEM
        || elem == TermsElem::ELEM
        || elem == ImageElem::ELEM
}

/// Whether `inner` owns cell slots: a table or a grid.
fn owns_cells(inner: &Content) -> bool {
    let elem = inner.elem();
    elem == TableElem::ELEM || elem == GridElem::ELEM
}

/// The first non-gutter row of a table's or grid's footer, read from the
/// grid the element resolved.
fn footer_start(inner: &Content) -> Option<usize> {
    let grid = if let Some(table) = inner.to_packed::<TableElem>() {
        table.grid.as_ref()?
    } else if let Some(grid) = inner.to_packed::<GridElem>() {
        grid.grid.as_ref()?
    } else {
        return None;
    };
    let footer = grid.footer.as_ref()?;
    // With gutters, rows and gutter rows alternate.
    Some(if grid.has_gutter { footer.start.div_ceil(2) } else { footer.start })
}

/// Whether a table cell says it is a footer cell.
fn is_footer_cell(inner: &Content) -> bool {
    inner.to_packed::<TableCell>().is_some_and(|cell| {
        matches!(
            cell.kind.get(StyleChain::default()),
            Smart::Custom(TableCellKind::Footer)
        )
    })
}

/// The resolved `(x, y)` position of a table or grid cell. The tag holds the
/// materialized cell, whose position the grid resolved.
fn cell_position(inner: &Content) -> Option<(usize, usize)> {
    let styles = StyleChain::default();
    if let Some(cell) = inner.to_packed::<TableCell>() {
        Some((cell.x.get(styles).custom()?, cell.y.get(styles).custom()?))
    } else if let Some(cell) = inner.to_packed::<GridCell>() {
        Some((cell.x.get(styles).custom()?, cell.y.get(styles).custom()?))
    } else {
        None
    }
}

/// Whether `inner` is a `place` that floats. Materialized, so `float` holds
/// its realized value (see `extract::is_excluded_scope`).
fn is_float(inner: &Content) -> bool {
    inner
        .to_packed::<PlaceElem>()
        .is_some_and(|place| place.float.get(StyleChain::default()))
}

/// Whether `inner` is a `place` that does not float: content positioned at
/// a fixed point of its container.
fn is_placed(inner: &Content) -> bool {
    inner
        .to_packed::<PlaceElem>()
        .is_some_and(|place| !place.float.get(StyleChain::default()))
}

/// Whether a `place` that does not float, written in the body of the
/// figure at `figure`, is open deeper on `stack` than the figure itself. A
/// `place` written around the figure — by a show rule
/// (`show figure: it => place(center, it)`), say — is laid out inside it
/// too, but is not written in its body, so it does not count. When the
/// body's source range is unknown, any such `place` counts.
fn is_placed_in(stack: &[Open], figure: Location) -> bool {
    let Some(index) = stack.iter().rposition(|open| open.location == figure) else {
        return false;
    };
    let body = stack[index].body_range;
    stack[index + 1..].iter().any(|open| {
        open.is_placed
            && match (body, open.range) {
                (Some(body), Some(place)) => contains(body, place),
                (Some(_), None) => false,
                (None, _) => true,
            }
    })
}

/// What identifies a float's body once it is drawn away from its tags.
struct Float {
    /// Where the `place` is written (see [`float_range`]). A paragraph whose
    /// own range lies inside it is that float's content, whatever text it
    /// realizes to.
    range: Option<SourceRange>,
    /// What the body is made of (see [`body_spans`]): a paragraph built from
    /// any of it is part of the body, not merely a paragraph that the call
    /// reaches across in the source.
    body: FxHashSet<Span>,
    /// The body's text, for a paragraph whose range says nothing about the
    /// float (a string passed through `upper`, `text`…, a body built from an
    /// earlier `#let`), which only its words can identify.
    text: String,
    /// The elements open where the `place` is written: what it really sits
    /// in, unlike what is open where its body is drawn.
    ancestors: FxHashSet<Location>,
    /// The innermost slot open where the `place` is written.
    slot: Option<SlotKey>,
    /// The `place` itself: the logical parent of the group its body is
    /// drawn in.
    location: Option<Location>,
}

impl Float {
    /// Whether `par` lies inside this float in the source.
    fn holds_by_range(&self, par: &OpenPar) -> bool {
        par.range
            .zip(self.range)
            .is_some_and(|(inner, outer)| contains(outer, inner))
    }

    /// Whether `par` is built from some of this float's body.
    fn holds_in_body(&self, par: &OpenPar) -> bool {
        par.content
            .traverse(&mut |element| {
                if self.body.contains(&element.span()) {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
            .is_break()
    }

    /// Whether `par` says nothing the float's body does not say.
    fn holds_by_text(&self, par: &OpenPar) -> bool {
        // Most paragraphs are longer than any float's text; skipping them
        // here spares setting up a search for each one.
        let text = par.text();
        text.len() <= self.text.len() && occurs_as_words(&self.text, text)
    }
}

/// Collect every float of the document.
///
/// A separate pass, because a float is drawn away from its tags: they open
/// and close where it is written, and its body lands wherever it fits —
/// possibly before them, at the top of the page.
fn floats(
    frame: &Frame,
    engine: &Engine,
    open: &mut Vec<(Location, Option<SlotKey>)>,
    out: &mut Vec<Float>,
) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => floats(&group.frame, engine, open, out),
            FrameItem::Tag(Tag::Start(inner, _)) => {
                if is_float(inner)
                    && let Some(place) = inner.to_packed::<PlaceElem>()
                {
                    out.push(Float {
                        range: float_range(engine, inner),
                        body: body_spans(&place.body),
                        text: paragraphs_text(&place.body),
                        ancestors: open.iter().map(|(location, _)| *location).collect(),
                        slot: open.iter().rev().find_map(|(_, slot)| *slot),
                        location: inner.location(),
                    });
                }
                let Some(location) = inner.location() else { continue };
                // An admonition's bracket, as in `walk`.
                match admonition::marker(inner) {
                    Some(admonition::Marker::Start) => {
                        open.push((location, Some(SlotKey::AdmonitionBody(location))));
                    }
                    Some(admonition::Marker::End) => {
                        if let Some(index) =
                            open.iter().rposition(|(_, slot)| is_bracket(*slot))
                        {
                            open.remove(index);
                        }
                    }
                    None => open.push((location, SlotKey::opened_by(inner, location))),
                }
            }
            // As in `walk`, pop to the matching start; a bracket is closed by
            // its end marker only.
            FrameItem::Tag(Tag::End(location, ..)) => {
                if let Some(index) = open
                    .iter()
                    .rposition(|(open, slot)| open == location && !is_bracket(*slot))
                {
                    open.truncate(index);
                }
            }
            _ => {}
        }
    }
}

/// Whether `slot` is an admonition's bracket.
fn is_bracket(slot: Option<SlotKey>) -> bool {
    slot.is_some_and(|slot| slot.kind() == SlotKind::AdmonitionBody)
}

/// The document's floats, indexed so that a paragraph or a tag is only
/// tested against the few floats that can hold it: a long document has
/// thousands of each.
struct Floats {
    all: Vec<Float>,
    /// Which floats' bodies hold each span (see [`body_spans`]). Usually
    /// one; several when the same content is the body of several floats.
    by_span: FxHashMap<Span, Vec<usize>>,
    /// The floats with a source range, per file, sorted by where it starts.
    by_start: FxHashMap<FileId, Vec<RangeEntry>>,
    /// The floats whose text has each word (a maximal run of letters and
    /// digits) in it.
    by_word: FxHashMap<String, Vec<usize>>,
    /// Each float, by its `place`'s location.
    by_location: FxHashMap<Location, usize>,
}

/// A float's source range in [`Floats::by_start`].
struct RangeEntry {
    start: usize,
    end: usize,
    /// The furthest end of this range and of every one sorted before it.
    reach: usize,
    index: usize,
}

impl Floats {
    fn new(all: Vec<Float>) -> Self {
        let mut by_span: FxHashMap<Span, Vec<usize>> = FxHashMap::default();
        let mut by_start: FxHashMap<FileId, Vec<RangeEntry>> = FxHashMap::default();
        let mut by_word: FxHashMap<String, Vec<usize>> = FxHashMap::default();
        for (index, float) in all.iter().enumerate() {
            // Each span's list is filled in float order whatever order the
            // set yields its spans in.
            #[allow(clippy::iter_over_hash_type)]
            for span in &float.body {
                by_span.entry(*span).or_default().push(index);
            }
            if let Some(range) = float.range {
                by_start.entry(range.file).or_default().push(RangeEntry {
                    start: range.start,
                    end: range.end,
                    reach: range.end,
                    index,
                });
            }
            for word in words(&float.text) {
                let floats = by_word.entry(word.to_owned()).or_default();
                if floats.last() != Some(&index) {
                    floats.push(index);
                }
            }
        }
        // Each file's list is sorted on its own, whatever order they come in.
        #[allow(clippy::iter_over_hash_type)]
        for entries in by_start.values_mut() {
            entries.sort_by_key(|entry| entry.start);
            let mut reach = 0;
            for entry in entries.iter_mut() {
                reach = reach.max(entry.end);
                entry.reach = reach;
            }
        }
        let by_location = all
            .iter()
            .enumerate()
            .filter_map(|(index, float)| Some((float.location?, index)))
            .collect();
        Self { all, by_span, by_start, by_word, by_location }
    }

    /// The floats whose body holds the element written at `span`.
    fn holding(&self, span: Span) -> &[usize] {
        self.by_span.get(&span).map_or(&[], Vec::as_slice)
    }

    /// The floats that may hold `par` by its range or by its words (see
    /// [`Float::holds_by_range`], [`Float::holds_by_text`]), sorted; a
    /// superset, which those two tests narrow down.
    fn candidates(&self, par: &OpenPar) -> Vec<usize> {
        let mut out = Vec::new();
        if self.all.is_empty() {
            return out;
        }
        // A float holding the paragraph's range starts at or before it, and
        // one sorted earlier still can only if some range that far back
        // reaches the paragraph's end.
        if let Some(range) = par.range
            && let Some(entries) = self.by_start.get(&range.file)
        {
            let after = entries.partition_point(|entry| entry.start <= range.start);
            for entry in entries[..after].iter().rev() {
                if entry.reach < range.end {
                    break;
                }
                if entry.end >= range.end {
                    out.push(entry.index);
                }
            }
        }
        // Text that occurs as words in a float's text has each of its own
        // words there whole (see `occurs_as_words`), so the floats having
        // its rarest word are enough. Text with no word at all (punctuation
        // alone) can be anywhere.
        let rarest = words(par.text())
            .map(|word| self.by_word.get(word).map_or(&[][..], Vec::as_slice))
            .min_by_key(|floats| floats.len());
        match rarest {
            Some(floats) => out.extend(floats),
            None if !par.text().is_empty() => out.extend(0..self.all.len()),
            None => {}
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// The maximal runs of letters and digits in `text`.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

/// How far a floating `place` reaches in the source: the call itself and
/// whatever its body writes from there on.
///
/// Unlike [`content_range`], this ignores body spans that start before the
/// call — content defined earlier by a `#let` and only used here. Their hull
/// would stretch from that definition to the call and take in every
/// paragraph written in between.
///
/// It does not settle every case. A helper defined in the preamble,
/// `#let aside(x) = place(…)[#x]`, has its `place` span there and its body at
/// the call site, so the range still reaches across the paragraphs written in
/// between, and they count as surely held by this float. That is why a
/// range never makes a paragraph a float's *body* on its own: that also
/// takes its words and its spans (see `OpenPar::body_of`).
fn float_range(engine: &Engine, place: &Content) -> Option<SourceRange> {
    let call = source_range(engine, place.span())?;
    let mut range = call;
    let _ = place.traverse(&mut |element| {
        if let Some(found) = source_range(engine, element.span())
            && found.file == call.file
            && found.start >= call.start
        {
            range.end = range.end.max(found.end);
        }
        ControlFlow::<()>::Continue(())
    });
    Some(range)
}

/// The spans of everything a float's body holds. Unlike a source range,
/// this does not reach across the text written between two of them: a
/// helper defined in the preamble, `#let aside(x) = place(…)[Aside: #x]`,
/// holds spans both there and at its call, and a paragraph written in between
/// shares none of them.
pub(crate) fn body_spans(body: &Content) -> FxHashSet<Span> {
    let mut spans = FxHashSet::default();
    let _ = body.traverse(&mut |element| {
        let span = element.span();
        if !span.is_detached() {
            spans.insert(span);
        }
        ControlFlow::<()>::Continue(())
    });
    spans
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

/// What is open in each printed note's frames, by the footnote's location.
///
/// A note is drawn in a frame of its own at the bottom of the page, while
/// the paragraph holding its marker — and whatever else of the flow — is
/// still open. A note too long for its page continues in another frame on
/// the next page, with no entry tag: its entry, and a list broken with it,
/// opened on the first page. So each note's frames are walked with a stack
/// of their own, kept from one frame to the next: the flow's tags neither
/// enclose the note's content nor close what the note left open, and the
/// rest of a note continues inside its entry.
#[derive(Default)]
struct NoteStacks(FxHashMap<Location, Vec<Open>>);

impl NoteStacks {
    fn take(&mut self, note: Location) -> Vec<Open> {
        self.0.remove(&note).unwrap_or_default()
    }

    fn keep(&mut self, note: Location, stack: Vec<Open>) {
        if !stack.is_empty() {
            self.0.insert(note, stack);
        }
    }
}

impl std::fmt::Debug for NoteStacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoteStacks").field("open", &self.0.len()).finish()
    }
}

/// A tag that is currently open while walking the frames.
struct Open {
    location: Location,
    /// The floats whose body holds this element's own span (see
    /// [`Floats::holding`]). Empty for the document's flow.
    floats: Vec<usize>,
    /// What a nested paragraph is tested against for being part of this one.
    /// `None` for non-paragraphs.
    par: Option<OpenPar>,
    is_artifact: bool,
    /// The slot this element opens, if any.
    slot: Option<SlotKey>,
    /// Whether this is an admonition's bracket (see the module docs): no
    /// end tag of its own closes it, its end marker does.
    bracket: bool,
    /// For an admonition's printed title, the admonition's start marker.
    title_of: Option<Location>,
    /// Whether a paragraph inside it is absorbed (see [`absorbs`]).
    absorbs: bool,
    /// Whether it owns item slots (see [`owns_items`]).
    owns_items: bool,
    /// Whether it owns cell slots (see [`owns_cells`]).
    owns_cells: bool,
    /// For a table or grid, the first row of its footer (see
    /// [`footer_start`]).
    footer_start: Option<usize>,
    /// For a cell, whether it may hold the content of a row laid out across
    /// pages: a body cell, the first copy at its position.
    is_body_cell: bool,
    /// Whether it is a grid, which needs an anchor (see [`Ancestry::grids`]).
    is_grid: bool,
    /// Whether it is a `place` that does not float (see [`is_placed`]).
    is_placed: bool,
    /// For such a `place`, where it is written.
    range: Option<SourceRange>,
    /// For a figure, where its body is written (see [`is_placed_in`]).
    body_range: Option<SourceRange>,
    /// For a paragraph laid out in a figure's body, that figure: whether
    /// the paragraph routes a node is settled once its glyphs are counted.
    figure_par: Option<Location>,
    /// For a cell, the table or grid it is a cell of, and its position.
    cell_owner: Option<Location>,
    cell_position: Option<(usize, usize)>,
    /// Whether some element opened inside it.
    had_child: bool,
    /// Glyph runs drawn for this paragraph inside / outside artifact-marked
    /// content. A paragraph all of whose glyphs are drawn inside an artifact
    /// is page furniture, whichever way the two tags happen to nest.
    glyphs_in_artifact: u32,
    glyphs_outside_artifact: u32,
    /// Whether this element becomes a node made of text alone — a paragraph,
    /// a heading, a list, a term list, a quote — so that drawing all of its
    /// text inside artifacts makes it decorative (see [`is_text_node`]).
    is_text_node: bool,
    /// Whether the locales of the glyphs laid out in it are counted (see
    /// [`Ancestry::lang_of`]): an element made of text, or a label text's
    /// scope (see [`label_owner`]).
    counts_locale: bool,
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
    /// The floats it may be (part of) the body of, as indices into the
    /// document's floats: by its range, or by its words. This is what
    /// shelters it from the paragraph it lands in. Usually empty.
    floats: Vec<usize>,
    /// The floats it is surely part of: by its range when it has one, by its
    /// words only when it has none. A paragraph written elsewhere — in
    /// another file, say — is not a float's body merely because the float
    /// repeats its words.
    floats_by_source: Vec<usize>,
    /// The floats whose body it is, as far as a paragraph laid out inside it
    /// is concerned. A paragraph with a range of its own must lie in the
    /// float's range, be built from some of the body's own content (see
    /// [`body_spans`]), and say nothing the body does not say; one without a
    /// range, only the last. The range alone is not enough: a helper defined
    /// in the preamble reaches across the paragraphs written before its call
    /// (see [`float_range`]), and may well quote one of them.
    body_of: Vec<usize>,
}

impl Open {
    /// Whether this open element can be what an element held by `floats`
    /// was written in.
    ///
    /// A float's tags open and close where it is written, and its body is
    /// drawn wherever it fits — possibly while a list, a quote or a table
    /// written elsewhere spans the page and is still open around it in the
    /// frames. Taking that for nesting would lose the float's list (taken
    /// for nested items) or its paragraphs (taken for the container's own
    /// text). So an element its own span puts in some float's body is only
    /// enclosed by elements of that same body, or by an element the float
    /// itself is written in — a list item holding the `place` renders its
    /// text.
    ///
    /// The test is one-sided. An element whose span says nothing (a body
    /// built by a show rule, detached text) keeps whatever is open around
    /// it, as does everything in the flow: flow content is never drawn
    /// inside a float's body. Content reused both in the flow and in a
    /// float maps to that float, so its flow copy may be emitted on its own
    /// as well — a duplicate, not a loss.
    fn encloses(&self, floats: &Floats, held_by: &[usize]) -> bool {
        held_by.is_empty()
            || held_by.iter().any(|index| {
                self.floats.contains(index)
                    || floats.all[*index].ancestors.contains(&self.location)
            })
    }
}

impl OpenPar {
    fn text(&self) -> &str {
        self.text.get_or_init(|| normalized_text(&self.content))
    }
}

/// The container owning the label text `inner` opens, if it opens one: a
/// figure's caption is owned by the figure, a term by its description's
/// slot, which also holds it, an admonition's printed title by its start
/// marker.
fn label_owner(inner: &Content, stack: &[Open]) -> Option<Location> {
    let owner = |kind: SlotKind| {
        stack
            .iter()
            .rev()
            .find(|open| open.slot.is_some_and(|slot| slot.kind() == kind))
            .map(|open| open.location)
    };
    if inner.elem() == FigureCaption::ELEM {
        return owner(SlotKind::FigureBody);
    }
    if admonition::is_title(inner) {
        return owner(SlotKind::AdmonitionBody);
    }
    let marker = inner.to_packed::<PdfMarkerTag>()?;
    (marker.kind == PdfMarkerTagKind::TermsItemLabel)
        .then(|| owner(SlotKind::TermDescription))
        .flatten()
}

/// Whether `inner` becomes a node made of text alone. When every glyph such
/// an element draws lies inside artifacts — a show rule that wraps a heading
/// in `pdf.artifact`, whose own tags then sit outside the artifact — it is
/// decorative. A figure or a table is left out: it may hold an image whose
/// meaning no glyph count sees.
fn is_text_node(inner: &Content) -> bool {
    let elem = inner.elem();
    elem == HeadingElem::ELEM
        || elem == ListElem::ELEM
        || elem == EnumElem::ELEM
        || elem == TermsElem::ELEM
        || elem == QuoteElem::ELEM
}

/// The document's floating `place`s, by location, and which of them are
/// decorative: written inside a PDF artifact, or inside the body of a
/// decorative float.
struct FloatPlaces {
    all: FxHashSet<Location>,
    decorative: FxHashSet<Location>,
}

impl FloatPlaces {
    fn of(paged: &typst_layout::PagedDocument) -> Self {
        let all: FxHashSet<Location> = paged
            .introspector()
            .query(&PlaceElem::ELEM.select())
            .iter()
            .filter(|place| is_float(place))
            .filter_map(|place| place.location())
            .collect();
        if all.is_empty() {
            return Self { all, decorative: FxHashSet::default() };
        }

        // Where each float is written: inside an artifact opened in its own
        // body scope, and inside which float's body, if any.
        let mut written: FxHashMap<Location, (bool, Option<Location>)> =
            FxHashMap::default();
        let mut open: Vec<(Location, bool)> = Vec::new();
        for page in paged.pages() {
            Self::scan(&page.frame, &all, 0, None, &mut open, &mut written);
        }

        fn decorative_place(
            place: Location,
            written: &FxHashMap<Location, (bool, Option<Location>)>,
            depth: usize,
        ) -> bool {
            match written.get(&place) {
                Some((true, _)) => true,
                Some((false, Some(outer))) if depth < 64 => {
                    decorative_place(*outer, written, depth + 1)
                }
                _ => false,
            }
        }
        let decorative = all
            .iter()
            .copied()
            .filter(|place| decorative_place(*place, &written, 0))
            .collect();
        Self { all, decorative }
    }

    fn scan(
        frame: &Frame,
        all: &FxHashSet<Location>,
        base: usize,
        body_of: Option<Location>,
        open: &mut Vec<(Location, bool)>,
        written: &mut FxHashMap<Location, (bool, Option<Location>)>,
    ) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Group(group) => match group.parent {
                    Some(parent) if all.contains(&parent.location) => Self::scan(
                        &group.frame,
                        all,
                        open.len(),
                        Some(parent.location),
                        open,
                        written,
                    ),
                    _ => Self::scan(&group.frame, all, base, body_of, open, written),
                },
                FrameItem::Tag(Tag::Start(inner, _)) => {
                    let Some(location) = inner.location() else { continue };
                    if all.contains(&location) {
                        let in_artifact = open
                            .get(base.min(open.len())..)
                            .is_some_and(|open| open.iter().any(|(_, art)| *art));
                        written.insert(location, (in_artifact, body_of));
                    }
                    open.push((location, inner.elem() == ArtifactElem::ELEM));
                }
                FrameItem::Tag(Tag::End(location, ..)) => {
                    if let Some(index) =
                        open.iter().rposition(|(open, _)| open == location)
                    {
                        open.truncate(index);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Where frame content is drawn, as far as decoration goes (cnd-sdk spec §2,
/// "Decorative content").
///
/// A float's body is not drawn between its `place` tags but in a group of its
/// own, at the top or bottom of the page, which layout marks with the float
/// as its logical parent (`FrameParent`). Inside that group, what is open
/// on the stack from the page's flow — a long artifact still open across the
/// page break, say — has nothing to do with the body: the body is decorative
/// exactly when its `place` was written inside an artifact (see
/// [`FloatPlaces`]), or when an artifact opens inside the body itself.
#[derive(Debug, Clone, Copy)]
struct DrawScope {
    /// Inside the body of a float written inside an artifact.
    decorative: bool,
    /// The stack depth where the current float body starts; `0` in the flow.
    base: usize,
    /// The float whose body this is, as an index into the document's
    /// floats; `None` in the flow.
    float: Option<usize>,
}

impl DrawScope {
    const FLOW: Self = Self { decorative: false, base: 0, float: None };

    /// The scope of a group's content: a float's body starts afresh.
    fn enter(
        self,
        group: &GroupItem,
        places: &FloatPlaces,
        floats: &Floats,
        depth: usize,
    ) -> Self {
        match group.parent {
            Some(parent) if places.all.contains(&parent.location) => Self {
                decorative: places.decorative.contains(&parent.location),
                base: depth,
                float: floats.by_location.get(&parent.location).copied(),
            },
            _ => self,
        }
    }

    /// Whether what is drawn here, with `stack` open, is decorative.
    fn in_artifact(self, stack: &[Open]) -> bool {
        self.decorative
            || stack
                .get(self.base.min(stack.len())..)
                .is_some_and(|open| open.iter().any(|open| open.is_artifact))
    }
}
fn walk(
    frame: &Frame,
    engine: &Engine,
    floats: &Floats,
    places: &FloatPlaces,
    scope: DrawScope,
    stack: &mut Vec<Open>,
    ancestry: &mut Ancestry,
) {
    for (_, item) in frame.items() {
        match item {
            // Depth-first keeps the tag stream in document order.
            FrameItem::Group(group) => {
                let scope = scope.enter(group, places, floats, stack.len());
                match group.parent {
                    // A note's frame: what is open around it is the flow's,
                    // not the note's (see `NoteStacks`).
                    Some(parent) if ancestry.printed_notes.contains(&parent.location) => {
                        let mut own = ancestry.note_stacks.take(parent.location);
                        let scope = DrawScope {
                            decorative: scope.decorative,
                            base: 0,
                            float: None,
                        };
                        walk(
                            &group.frame,
                            engine,
                            floats,
                            places,
                            scope,
                            &mut own,
                            ancestry,
                        );
                        ancestry.note_stacks.keep(parent.location, own);
                    }
                    _ => {
                        walk(
                            &group.frame,
                            engine,
                            floats,
                            places,
                            scope,
                            stack,
                            ancestry,
                        );
                    }
                }
            }
            FrameItem::Tag(Tag::Start(inner, flags)) => {
                let Some(location) = inner.location() else { continue };
                let is_par = inner.elem() == ParElem::ELEM;
                if inner.elem() == GridElem::ELEM
                    && ancestry.grid_locations.insert(location)
                {
                    ancestry.grids.push((location, inner.clone()));
                }
                if flags.introspectable && is_node_kind(inner) {
                    for open in stack.iter().filter(|open| open.is_grid) {
                        ancestry.grid_anchors.entry(open.location).or_insert(location);
                        // A grid becomes a node once it has an anchor.
                        if let Some((figure, placed)) =
                            ancestry.grids_in_figures.remove(&open.location)
                        {
                            ancestry.note_figure_node(figure, placed);
                        }
                    }
                }
                let bracket = admonition::marker(inner);
                if bracket == Some(admonition::Marker::End)
                    && let Some(start) = close_bracket(stack)
                {
                    ancestry.admonitions.insert(start);
                }
                // An admonition laid out inside a paragraph: in an inline
                // `box`, whose paragraph may render all of it.
                if bracket == Some(admonition::Marker::Start)
                    && let Some(par) = stack.get(scope.base..).and_then(|open| {
                        open.iter().rev().find(|open| open.par.is_some())
                    })
                {
                    ancestry.admonitions_in_par.insert(location, par.location);
                }
                let opens = match bracket {
                    Some(admonition::Marker::Start) => {
                        Some(SlotKey::AdmonitionBody(location))
                    }
                    _ => SlotKey::opened_by(inner, location),
                };
                let mut in_floats = floats.holding(inner.span()).to_vec();
                // An admonition marker's span is in the authoring package,
                // the same for every call: like a layout marker, it sits
                // where its parent does — or, first in a float's body, where
                // nothing of the body is open yet, in that float.
                if bracket.is_some() {
                    in_floats = match scope.float {
                        Some(float) if stack.len() <= scope.base => vec![float],
                        _ => stack
                            .get(scope.base..)
                            .and_then(<[Open]>::last)
                            .map(|open| open.floats.clone())
                            .unwrap_or_default(),
                    };
                }
                // A marker is made by layout, often with no span of its own:
                // it sits wherever what made it does. An item's body sits
                // where its list does, any other marker where its parent does.
                if inner.is::<PdfMarkerTag>() && in_floats.is_empty() {
                    let maker = match opens {
                        Some(SlotKey::Item(_) | SlotKey::TermDescription(_)) => {
                            stack.iter().rev().find(|open| open.owns_items)
                        }
                        _ => stack.last(),
                    };
                    if let Some(maker) = maker {
                        in_floats = maker.floats.clone();
                    }
                }

                let par = is_par.then(|| open_par(engine, floats, inner));
                if let Some(par) = &par
                    && let Some(parent) = covering_par(stack, par)
                {
                    ancestry.covered_by_par.insert(location, parent);
                    // An admonition's printed title that an enclosing
                    // paragraph renders.
                    if let Some(admonition) =
                        stack.iter().rev().find_map(|open| open.title_of)
                    {
                        ancestry.covered_titles.insert(admonition);
                    }
                }
                // A list, a table, a grid or a block quote in an inline
                // `box` is laid out inside the line of the paragraph that
                // holds it, which renders its text: the same test as for a
                // box's paragraph. A block quote whose own show rule boxes
                // it (`show quote: it => box(it.body)`) is not: the
                // paragraph holds the quote's tags, and its text leaves the
                // quote out (see `extract::is_excluded_scope`).
                let is_quote = matches!(opens, Some(SlotKey::QuoteBody(_)));
                if (owns_items(inner) || owns_cells(inner) || is_quote)
                    && stack.iter().any(|open| open.par.is_some())
                    && let Some(parent) =
                        covering_par(stack, &open_par(engine, floats, inner))
                    && !(is_quote && holds_tag_of(stack, parent, location))
                {
                    ancestry.covered_by_par.insert(location, parent);
                }
                if scope.in_artifact(stack) {
                    ancestry.in_artifact.insert(location);
                }
                if stack
                    .iter()
                    .any(|open| open.absorbs && open.encloses(floats, &in_floats))
                {
                    ancestry.absorbed.insert(location);
                }
                let mut figure_par = None;
                match innermost_slot(stack, floats, &in_floats, location) {
                    Some(Found::Slot(slot)) => {
                        ancestry.slots.insert(location, slot);
                        if let SlotKey::FigureBody(figure) = slot {
                            if ancestry.in_artifact.contains(&location) {
                                ancestry.figures_with_artifacts.insert(figure);
                            } else if !ancestry.absorbed.contains(&location) {
                                let placed = is_placed_in(stack, figure);
                                let node = flags.introspectable && is_node_kind(inner);
                                let covered =
                                    ancestry.covered_by_par.contains_key(&location);
                                // A label placed in a box is part of the
                                // box's paragraph, yet still a positioned
                                // piece of a drawing.
                                if node && placed {
                                    ancestry.figures_with_placed_nodes.insert(figure);
                                }
                                if covered {
                                    // Its text is the covering paragraph's.
                                } else if is_par {
                                    // Settled when it ends: an empty one
                                    // routes nothing.
                                    figure_par = Some(figure);
                                } else if node {
                                    ancestry.note_figure_node(figure, placed);
                                } else if inner.elem() == GridElem::ELEM {
                                    // Settled when the grid finds its anchor.
                                    ancestry
                                        .grids_in_figures
                                        .insert(location, (figure, placed));
                                }
                            }
                        }
                    }
                    Some(Found::InTable(owner)) => {
                        if let Some((_, cells)) = ancestry.fragment_cells.get(&owner)
                            && let Some((slot, _)) = cells.last()
                        {
                            ancestry.slots.insert(location, *slot);
                            if cells.len() > 1 {
                                ancestry
                                    .pending_fragments
                                    .push((location, cells.clone()));
                            }
                        }
                    }
                    None => {}
                }

                let slot = opens;
                let cell_owner = match slot {
                    Some(SlotKey::Cell(_)) => stack
                        .iter()
                        .rev()
                        .find(|open| open.owns_cells)
                        .map(|open| open.location),
                    _ => None,
                };
                if let Some(slot) = slot
                    && let Some(owner) = cell_owner
                    && let Some(position) = cell_position(inner)
                {
                    // The first copy of a position is the cell; a repeated
                    // header's or footer's copies are not.
                    ancestry
                        .cell_slots
                        .entry(owner)
                        .or_default()
                        .entry(position)
                        .or_insert(slot);
                }
                if let Some(slot) = slot
                    && matches!(slot, SlotKey::Item(_) | SlotKey::TermDescription(_))
                {
                    // An item's body belongs to the innermost list around it:
                    // the list's own layout makes the tag, so no float can be
                    // drawn in between.
                    if let Some(owner) = stack.iter().rev().find(|open| open.owns_items) {
                        ancestry.item_slots.entry(owner.location).or_default().push(slot);
                    }
                    // A tight item's single paragraph opens around its body:
                    // that paragraph is the item's.
                    if let Some(parent) = stack.last()
                        && parent.par.is_some()
                    {
                        ancestry.slots.insert(parent.location, slot);
                    }
                }

                // A cell may hold a broken row's content only if it is the
                // cell at its position (not a repeated copy) and not in a
                // footer, whose first copy is drawn below the row's first
                // page.
                let is_body_cell = match (slot, cell_owner, cell_position(inner)) {
                    (Some(slot), Some(owner), Some((x, y))) => {
                        let footer = stack
                            .iter()
                            .rev()
                            .find(|open| open.location == owner)
                            .and_then(|open| open.footer_start)
                            .is_some_and(|start| y >= start)
                            || is_footer_cell(inner);
                        !footer && ancestry.cell_slot(owner, x, y) == Some(slot)
                    }
                    _ => false,
                };
                if let Some(parent) = stack.last_mut() {
                    parent.had_child = true;
                }
                let label_owner = label_owner(inner, stack);
                if let Some(owner) = label_owner {
                    ancestry.label_scopes.entry(owner).or_insert(location);
                }
                stack.push(Open {
                    location,
                    floats: in_floats,
                    par,
                    is_artifact: inner.elem() == ArtifactElem::ELEM,
                    slot,
                    bracket: bracket == Some(admonition::Marker::Start),
                    title_of: label_owner.filter(|_| admonition::is_title(inner)),
                    absorbs: absorbs(inner),
                    owns_items: owns_items(inner),
                    owns_cells: owns_cells(inner),
                    footer_start: footer_start(inner),
                    is_body_cell,
                    is_grid: inner.elem() == GridElem::ELEM,
                    is_placed: is_placed(inner),
                    range: is_placed(inner)
                        .then(|| source_range(engine, inner.span()))
                        .flatten(),
                    body_range: inner
                        .to_packed::<FigureElem>()
                        .and_then(|figure| source_range(engine, figure.body.span())),
                    figure_par,
                    cell_owner,
                    cell_position: cell_position(inner),
                    had_child: false,
                    glyphs_in_artifact: 0,
                    glyphs_outside_artifact: 0,
                    is_text_node: is_par || is_text_node(inner),
                    counts_locale: is_par || is_text_node(inner) || label_owner.is_some(),
                });
            }
            FrameItem::Tag(Tag::End(location, ..)) => {
                // Pop to the matching frame. Tags are balanced in practice;
                // popping by search rather than blindly keeps a stray end tag
                // from unwinding unrelated ancestors.
                // A bracket is closed by its end marker, not by the end tag
                // of its start marker, which shares its location.
                if let Some(index) = stack
                    .iter()
                    .rposition(|open| open.location == *location && !open.bracket)
                {
                    for open in stack.drain(index..) {
                        // A cell holding only untagged content (a `rect`, a
                        // line) counts as holding nothing here: what follows
                        // in its table is still a broken row's content.
                        if let (Some(slot), Some(owner), Some((_, row))) =
                            (open.slot, open.cell_owner, open.cell_position)
                            && !open.had_child
                            && open.is_body_cell
                        {
                            let (last_row, cells) =
                                ancestry.fragment_cells.entry(owner).or_default();
                            if *last_row != row {
                                *last_row = row;
                                cells.clear();
                            }
                            cells.push((slot, open.location));
                        }
                        if let Some(figure) = open.figure_par {
                            if open.glyphs_outside_artifact > 0 {
                                ancestry.figures_with_nodes.insert(figure);
                            } else if open.glyphs_in_artifact > 0 {
                                ancestry.figures_with_artifacts.insert(figure);
                            }
                        }
                        if open.is_text_node
                            && open.glyphs_outside_artifact == 0
                            && open.glyphs_in_artifact > 0
                        {
                            ancestry.in_artifact.insert(open.location);
                        }
                    }
                }
            }
            FrameItem::Text(text) => {
                let in_artifact = scope.in_artifact(stack);
                let locale = Locale::new(text.lang, text.region);
                for open in stack.iter_mut() {
                    if open.is_text_node {
                        if in_artifact {
                            open.glyphs_in_artifact += 1;
                        } else {
                            open.glyphs_outside_artifact += 1;
                        }
                    }
                    if in_artifact || !open.counts_locale {
                        continue;
                    }
                    let counts = ancestry.locales.entry(open.location).or_default();
                    match counts.iter_mut().find(|(known, _)| *known == locale) {
                        Some((_, count)) => *count += text.glyphs.len(),
                        None => counts.push((locale, text.glyphs.len())),
                    }
                }
            }
            _ => {}
        }
    }
}

/// Close the innermost open admonition bracket, if any, and return its start
/// marker: an end marker with no bracket open closes nothing.
fn close_bracket(stack: &mut Vec<Open>) -> Option<Location> {
    let index = stack.iter().rposition(|open| open.bracket)?;
    Some(stack.remove(index).location)
}

/// Whether the open paragraph `par` holds the tags of the element at
/// `location` in its own content, realized: then its text leaves that
/// element out when it is an excluded scope.
fn holds_tag_of(stack: &[Open], par: Location, location: Location) -> bool {
    let Some(content) = stack
        .iter()
        .rev()
        .find(|open| open.location == par)
        .and_then(|open| open.par.as_ref())
        .map(|par| &par.content)
    else {
        return false;
    };
    content
        .traverse(&mut |element: Content| match element
            .to_packed::<typst_library::introspection::TagElem>(
        ) {
            Some(tag) if tag.tag.location() == location => ControlFlow::Break(()),
            _ => ControlFlow::Continue(()),
        })
        .is_break()
}

/// The innermost slot enclosing an element held by `held_by` (see
/// [`Floats::holding`]): the innermost open slot that encloses it (see
/// [`Open::encloses`]) or, for a float's body, the slot open where the float
/// is written. A float's body can be drawn before that slot opens — on an
/// earlier page, while its list is already open — and then only the
/// float's own record says where it belongs.
///
/// An element laid out in a table or a grid with no cell open around it is
/// the content of a row laid out across pages, whose cells' tags closed
/// before it (see the module docs): `InTable` names that table or grid.
///
/// A slot that `own`, the element itself, opens is never its slot. A figure
/// with a `placement` floats through a `place` that its own show rule
/// writes inside it: that float is written in the figure's body, and the
/// figure is held by it, since they share their span.
fn innermost_slot(
    stack: &[Open],
    floats: &Floats,
    held_by: &[usize],
    own: Location,
) -> Option<Found> {
    let written_in = held_by
        .iter()
        .filter_map(|index| floats.all[*index].slot)
        .find(|slot| slot.location() != own);
    for open in stack.iter().rev().filter(|open| open.encloses(floats, held_by)) {
        if open.owns_cells {
            return Some(Found::InTable(open.location));
        }
        let Some(slot) = open.slot else { continue };
        // A slot inside the float's body is the element's own. One the
        // float is written in may have a deeper one that is not open yet.
        let in_body =
            held_by.is_empty() || held_by.iter().any(|index| open.floats.contains(index));
        return Some(Found::Slot(if in_body {
            slot
        } else {
            written_in.unwrap_or(slot)
        }));
    }
    written_in.map(Found::Slot)
}

/// What [`innermost_slot`] finds.
enum Found {
    Slot(SlotKey),
    /// Laid out directly in this table or grid, outside any cell.
    InTable(Location),
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

/// An element laid out as (part of) a paragraph, as [`covering_par`] tests
/// it: its source range, and the floats that may hold it.
fn open_par(engine: &Engine, floats: &Floats, inner: &Content) -> OpenPar {
    let mut par = OpenPar {
        range: content_range(engine, inner),
        content: inner.clone(),
        text: OnceCell::new(),
        floats: Vec::new(),
        floats_by_source: Vec::new(),
        body_of: Vec::new(),
    };
    for index in floats.candidates(&par) {
        let float = &floats.all[index];
        let by_range = float.holds_by_range(&par);
        let by_text = float.holds_by_text(&par);
        if by_range || by_text {
            par.floats.push(index);
        }
        let sure = by_range || (by_text && par.range.is_none());
        if sure {
            par.floats_by_source.push(index);
        }
        // Traversing the paragraph is the costly test, and
        // rarely needed: last.
        if sure && by_text && (par.range.is_none() || float.holds_in_body(&par)) {
            par.body_of.push(index);
        }
    }
    par
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
/// A float settles it first. Its tags open and close where it is written,
/// but its body is drawn wherever it fits — possibly inside a later
/// paragraph that spans the page, and that may well repeat the float's
/// sentence. Nothing in the frames around the body says it is a float, so a
/// paragraph that any float may hold (`OpenPar::floats`: by its range, or
/// by its words) is never covered, whichever signal would otherwise apply,
/// unless both of these hold:
///
/// - some float surely holds the nested paragraph
///   (`OpenPar::floats_by_source` is not empty), and
/// - the enclosing paragraph is the body (`OpenPar::body_of`) of every float
///   that surely holds it.
///
/// The first condition is not implied by the second: an empty set is a
/// subset of anything, so without it a paragraph that floats hold only by
/// its words — none surely — would be absorbed by any float body repeating
/// those words, including content shown both in the flow and through a
/// float helper. An empty sure set therefore never permits absorption.
/// Together they make the enclosing paragraph the body of a float that may
/// hold the nested one, since a float that surely holds it also may.
///
/// Then the nested paragraph is a box inside that body, and the usual
/// signals decide. A paragraph no float may hold follows the usual signals
/// alone.
///
/// A float's body is a frame of its own, so no other float lands inside one
/// of its paragraphs: requiring the enclosing paragraph to *be* a float's
/// body, not merely to be reached by the float's range or repeated by its
/// words, is what keeps a float from being taken for a box. Requiring it of
/// *every* float that surely holds the nested paragraph keeps a paragraph of
/// some other float from being taken for a box of this body. Only an
/// enclosing paragraph with no range of its own is taken for a body on its
/// words alone. The cost is
/// duplication, not loss: a box inside a float body that the float holds
/// only by words — a body built from an earlier `#let` — is kept twice, and
/// so is a box inside a float whose body realizes to other words than it is
/// written with.
fn covering_par(stack: &[Open], par: &OpenPar) -> Option<Location> {
    let nearest = stack.iter().rev().find(|open| open.par.is_some())?;
    let parent = nearest.par.as_ref()?;
    // A paragraph any float may hold is never covered, unless some float
    // surely holds it and the parent is the body of every float that surely
    // holds it: then it is a box in that body. An empty sure set never
    // permits absorption.
    if !par.floats.is_empty()
        && (par.floats_by_source.is_empty()
            || !par
                .floats_by_source
                .iter()
                .all(|index| parent.body_of.contains(index)))
    {
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

#[cfg(test)]
mod tests {
    use super::{occurs_as_words, words};

    /// `Floats::candidates` looks a float up by one word of the text it
    /// searches for: wherever that text occurs as words, each of its words
    /// must be one of the float's own.
    #[test]
    fn text_occurring_as_words_has_its_words_whole() {
        for (haystack, needle) in [
            ("Float 10 body with a few words.", "body with"),
            ("He said “Yes indeed” twice.", "“Yes indeed”"),
            ("(see note) and more", "(see note)"),
            ("état-major, à l’ouest", "major, à"),
            ("x1 y2", "y2"),
        ] {
            assert!(occurs_as_words(haystack, needle), "{needle:?} in {haystack:?}");
            for word in words(needle) {
                assert!(
                    words(haystack).any(|own| own == word),
                    "{word:?} must be a word of {haystack:?}"
                );
            }
        }
        assert!(!occurs_as_words("Drafting", "Draft"));
    }
}
