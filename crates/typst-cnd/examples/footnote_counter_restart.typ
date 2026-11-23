// The author restarts the footnote counter, so the built document shows the
// number 1 twice. A label is unique across the CND (spec §2): the first note
// keeps the number it shows, the repeat takes a suffix, and each marker still
// points at its own note.
#set document(title: "Footnote counter restart", author: "typst-cnd")

= One
First#footnote[Note A1] and again#footnote[Note A2].

#counter(footnote).update(0)

= Two
Second#footnote[Note B1].
