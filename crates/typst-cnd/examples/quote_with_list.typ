// A block quote holding a paragraph and a list: both are blocks of the
// quote, and the list is a node nowhere else.
#set document(title: "Quote with a list", author: "typst-cnd")
#quote(block: true)[QW1 quote paragraph.

- QW2 quote list item
- QW3 quote list item
]
QW4 after.
