#set text(lang: "fr")
#set heading(numbering: "1")
#set figure(numbering: "1")
#set document(
  title: "Manuel d'exploitation DCS — Fixture intégration CND",
  author: ("Équipe CND", "L. Faure"),
  date: datetime(year: 2026, month: 6, day: 6),
  keywords: ("DCS", "chunking", "cross-refs", "tables", "fixture"),
  description: "Manifest de référence couvrant les cas limites du chunker : contenu pré-heading, imbrication profonde, refs intra/inter-chunks, tables complexes.",
)

Résumé exécutif placé avant le premier titre. Ce paragraphe doit produire un chunk isolé avec un heading_path vide.

= Architecture générale <ch-architecture>

#cnd.metadata.update(it => it + (revision: "4.2", status: "approved"))

Le système DCS repose sur trois couches fonctionnelles : acquisition, traitement et supervision.

== Couche d'acquisition <sec-acquisition>

Les capteurs analogiques et numériques sont regroupés par bus de terrain. Voir le tableau des plages de mesure.

La fréquence d'échantillonnage nominale est de 10 Hz pour les boucles rapides.

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Capteur], [], [Plage]),
    table.cell(rowspan: 2)[Pression], [PT-101], [0–16 bar],
    [PT-102], [0–25 bar],
    [Débit], [FT-201], [0–500 L/min],
  ),
  caption: [Plages de mesure des capteurs de terrain.],
) <tab-plages-mesure>

== Couche de traitement <sec-traitement>

Les algorithmes de régulation s'exécutent sur le contrôleur redondant. Les plages capteurs (@tab-plages-mesure) et le récapitulatif annexe (@tab-recap-signaux) doivent rester cohérents.

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Boucle], [Mode], [Kp]),
    [LC-101], [AUTO], [1.25],
    [FC-202], [CASCADE], [0.80],
  ),
  caption: [Boucles de régulation PID actives.],
) <tab-boucles-regulation>

= Exploitation <ch-exploitation>

== Surveillance opérateur

=== Gestion des alarmes <sec-alarmes>

Les alarmes sont classées en quatre niveaux de criticité : info, avertissement, alarme, critique.

Le délai d'acquittement maximal est défini par la politique site. Consulter la matrice des délais.

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Niveau], [], [Délai max]),
    [Info], [—], [24 h],
    [Critique], [P1], [5 min],
  ),
  caption: [Délais d'acquittement par niveau de criticité.],
) <tab-delais-alarmes>

== Maintenance préventive <sec-maintenance>

Les interventions planifiées suivent le calendrier OEM. Aucune table n'est associée à cette section.

= Annexes <ch-annexes>

#figure(
  table(
    columns: (auto, auto, auto),
    table.header([Tag], [Unité], [Bus]),
    [PT-101], [bar], [Profibus],
    [FT-201], [L/min], [Modbus],
  ),
  caption: [Récapitulatif des signaux instrumentés.],
) <tab-recap-signaux>
