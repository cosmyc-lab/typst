// A footnote's shown number that a node already carries as its label is
// taken: the footnote gets the next free suffixed label.
#set document(title: "Footnote label clash", author: "typst-cnd")

= Section one <1>

A paragraph with a note#footnote[The only note.].
