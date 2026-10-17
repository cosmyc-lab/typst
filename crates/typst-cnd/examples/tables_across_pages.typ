// A table that breaks across pages repeats its header and footer on each
// page; each is still one cell. A row too tall for one page keeps each of
// its cells' paragraphs in that cell, on every page it spans. A second
// table spans rows and columns, with a two-row header and gutters.
#set document(title: "Tables across pages", author: "typst-cnd")
#set page(height: 9cm)

= Long table

#table(
  columns: 2,
  table.header([Header key], [Header value]),
  ..range(1, 9).map(i => ([Body key #i], [Body value #i])).flatten(),
  [
    #for i in range(1, 9) [
      Left paragraph #i of the broken row.

    ]
  ],
  [
    #for i in range(1, 13) [
      Tall paragraph #i of the broken cell.

    ]
  ],
  table.footer([Footer key], [Footer value]),
)

= Spans

#table(
  columns: 3,
  gutter: 4pt,
  table.header(
    table.cell(colspan: 3)[Top header],
    [Sub one], [Sub two], [Sub three],
  ),
  table.cell(rowspan: 2)[Tall span], [Row one middle], [Row one end],
  table.cell(colspan: 2)[Wide span],
)

// A broken row beside header and footer cells that hold nothing: those
// cells, drawn between the row's pages, never take its content.
= Empty header cell

#table(
  columns: 2,
  table.header([], [Header right]),
  [
    #for i in range(1, 12) [
      Empty header left #i.

    ]
  ],
  [
    #for i in range(1, 14) [
      Empty header right #i.

    ]
  ],
)

= Empty footer cell

#table(
  columns: 2,
  [
    #for i in range(1, 12) [
      Empty footer left #i.

    ]
  ],
  [
    #for i in range(1, 14) [
      Empty footer right #i.

    ]
  ],
  table.footer([Footer total], []),
)

= Broken grid

#grid(
  columns: 2,
  gutter: 6pt,
  [Short grid cell.],
  [
    #for i in range(1, 14) [
      Grid paragraph #i.

    ]
  ],
)

After the broken grid.

// Tight lists, enums and term lists in a row that breaks across pages:
// each item's single paragraph stays in its item, in order.
#let tight-items(name) = [
  - #name bullet one.
  - #name bullet two #("runs on and on " * 40)until it ends.
  - #name bullet three.
  + #name enum one.
  + #name enum two #("keeps going " * 30)to its end.
  / #name term: #name description #("goes further " * 30)to its close.
  / #name other: #name short description.
]

= Broken row with tight items

#table(
  columns: 2,
  [Tight left.],
  tight-items("Table"),
)

= Broken grid with tight items

#grid(
  columns: 2,
  gutter: 6pt,
  [Grid tight left.],
  tight-items("Grid"),
)

After the tight grid.
