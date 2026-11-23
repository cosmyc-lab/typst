// Horizontal spacing separates the words around it like a space: one space,
// never two, none where nothing follows, and none for a negative amount
// (kerning). A soft hyphen only marks where a word may break: it is not
// text.
#set document(title: "Spacing and soft hyphens", author: "typst-cnd")

= Name:#h(1fr)Heading

Name:#h(1fr)Value.

Left #h(1cm) right.

Kerned L#h(-0.1em)A word and #h(0pt)glued.

Trailing#h(1fr)

A soft-hy-?phen and an Ex-?ample with *bold* after.
