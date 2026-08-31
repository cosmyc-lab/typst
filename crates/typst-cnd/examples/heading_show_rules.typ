// Headings styled through custom show rules that re-emit `it.body` inside
// fresh markup — the realized document then carries an extra paragraph
// wrapping each heading's body, which the emitter must not surface as a
// content paragraph of its own.
#set document(title: "Heading show rules", author: "typst-cnd tests")
#set text(size: 10pt)

#show heading.where(level: 1): it => block(
  width: 100%,
  inset: (bottom: 5pt),
  text(size: 16pt, weight: "bold", it.body)
)

#show heading.where(level: 2): it => [
  #v(1.2em, weak: true)
  #text(size: 13pt, weight: "bold", it.body)
  #v(0.4em, weak: true)
]

= 1. Overview

The first section's real paragraph.

== 1.1 Details

A nested section's real paragraph.

= 2. Operations

Another real paragraph, under the second top-level heading.
