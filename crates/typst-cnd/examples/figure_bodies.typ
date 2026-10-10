// Figures around content that is not an image, a raw block or a table: a
// framed block, grids written in different ways, and a bare grid for
// comparison. Nothing may be emitted twice.
#set document(title: "Figure bodies", author: "typst-cnd")
#let boxed(b) = block(stroke: 0.5pt, inset: 4pt, b)
#figure(boxed[FA1 paragraph in figure.

- FA2 list item in figure
], caption: [FA3 caption one])

#figure(grid(columns: 2, [FB1 key], [FB2 value], [FB3 key], [FB4 value]), caption: [FB5 caption grid])

#figure(context grid(columns: 2, [FC1 key], [FC2 value]), caption: [FC3 caption context grid])

#figure(block(grid(columns: 2, [FD1 key], [FD2 value])), caption: [FD3 caption block grid])

#grid(columns: 2, [FE1 bare grid key], [FE2 bare grid value])

#figure(table(columns: 2, [FF1 key], [FF2 value]), caption: [FF3 caption table])
