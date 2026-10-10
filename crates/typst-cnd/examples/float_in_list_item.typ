// Regression: a float written inside a list item is that item's content,
// even when its body is drawn on a later page while the list is still
// open. It is not emitted a second time as a paragraph of its own.
#set document(title: "Float in a list item", author: "typst-cnd")
#let aside() = place(top, float: true)[FIL1 aside written in an item.]
#set page(width: 160pt, height: 120pt, margin: 10pt)
- First item with filler words: one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen.
- #aside() Second item, long enough to reach the next page: one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree.
- Third item.
