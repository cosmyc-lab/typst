#set text(lang: "fr")
#set heading(numbering: "1")
#set figure(numbering: "1")
#set math.equation(numbering: "1")
#set page(columns: 2, height: 130mm, margin: (x: 1.4cm, y: 1.2cm))
#set document(
  title: "Hardcore mixed layout + semantics",
  author: ("Layout QA",),
  date: datetime(year: 2026, month: 6, day: 20),
  keywords: ("columns", "semantic", "stress", "reading-order"),
  description: "Page columns + block columns + quotes/code/math/lists in one document.",
)

[HC-0] Full-width intro before page columns take effect on following pages.

#pagebreak()

= Page columns + semantics <ch-hardcore>

== Block columns with embedded blocks <sec-cols-blocks>

#columns(2, gutter: 8pt)[
  #cnd.metadata.update(it => it + (track: "left"))

  [HC-L1] Left column paragraph one.

  #quote(attribution: [Source L])[
    Quote trapped in left column — must be QuoteNode not Paragraph.
  ]

  ```python
  left_col = {"role": "acquisition"}
  ```

  #colbreak()

  [HC-R1] Right column paragraph one.

  $ E = m c^2 $ <eq-hc-energy>

  #list(
    [Right bullet A tag [HC-RB-A]],
    [Right bullet B],
  )

  [HC-R2] Right column paragraph two after list.
]

== Nested columns + table <sec-nested-table>

#columns(2)[
  [HC-NL] Outer left.

  #columns(2)[
    [HC-IL] Inner left tag [HC-IL].

    #colbreak()

    [HC-IR] Inner right tag [HC-IR].
  ]

  #colbreak()

  #figure(
    table(
      columns: (auto, auto),
      table.header([Tag], [State]),
      [T-101], [OK],
      [T-102], [WARN],
    ),
    caption: [Telemetry in nested column layout.],
  ) <tab-hc-nested>
]

== Reading order tail <sec-tail>

[TAIL-1] Single-column tail after column blocks — references @eq-hc-energy and @tab-hc-nested. Tag [TAIL-2].

#figure(
  grid(
    columns: 2,
    [X], [Y],
    [1], [2],
  ),
  caption: [Minimal grid after column stress.],
) <fig-hc-grid>

[TAIL-3] Final marker paragraph.
