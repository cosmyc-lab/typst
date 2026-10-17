// Tables and grids whose cells hold blocks: a cell's paragraphs, images
// and lists are its children, a bare grid is a `layout`, a heading in a
// cell opens a section in that cell only, and a marker written in a cell
// sits on the cell's own paragraph.
#set document(title: "Containers: grids", author: "typst-cnd")

= Outer

#table(
  columns: 2,
  table.header([Notes], [Picture]),
  [
    First paragraph of the alpha notes.

    Second paragraph of the alpha notes.
  ],
  image("newsletter/newsletter-cover.png", alt: "Alpha cover", width: 1cm),
)

#grid(
  columns: 2,
  gutter: 8pt,
  [Left block of the bare grid.], [Right block of the bare grid.],
)

#grid(
  columns: 2,
  [
    == Inner

    Paragraph under the inner heading.
  ],
  [Second cell of the sectioned grid.],
)

After the sectioned grid.

#heading(numbering: "1")[Related] <grid-note>

#table(
  columns: 2,
  [Key], [Cell text pointing to @grid-note with a note.#footnote[A note written in a cell.]],
)

= Nesting

#quote(block: true)[
  #table(
    columns: 1,
    [
      - Quoted cell item one.
      - Quoted cell item two.
    ],
  )
]

Footnoted table.#footnote[
  #table(
    columns: 1,
    [
      - Noted cell item one.
      - Noted cell item two.
    ],
  )
]

- Item holding a table:
  #table(
    columns: 1,
    [
      - Itemized cell entry one.
      - Itemized cell entry two.
    ],
  )

A sentence with an inline #box[#list[Boxed list entry.]] inside it.
