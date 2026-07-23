#set text(lang: "en")
#set heading(numbering: "1.1")
// The counter pattern carries the value only; the displayed word comes
// from the supplement. "Figure 1" as a pattern is wrong — Typst reads the
// `i` as a roman counter, yielding "Fi"/"Fii".
#set figure(numbering: "1")
#set math.equation(numbering: "(1)")
#set document(
  title: "Hardcore semantic fixture — CND integration",
  author: ("CND QA", "Semantic Team"),
  date: datetime(year: 2026, month: 6, day: 20),
  keywords: ("quote", "code", "math", "list", "grid", "figure", "refs", "nested"),
  description: "Stress test for semantic node extraction: nested headings, quotes, lists, code, equations, tables, grids, figures, cross-refs, metadata.",
)

[PREAMBLE] Executive summary before the first heading — must land in a chunk with empty heading_path.

= Semantic corpus <ch-corpus>

#cnd.metadata.update(it => it + (domain: "semantic", revision: "hard-1"))

Overview paragraph introducing the corpus. Every block type below must map to a dedicated CND node, not a duplicate paragraph.

== Quotations and attribution <sec-quotes>

#quote(attribution: [Donald Knuth])[
  Programs are meant to be read by humans and only incidentally for machines to execute.
] <quote-knuth>

#quote(block: true, attribution: [Grace Hopper])[
  The most dangerous phrase in the language is: we've always done it this way.
]

Nested attribution chain in prose only — the quote above must remain a single QuoteNode.

== Lists — bullet, nested, numbered <sec-lists>

#list(
  tight: false,
  [Root item Alpha with tag [LI-A]],
  [Root item Beta with nested list:
    #list(
      [Nested Beta-1],
      [Nested Beta-2 with tag [LI-B2]],
    )
  ],
  [Root item Gamma],
)

+ Numbered one with tag [EN-1]
+ Numbered two
  + Nested numbered two-dot-one
  + Nested numbered two-dot-two with tag [EN-2.2]
+ Numbered three

== Code blocks <sec-code>

```rust
fn checksum(data: &[u8]) -> u32 {
    data.iter().map(|b| *b as u32).sum()
}
```

```typ
#let pipeline(doc) = doc
  .with_chunker(SimpleHeadingChunker)
  .chunk()
```

Inline `let x = 1` must not produce a CodeNode — only block fences above.

== Equations <sec-math>

$ phi.alt := (1 + sqrt(5)) / 2 $ <eq-golden>

$ sum_(k=1)^n k = (n(n+1)) / 2 $ <eq-sum>

Block equation with label for cross-reference below.

== Figures, tables, and grids <sec-figures>

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Signal], [Unit], [Range]),
    [Pressure], [bar], [0–16],
    [Flow], [L/min], [0–500],
    [Temp], [°C], [-40–120],
  ),
  caption: [Instrument signal ranges for the semantic corpus.],
) <tab-signals>

#figure(
  grid(
    columns: 3,
    grid.header([Zone], [Role], [Load]),
    [A], [Acquisition], [High],
    [B], [Processing], [Medium],
    [C], [HMI], [Low],
  ),
  caption: [Zone layout grid — layout only, not a semantic table.],
) <fig-grid-zones>

#figure(
  rect(width: 5cm, height: 2.5cm, fill: green.lighten(85%), stroke: 1pt + green),
  caption: [Placeholder process diagram.],
) <fig-diagram>

== Cross-references <sec-xrefs>

#cnd.metadata.update(it => it + (section: "xrefs"))

Closing paragraph tying references together: golden ratio @eq-golden, summation @eq-sum, signal table @tab-signals, zone grid @fig-grid-zones, and diagram @fig-diagram. Tag [XREF-CLOSE].

= Standalone table annex <ch-annex>

No figure wrapper — table node without caption.

#table(
  columns: (auto, auto),
  table.header([Key], [Value]),
  [fixture], [complex_semantic],
  [version], [hard-1],
)

Annex paragraph referencing signal table @tab-signals again from standalone context. Tag [ANNEX-END].
