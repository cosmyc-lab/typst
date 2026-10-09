// Regression: a list inside a float or a footnote is never lost, and
// never emitted twice.
#set document(title: "List in a footnote in a paragraph", author: "typst-cnd")
Para PP1 text#footnote[FN2 note
  - FL3 list in footnote of paragraph
]
