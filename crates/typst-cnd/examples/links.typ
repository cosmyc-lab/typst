#set document(title: "Hyperlinks", author: "typst-cnd")
#set heading(numbering: "1")

// This section holds only the nested-ref paragraph below, deliberately
// alone under its own heading. `refs.rs::ref_edges_from_content` has a
// pre-existing, unrelated bug (see comment in that file / follow-up):
// its position-correlation heuristic only tracks `HeadingElem`, never
// `ParElem` (which does not exist pre-realization), so it attributes every
// bare `@ref` under a heading to that heading, then also stamps a spurious
// spanless `refs` edge onto the *last* paragraph under it — unless that
// paragraph already has its own spanned ref markers. Putting every other
// test paragraph under a *second* heading with no bare `@ref` in it keeps
// that bug from touching this fixture's assertions. Do not add more
// paragraphs under "Overview", or move the position-link/list cases here:
// either would silently pick up the same spurious edge again.
= Overview <sec-overview>

The nested case: #link("https://example.com")[@sec-overview and more text]
is the case that motivated capturing `LinkElem` via `open_frame` (ADR 0024):
one `links` entry for the whole body, one `refs` entry for the ref inside
it, both non-null and non-colliding.

= Other cases

A plain hyperlink to #link("https://example.com/docs") in running text.

A link to a label with a custom body: #link(<sec-overview>)[see the overview].

An empty-bodied link carries no rendered text: #link("https://example.com/cover")[].

// A paragraph-initial body that splits into two paragraphs (the link opens
// the block, with no leading text before it). Verified against a real
// compile: this link produces NO `links` entry on either resulting
// paragraph, not a null-spanned one. This is the one shape the null-span
// flush in `extract.rs::extract_with_markers` cannot reach: a
// paragraph-initial link's `Tag::Start` sits before any paragraph group's
// trigger range, so both its tags stay in the outer flow and never enter
// either paragraph's own walk. A link with leading text before it in the
// same paragraph (see the other cases in this file) does not have this
// problem — the flush rescues those. See that doc comment for the exact
// rule and for the separate `#block`/`#image` case.
#link("https://example.com/multipart")[First half of a multi-paragraph link body.

Second half of the same link body.]

A position link is not durable: #link((page: 1, x: 0pt, y: 0pt))[Go to top].

- A list item linking #link("https://example.com/list")[here].
