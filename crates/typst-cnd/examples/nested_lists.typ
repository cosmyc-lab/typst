// Nested lists: each nested item is a child of its parent item, never fused
// into the parent's text nor repeated as a list of its own.
#set document(title: "Nested lists", author: "typst-cnd")

- NA1 top one
- NA2 top two
  - NA3 child one
  - NA4 child two
    - NA5 grandchild
- NA6 top three

Separator one.

+ NB1 enum one
+ NB2 enum two
  - NB3 bullet in enum
  - NB4 bullet in enum

Separator two.

- NC1 bullet with enum children
  #set enum(start: 5)
  + NC2 enum at five
  + NC3 enum at six

Separator three.

- ND1 bullet with explicit numbers
  5. ND2 explicit five
  + ND3 follows five

Separator four.

- NE1 explicit list inside
  #list[NE2 explicit child][NE3 explicit child]

Separator five.

#quote(block: true)[NF1 quote paragraph.

- NF2 quote list item
- NF3 quote list item
]

#figure(block[NG1 paragraph in figure.

- NG2 list item in figure
], caption: [NG3 figure caption])

NH1 closing paragraph.
