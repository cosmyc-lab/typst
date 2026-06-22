#set text(lang: "fr")
#set heading(numbering: "1")
#set figure(numbering: "1")
#set document(
  title: "Graphe de références croisées — test CND",
  author: ("L. Faure",),
  date: datetime(year: 2026, month: 6, day: 21),
  keywords: ("cross-refs", "labels", "cnd"),
  description: "Réseau dense de références entre titres, paragraphes et tables.",
)

= Vue d'ensemble <ch-overview>

Ce chapitre introduit les liens vers @sec-detail, @tab-signaux et @sec-annexe.

== Détail technique <sec-detail>

Les signaux instrumentés sont décrits dans @tab-signaux.

Le paragraphe suivant renvoie aussi vers la section @sec-annexe pour le récapitulatif.

Les boucles @tab-boucles et @tab-signaux doivent rester alignées sur le P&ID.

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Tag], [Type], [Unité]),
    [PT-101], [Pression], [bar],
    [TT-201], [Température], [°C],
  ),
  caption: [Signaux de terrain référencés.],
) <tab-signaux>

== Régulation <sec-regulation>

#cnd.metadata.update(it => it + (domain: "regulation"))

Voir @tab-boucles pour les paramètres PID actifs.

#figure(
  table(
    columns: (auto, auto),
    table.header([Boucle], [Kp]),
    [LC-101], [1.10],
    [TC-202], [0.95],
  ),
  caption: [Boucles PID actives.],
) <tab-boucles>

= Annexe <sec-annexe>

Références retour : @sec-detail, @tab-signaux, @tab-boucles et @ch-overview.

Le tableau @tab-recap consolide les tags cités plus haut.

#figure(
  table(
    columns: (auto, auto),
    table.header([Tag], [Bus]),
    [PT-101], [Profibus],
    [TT-201], [Modbus],
  ),
  caption: [Récapitulatif bus de terrain.],
) <tab-recap>
