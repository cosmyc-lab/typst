// Block quotes and footnotes hold blocks. A block quote's paragraphs and
// list are its children, and its attribution is a text of its own. A
// footnote's paragraphs and list are the children of its pool entry, on the
// page where the note is printed. An inline quote stays text of its
// paragraph. A marker written in an attribution is an edge of the
// attribution, never of the body. A decorative footnote is not emitted at
// all.
#set document(title: "Quotes and notes", author: "typst-cnd")
#set math.equation(numbering: "(1)")

= Quotes

#quote(block: true, attribution: [Ada])[
  QA1 first quote paragraph.

  QA2 second quote paragraph cites @knuth and carries a note.#footnote[QN1 note written in the quote.]

  - QA3 quote list item
  - QA4 quote list item
]

The paragraph holds an inline #quote[short] quotation.

A sentence holds #box(quote(block: true)[QD1 boxed block quote]) inline.

= Attributions

#quote(block: true, attribution: [Grace, after @knuth])[
  QB1 body referring to @eq-1.
]

#quote(block: true, attribution: <knuth>)[QC1 body.]

= Notes

$ x = y $ <eq-1>

A paragraph with a long note.#footnote[NA1 first note paragraph.

  NA2 second note paragraph, see @eq-1.

  - NA3 note item
  - NA4 note item
]

Decorative #pdf.artifact[#footnote[DN1 decorative note]] and kept.#footnote[NK1 kept note.]

#pagebreak()

Page two paragraph.#footnote[NP1 note on page two.]

#bibliography("refs.yml", style: "ieee")
