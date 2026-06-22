#set text(lang: "fr")
#set heading(numbering: "1")
#set figure(numbering: "1")
#set page(numbering: "1")
#set document(
  title: "Rapport multipages — test CND",
  author: ("Équipe validation",),
  date: datetime(year: 2026, month: 6, day: 20),
  keywords: ("multipage", "pagination", "cnd"),
  description: "Document long avec sauts de page, titres répartis sur plusieurs pages et tables en fin de sections.",
)

Ce paragraphe d'introduction occupe la page 1 avant tout titre de chapitre.

= Contexte réglementaire <ch-contexte>

Le présent rapport couvre les exigences applicables au site industriel pilote.

== Cadre normatif <sec-normes>

Les normes IEC 61511 et ISO 13849 s'appliquent au périmètre automatisme.

=== Exigences SIL <sec-sil>

Les boucles de sécurité doivent être classées avant mise en service.

#pagebreak()

= Analyse fonctionnelle <ch-af>

== Description des fonctions <sec-fonctions>

Chaque fonction est décrite par son objectif, ses entrées et ses sorties.

=== Fonction F-101 <sec-f101>

La fonction F-101 assure la régulation de pression sur le réacteur R-101.

#cnd.metadata.update(it => it + (zone: "production", criticality: "high"))

Le paramétrage nominal est récapitulé dans @tab-f101-params.

#figure(
  table(
    columns: (auto, auto),
    table.header([Paramètre], [Valeur]),
    [Consigne], [8.5 bar],
    [Tolérance], [±0.2 bar],
  ),
  caption: [Paramètres nominaux F-101.],
) <tab-f101-params>

=== Fonction F-102 <sec-f102>

La fonction F-102 gère l'alarme haute pression en cascade avec F-101.

#pagebreak()

= Synthèse <ch-synthese>

== Points de vigilance <sec-vigilance>

Les écarts constatés lors des essais FAT sont listés ci-dessous.

Les sections @sec-f101 et @sec-f102 doivent être relues avant la recette.

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Réf.], [Écart], [Statut]),
    [E-01], [Dérive capteur], [Ouvert],
    [E-02], [Latence bus], [Clos],
  ),
  caption: [Synthèse des écarts FAT.],
) <tab-ecarts-fat>
