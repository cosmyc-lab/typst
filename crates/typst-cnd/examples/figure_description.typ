// A figure's `alt` is the text equivalent of its visual: the figure's
// description. The caption stays the short title shown on the page.
#set document(title: "Figure description", author: "typst-cnd")

= Shares

#figure(
  image("newsletter/newsletter-cover.png", width: 3cm),
  alt: "Bars: A 38 %, B 27 %",
  caption: [Shares],
) <fig-shares>

#figure(
  image("newsletter/newsletter-cover.png", alt: "Cover art", width: 3cm),
  caption: [No description],
) <fig-plain>

The shares are in @fig-shares.

#figure(
  image("newsletter/newsletter-cover.png", width: 3cm),
  alt: "  ",
  caption: [Blank alt],
) <fig-blank>
