// A heading the outline or the bibliography prints stays a heading when
// body content follows it before the next section: that content is in its
// section, as the built document shows it.
#set document(title: "Generated headings followed by content", author: "typst-cnd")

#outline()

A paragraph after the outline.

= Intro

Text citing @smith2024.

#bibliography("refs.yml")

An appendix note after the bibliography.
