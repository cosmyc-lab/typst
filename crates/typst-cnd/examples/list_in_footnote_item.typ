// Regression: a list inside a float or a footnote is never lost, and
// never emitted twice.
#set document(title: "List in a footnote in a list item", author: "typst-cnd")
- FA1 parent#footnote[FN1 note text
  - FL1 list in footnote
  - FL2 list in footnote
]
- FA2 sibling
