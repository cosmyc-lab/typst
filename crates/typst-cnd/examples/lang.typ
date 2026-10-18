// A text carries `lang` only where its language differs from the
// document's. A label text (a caption, a term, an attribution) takes the
// language its own glyphs are set in. A text in several languages takes the
// one most of its glyphs are in, and the first one met on a tie.
#set text(lang: "en")
#set document(title: "Languages", author: "typst-cnd")

= Languages

#[
  #set text(lang: "fr")
  #figure(rect(width: 1cm, height: 1cm), caption: [Une légende])

  / Terme: Une définition.

  #quote(block: true, attribution: [Un auteur])[Une citation.]
]

Mostly English words with #text(lang: "fr")[un mot] inside.

#text(lang: "fr")[abcd] #text(lang: "de")[abcd]
