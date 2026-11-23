// The outline and the bibliography print a heading of their own. It titles
// nothing the CND holds as nodes (the outline's entries are absorbed, the
// bibliography is its pool), so it is no node: the paragraph after the
// outline is not a child of "Contents".
#set document(title: "Generated headings", author: "typst-cnd")

#outline()

An opening paragraph before any section.

= Intro

Text citing @smith2024.

#bibliography("refs.yml")
