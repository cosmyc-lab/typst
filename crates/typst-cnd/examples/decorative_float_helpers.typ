// Decorative floats built by helpers: the `place` is written in the helper
// and its body at the call site, so its source range reaches across the
// paragraphs in between. None of that kept text may be dropped. (A float
// whose body is not written inside its own `place` call is not claimed as
// decorative at all: it leaks rather than risk a loss.)
#set document(title: "Decorative float helpers", author: "typst-cnd tests")
#let decofloat(x) = pdf.artifact(place(top, float: true)[#x])
#let aside(x) = place(bottom, float: true)[#x]

= Kept heading

Kept alpha paragraph.

- Kept list item

Kept beta paragraph.

#decofloat[Decorative helper words.]

Kept gamma paragraph.

#aside[Kept aside words.]

Kept delta between.

#pdf.artifact(aside[Decorative aside words.])

#let reused = [
  - Kept reused item
  #table(columns: 1, [Kept reused cell])
]

#reused

#decofloat(reused)

#pdf.artifact(place(top, float: true)[Ornament outer #place(bottom, float: true)[Ornament nested]])

Kept end.
