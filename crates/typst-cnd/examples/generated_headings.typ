// The outline and the bibliography print a heading of their own. When
// nothing of the body follows it before the next section, it titles nothing
// the CND holds as nodes (the outline's entries are absorbed, the
// bibliography is its pool), so it is no node.
#set document(title: "Generated headings", author: "typst-cnd")

#outline()

= Intro

Text citing @smith2024.

#bibliography("refs.yml")
