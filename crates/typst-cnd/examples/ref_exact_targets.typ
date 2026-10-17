// Reference targets are exact: an edge lands only on the node that carries
// the label it names. The drawing below keeps its source, not the figure
// placed inside it, so that inner figure is no node and a reference to it
// has no edge; it never falls back to another figure.
#set document(title: "Exact reference targets", author: "typst-cnd")
#set figure(numbering: "1")

= Targets

#figure(
  table(columns: 2, [Key], [Value], [mode], [on]),
  caption: [A plain table],
) <fig-plain>

#figure(
  box(width: 3cm, height: 2cm, {
    place(line(start: (0pt, 0pt), end: (2cm, 1cm)))
    place(dx: 1cm, dy: 0.5cm)[#figure(rect(width: 5mm, height: 5mm), caption: [Inner part]) <fig-inner>]
  }),
  caption: [A drawing],
) <fig-drawing>

The table is @fig-plain, the drawing is @fig-drawing and its part is
@fig-inner.

= Elsewhere

A later paragraph names the table again: @fig-plain.
