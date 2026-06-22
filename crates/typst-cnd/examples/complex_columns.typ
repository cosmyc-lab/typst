#set text(lang: "fr")
#set heading(numbering: "1")
#set figure(numbering: "1")
#set document(
  title: "Layout multicolonne — test CND",
  author: ("Équipe layout",),
  date: datetime(year: 2026, month: 6, day: 23),
  keywords: ("columns", "multicol", "reading-order", "flatten"),
  description: "Colonnes block, colonnes page, imbrication et table dans une colonne — ordre de lecture attendu.",
)

[INTRO] Introduction pleine largeur avant tout bloc multicolonne.

= Colonnes block <ch-block-cols>

== Trois colonnes avec colbreaks <sec-three-cols>

#columns(3, gutter: 8pt)[
  #cnd.metadata.update(it => it + (track: "left"))

  [L1] Colonne gauche — paragraphe 1.

  [L2] Colonne gauche — paragraphe 2, plus long pour occuper verticalement la première colonne et dépasser visuellement le début de la colonne centrale.

  #colbreak()

  [M1] Colonne centrale — paragraphe 1.

  [M2] Colonne centrale — paragraphe 2.

  #colbreak()

  [R1] Colonne droite — paragraphe 1.

  #figure(
    table(
      columns: (auto, auto),
      table.header([Bus], [Débit]),
      [Profibus], [120 L/min],
      [Modbus], [80 L/min],
    ),
    caption: [Débits bus dans la colonne droite.],
  ) <tab-bus-col>
]

== Colonnes imbriquées <sec-nested-cols>

#columns(2, gutter: 10pt)[
  [OUT-L] Colonne externe gauche.

  #columns(2, gutter: 4pt)[
    [IN-L1] Sous-colonne interne gauche.

    #colbreak()

    [IN-R1] Sous-colonne interne droite.
  ]

  #colbreak()

  [OUT-R] Colonne externe droite après le bloc imbriqué.
]

= Flux page colonnes <ch-page-cols>

#pagebreak()
#set page(columns: 2, height: 130mm, margin: (x: 1.4cm, y: 1.2cm))

[PC1] Page-colonnes — paragraphe 1.

[PC2] Page-colonnes — paragraphe 2.

[PC3] Page-colonnes — paragraphe 3.

[PC4] Page-colonnes — paragraphe 4.

[PC5] Page-colonnes — paragraphe 5.

[PC6] Page-colonnes — paragraphe 6.

[PC7] Page-colonnes — paragraphe 7.

[PC8] Page-colonnes — paragraphe 8.

#pagebreak()
#set page(columns: 1)

= Retour mono-colonne <ch-single>

[POST] Paragraphe après retour en mono-colonne. Voir @tab-bus-col et @sec-three-cols.
