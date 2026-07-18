#set document(title: "Image figures", author: "typst-cnd")

= Figures with images

A captioned image is an `ImageNode` wrapped in a `FigureNode` (ADR 0010):
the wrapper carries the caption/number/label, the image child carries the
path and alt text.

#figure(
  image("newsletter/newsletter-cover.png", alt: "Department cover art", width: 3cm),
  caption: [The department cover art.],
) <fig-cover>

See @fig-cover for the cover.
