// A figure around a drawing built by a helper defined earlier in the file: a
// decorative grid, an image and a text label, each placed in a box.
#set document(title: "Figure helper drawing", author: "typst-cnd")
#let drawing = box(width: 3cm, height: 3cm, {
  place(grid(columns: 2, box(width: 1cm, height: 3mm, fill: gray), box(width: 1cm, height: 3mm, fill: gray)))
  place(dy: 1cm, image("newsletter/newsletter-cover.png", width: 1cm))
  place(dx: 1.5cm, dy: 1.5cm, text("HA1 callout label"))
})
#figure(drawing, caption: [HA2 caption])
