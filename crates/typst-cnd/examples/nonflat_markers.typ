#set document(title: "Markers in non-flat nodes", author: "typst-cnd")

= Lists with markers

A marker inside a list item still produces its edge, but with a null text
span (the list's text is a concatenation of item strings, not a single
string the offsets index into):

- A plain first item.
- An item carrying a footnote.#footnote[Note attached inside a list item.]
- A plain third item.
