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

// A body that splits into two paragraphs. Verified against a real compile:
// this link produces NO `links` entry on either resulting paragraph, not a
// null-spanned one — a known, currently-uncovered gap, not the outcome the
// null-span flush in `extract.rs` was hoped to fix. `typst-realize`'s
// paragraph grouping hoists a `Tag::Start`/`Tag::End` pair that straddles a
// paragraph-break boundary *out of both* resulting groups, so neither tag
// ever appears in either paragraph's own walk — there is no open frame in
// either one for the flush to catch. The same is true of `#block[..]` and
// `#image(..)` bodies (see `extract.rs::extract_with_markers`'s doc
// comment): all three land in the gap between nodes.
#link("https://example.com/multipart")[First half of a multi-paragraph link body.

Second half of the same link body.]

A position link is not durable: #link((page: 1, x: 0pt, y: 0pt))[Go to top].

- A list item linking #link("https://example.com/list")[here].
