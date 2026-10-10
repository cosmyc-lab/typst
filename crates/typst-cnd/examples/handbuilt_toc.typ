// A hand-built table of contents: a grid row per heading, with the number in
// a styled box, the title, a dot leader and the page number. The boxed number
// is part of its cell's paragraph and must come out once, not twice. The
// title cells repeat headings that are nodes of their own.
#set document(title: "Hand-built table of contents", author: "typst-cnd")
#set heading(numbering: "1.")
#let pill(n) = box(fill: luma(230), inset: 2pt, text(weight: 700, n))
#context {
  for hd in query(heading) {
    let n = counter(heading).at(hd.location()).first()
    grid(
      columns: (auto, auto, 1fr, auto),
      column-gutter: 3mm,
      pill("TP" + str(n)), text(hd.body), box(width: 1fr, repeat[.]), "TG" + str(hd.location().page()),
    )
  }
}
#pagebreak()
= TA1 first part
TA2 body.
#pagebreak()
= TB1 second part
TB2 body.
