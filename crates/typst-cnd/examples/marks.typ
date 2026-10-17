// Inline formatting becomes standoff marks on a paragraph's text (spec
// §4.1). Typst markup cannot cross two delimiters (`*a _b* c_` does not
// parse), so the overlapping strong/emph case is written as an emphasis
// nested in the strong run followed by one right after it: the two emph
// runs touch and merge, and the result overlaps the strong.
#set text(lang: "en")
#set document(
  title: "Inline marks",
  author: ("Test",),
  date: datetime(year: 2026, month: 10, day: 17),
)

= Inline marks

Start *bold _both_*_ italic_ end.

Use `x = 1` now.

Speed is $v = d/t$ here.

The area is 4 m#super[2] in total.

A claim with a note.#footnote[The note body.]

😀 *bold* @fig-a

Join *a**b* here.

Read the #link("https://example.com")[`api`] docs.

#pdf.artifact[★] $w$

$z$

#figure(rect(width: 1cm, height: 1cm), caption: [A square.]) <fig-a>
