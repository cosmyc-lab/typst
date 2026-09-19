// Paragraphs that only exist because a fragment body was realized, and the
// three ways that can go wrong: an inline `box` duplicating its enclosing
// paragraph, a deferred float being mistaken for one, and page furniture or
// a rendered pool entry becoming prose.
#set document(title: "Inline boxes and floats", author: "typst-cnd")
#set page(
  width: 280pt,
  height: 120pt,
  margin: 20pt,
  foreground: rotate(45deg, text(fill: luma(210), size: 24pt)[DRAFT STAMP]),
)
#set heading(numbering: "1.")

= Target section <sec>

A sentence with #box[see @sec], with #box[@smith2024], and with
#box[boxed#footnote[Note written inside a box.] tail] in it.
#place(bottom, float: true)[FLOATED BODY TEXT]
Filler prose after the float, long enough to push the float onto a page of
its own and to keep the two paragraphs apart.

= References

#bibliography("refs.yml", title: none)
