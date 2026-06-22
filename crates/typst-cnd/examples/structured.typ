#set text(lang: "fr")
#set heading(numbering: "1.")
#set figure(numbering: "1")
#set document(
  title: "Spécification Technique DCS v4.2",
  author: ("L. Faure", "P. Simon"),
  date: datetime(year: 2026, month: 5, day: 31),
  keywords: ("DCS", "automatisme"),
  description: "Document de test avec table et références croisées.",
)

= Description du système

== Paramètres nominaux <sec-params>

#cnd.metadata.update(it => it + (revision: "4.2"))

Le système est composé de quatre modules principaux. Voir @tab-params-nominaux pour le détail.

= Annexes

#figure(
  table(
    columns: (auto, auto),
    table.header([Paramètre], [Valeur]),
    [Débit nominal], [120 L/min],
    [Pression max], [10 bar],
  ),
  caption: [Paramètres nominaux de fonctionnement.],
) <tab-params-nominaux>
