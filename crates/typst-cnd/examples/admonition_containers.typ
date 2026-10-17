// Callouts in containers — a table cell, a block quote, a footnote — one
// holding a figure and a heading, and one restyled by show rules.
#import "vendor/cnd.typ"
#set document(title: "Callouts in containers", author: "typst-cnd")

= Section

#table(columns: 2, [#cnd.admonition(title: [In cell])[Cell body.]], [Plain cell.])

#quote(block: true)[#cnd.admonition(title: [In quote])[Quote body.]]

Text with a note.#footnote[#cnd.admonition(title: [In note])[Note body.]]

#cnd.admonition(title: [Holds a figure])[
  Before figure.
  #figure(rect[x], caption: [Inner caption]) <inner>
  After figure, see @inner.
  = Heading inside
  Paragraph after heading.
]

#[
  #show block: it => { set text(fill: blue); it }
  #show <cnd-admonition-title>: it => block(emph(it.body))
  #cnd.admonition(kind: "warning", title: [Restyled])[Restyled body.

  - restyled item]
]

Closing.
