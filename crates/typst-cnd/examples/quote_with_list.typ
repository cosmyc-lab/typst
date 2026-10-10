// A block quote holding a paragraph and a list: the quote carries the list's
// text, so the list is not a node of its own.
#set document(title: "Quote with a list", author: "typst-cnd")
#quote(block: true)[QW1 quote paragraph.

- QW2 quote list item
- QW3 quote list item
]
QW4 after.
