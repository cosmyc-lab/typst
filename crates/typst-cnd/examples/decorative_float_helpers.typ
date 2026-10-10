// Decorative floats built by helpers: the `place` is written in the helper
// and its body at the call site, and content reused from a `#let` shares its
// spans with its kept copy. A float's body is told apart by the frame group
// layout draws it in (its logical parent is the `place`), never by source
// positions: the decorative bodies are dropped, the kept text around them
// and the kept copies stay.
#set document(title: "Decorative float helpers", author: "typst-cnd tests")
#let decofloat(x) = pdf.artifact(place(top, float: true)[#x])
#let aside(x) = place(bottom, float: true)[#x]

= Kept heading

Kept alpha paragraph.

- Kept list item

Kept beta paragraph.

#decofloat[Ornament helper words.]

Kept gamma paragraph.

#aside[Kept aside words.]

Kept delta between.

#pdf.artifact(aside[Ornament aside words.])

#let reused = [
  - Kept reused item
  #table(columns: 1, [Kept reused cell])
]

#reused

#decofloat(reused)

#pdf.artifact(place(top, float: true)[Ornament outer #place(bottom, float: true)[Ornament nested]])

#let lst = [
  - Shared item
]
#place(top, float: true)[#lst]
#pdf.artifact[#lst]

#show heading.where(level: 3): it => pdf.artifact(it)
=== Ornament subheading wrapped by a show rule

Kept end.
