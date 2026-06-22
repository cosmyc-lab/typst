#set text(lang: "fr")
#set heading(numbering: "1")
#set figure(numbering: "1")
#set document(
  title: "Variantes de tables — test CND",
  author: ("P. Simon",),
  date: datetime(year: 2026, month: 6, day: 22),
  keywords: ("tables", "rowspan", "colspan", "standalone"),
  description: "Tables autonomes, figures avec fusion de cellules et contenus proches mais distincts.",
)

= Tables autonomes et figurées <ch-tables>

== Table sans figure <sec-standalone>

Le tableau suivant n'est pas encapsulé dans une figure mais porte un label.

#table(
  columns: (auto, auto),
  table.header([Clé], [Valeur]),
  [mode], [production],
  [revision], [3.1],
) <tab-config-standalone>

== Tables figurées similaires <sec-figures>

Deux figures consécutives avec des contenus proches mais distincts (test anti-doublon).

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Capteur], [], [Plage]),
    table.cell(rowspan: 2)[Pression], [PT-A], [0–10 bar],
    [PT-B], [0–16 bar],
    [Débit], [FT-A], [0–100 L/min],
  ),
  caption: [Plages capteurs — train A.],
) <tab-plages-a>

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Capteur], [], [Plage]),
    table.cell(rowspan: 2)[Pression], [PT-C], [0–10 bar],
    [PT-D], [0–16 bar],
    [Débit], [FT-B], [0–200 L/min],
  ),
  caption: [Plages capteurs — train B.],
) <tab-plages-b>

== Références <sec-refs>

Voir @tab-plages-a et @tab-plages-b.

La table autonomie `tab-config-standalone` n'est pas référencée via `@` (limitation Typst).

Les deux trains partagent une structure identique mais des tags différents.
