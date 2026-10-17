// Callouts and floats: a float written inside a callout, a callout drawn in a
// float at the top and at the bottom of the page, one in a `place` that does
// not float, and one in a figure that floats.
#import "vendor/cnd.typ"
#set document(title: "Floating callouts", author: "typst-cnd")

Opening.

#cnd.admonition(kind: "note", title: [Holds a float])[
  Body one.
  #place(top, float: true)[Floated paragraph inside.]
  Body two.
]

#place(top, float: true)[#cnd.admonition(kind: "tip", title: [Float top])[Top body.]]

#place(bottom, float: true)[#cnd.admonition(kind: "tip", title: [Float bottom])[Bottom body.]]

#place(bottom)[#cnd.admonition(kind: "tip", title: [Placed])[Placed body.]]

#figure(
  placement: top,
  cnd.admonition(kind: "example")[Figure float body.],
  caption: [A floating figure],
)

Closing.
