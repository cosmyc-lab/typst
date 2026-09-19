// Text carried by layout-only containers: a `block`, a grid cell, a `place`,
// an inline `box`, and content produced by a user function. None of these
// wrappers is emitted as a node, so the paragraphs inside them are the only
// carrier of that text.

#set document(title: "Layout containers", author: "typst-cnd")

#set page(
  paper: "a5",
  margin: 1.5cm,
  footer: context [RUNNING FOOTER · #counter(page).display()],
)

#let section(kicker, title) = {
  block(spacing: 0.9em)[#text(weight: "bold")[#upper(kicker)]]
  block(below: 1.1em)[#text(size: 18pt)[#title]]
}

#let card(title, items) = {
  block(fill: luma(240), inset: 8pt, width: 100%)[
    #text(weight: "bold")[#upper(title)]
    #v(0.6em)
    #for it in items {
      block(spacing: 0.6em)[#it]
    }
  ]
}

#let chip(body) = box(inset: (x: 6pt, y: 3pt), stroke: 0.5pt)[#body]

= Layout containers

#section("Kicker line", "Title inside a block")

A paragraph with #chip[FIRST CHIP] and #chip[SECOND CHIP] inline.

A paragraph where #chip[idem] repeats a word the sentence itself uses: idem.

#grid(
  columns: (1fr, 1fr),
  card("Left card", ("Left item one", "Left item two")),
  card("Right card", ("Right item one", "Right item two")),
)

#place(bottom + left, block(width: 100%)[Placed punch line at the bottom.])

Hard break here: first line \ second line.
