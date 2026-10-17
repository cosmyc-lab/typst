// Figures around different bodies: an author-defined kind holding
// paragraphs, two images side by side in a grid, two subfigures, a
// drawing, two inline images side by side, and a floating figure in a
// list item.
#set document(title: "Figure kinds", author: "typst-cnd")

= Kinds

#figure(
  kind: "theorem",
  supplement: [Theorem],
  [Every *bounded* set of real numbers has a least upper bound.

  A second paragraph, beside @fig-pair.],
  caption: [KA1 completeness],
) <thm-complete>

#figure(
  grid(
    columns: 2,
    image("newsletter/newsletter-cover.png", alt: "Left cover", width: 2cm),
    image("newsletter/newsletter-cover.png", alt: "Right cover", width: 2cm),
  ),
  caption: [KB1 two covers],
) <fig-pair>

#figure(
  [
    #figure(
      image("newsletter/newsletter-cover.png", alt: "First part", width: 2cm),
      caption: [KC1 first part],
    ) <fig-part-a>
    #figure(
      image("newsletter/newsletter-cover.png", alt: "Second part", width: 2cm),
      caption: [KC2 second part],
    ) <fig-part-b>
  ],
  caption: [KC3 two parts],
) <fig-parts>

#figure(rect(width: 2cm, height: 1cm), caption: [KD1 a drawing]) <fig-drawing>

#figure(
  [#box(image("newsletter/newsletter-cover.png", alt: "Inline left", width: 1cm)) #box(image("newsletter/newsletter-cover.png", alt: "Inline right", width: 1cm))],
  caption: [KF1 inline pair],
) <fig-inline>

- KE1 item holding a floating figure.
  #figure([KE2 floating body.], caption: [KE3 floating caption], placement: bottom) <fig-float>
