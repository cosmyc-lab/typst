// Drawings in figures: a canvas built in a box, whose labels are placed;
// a boxed shape; a boxed shape alone in the flow; and a figure that a show
// rule places, which is no drawing.
#set document(title: "Figure drawings", author: "typst-cnd")
#let canvas = box(width: 3cm, height: 3cm, {
  place(line(start: (0pt, 0pt), end: (2cm, 2cm)))
  place(dx: 0cm, dy: 0cm, text("DA1 node start"))
  place(dx: 1.5cm, dy: 1.5cm, text("DA2 node end"))
})
#figure(canvas, caption: [DA3 boxed canvas]) <fig-canvas>

#figure(box(rect(width: 1cm, height: 1cm)), caption: [DB1 boxed shape]) <fig-shape>

DC1 before the shape.

#box(rect(width: 1cm, height: 1cm))

DC2 after the shape.

#[
  #show figure.where(kind: "framed"): it => place(center, it)
  #figure(kind: "framed", supplement: [Frame], [DD1 placed by a rule.], caption: [DD2 framed]) <fig-framed>
]
#v(3cm)
