// Decorative content: everything wrapped in `pdf.artifact` is presentation
// only and never reaches the CND — no node, no footnote, no link edge. Two
// decorative footnotes (one in a float) come first, so the kept one is
// numbered 3. Decorative references, links, citations and footnote
// re-references add no edge, even to kept targets, and a decorative float
// is dropped while content it reuses from a `#let` stays where it is kept.
// Every decorative word below is "Ornament…" so a test can look for it
// anywhere in the output.
#set document(title: "Decorative content", author: "Test Author")
#set heading(numbering: "1.")
#let deco(body) = pdf.artifact(body)

= Kept heading

Kept paragraph.

#deco[= Ornament heading]
#deco[Ornament paragraph.]
#deco[
  - Ornament item one
  - Ornament item two
]
#deco(table(columns: 2, [Ornament A], [Ornament B]))
#deco(image("newsletter/newsletter-cover.png", alt: "Ornament image", width: 1cm))
#deco(figure(image("newsletter/newsletter-cover.png", width: 1cm), caption: [Ornament caption]))
#deco(quote(block: true)[Ornament quote])
#deco(raw(block: true, "ornament_code()"))
#deco[$ x^2 + "Ornament" $]
#deco[/ OrnamentTerm: Ornament definition]
#place(top + right, deco[Ornament placed])
#deco[#box[Ornament boxed]]
#deco(place(top, float: true, scope: "parent")[Ornament floating paragraph#footnote[Ornament float note.]

  - Ornament floating item
])
#let reused = [Kept reused words.]
#reused
#deco(place(bottom, float: true)[#reused])

#deco[#figure(table(columns: 1, [Ornament cell]), caption: [Ornament table]) <ornament-fig>]
#deco[== Ornament section <ornament-head>]

Kept text refers to @ornament-fig and @ornament-head.

#figure(table(columns: 1, [Kept cell]), caption: [Kept table]) <kept-fig>

Kept inline #deco[see @kept-fig and #link("https://ornament.example")[ornament site] and #link(<kept-fig>)[ornament link]] host.

#deco[Ornament block refers to @kept-fig.]

Kept after the decorative block.

- Kept item #deco[#link("https://ornament.example/item")[x] @kept-fig] tail

#deco[Ornament#footnote[Ornament note.] <ornament-fn> footnote.]

Kept footnote#footnote[Kept note.] <kept-fn> host.

Kept again#deco[#footnote(<kept-fn>)] host.

Kept re-reference#footnote(<ornament-fn>) host.

Inline #deco[Ornament] word kept.

Kept cites @smith2024 and #deco[@jones2022] here.

#bibliography("refs.yml", style: "ieee")
