// Callouts written with `cnd.admonition`: a titled warning holding a
// paragraph and a list, a free kind, one nested in a list item, one nested
// in another, a titled one carrying a label of the author's, one whose title
// is in another language, a decorative one that is never emitted, and one
// shown as a figure.
#import "vendor/cnd.typ"
#set document(title: "Callouts", author: "typst-cnd")

= Callouts

Before the callouts.

#cnd.admonition(kind: "warning", title: [Careful])[Para one.

- item]

#cnd.admonition(kind: "example")[An example body.]

- An outer item.

  #cnd.admonition(kind: "tip", title: [Nested])[Inside the item.]

#cnd.admonition(kind: "important", title: [A _styled_ title])[Outer body.

  #cnd.admonition(
    kind: "note",
    title: [See @fig-tip and #link("https://example.com")[the guide]],
  )[Inner body.]

  Outer tail.
] <warn-1>

#cnd.admonition(kind: "note", title: text(lang: "de")[Achtung])[A body in the document's language.]

#pdf.artifact[#cnd.admonition(kind: "caution", title: [Hidden])[Ornamental callout text.]]

After the callouts.

#figure(
  cnd.admonition(kind: "tip")[A callout shown as a figure.],
  caption: [A boxed tip],
) <fig-tip>
