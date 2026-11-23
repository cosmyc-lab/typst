// Nothing sets the document's language before its content, so it is the
// language most of its text is set in: French here, although the first
// language set (on a German word) is not. The English paragraph carries its
// own language, since it differs from the document's.
#set document(title: "Majority language", author: "typst-cnd")

An English opening with one #text(lang: "de")[Wort].

#[
  #set text(lang: "fr")
  Un long paragraphe en français, avec bien plus de mots que l'ouverture.

  Un second paragraphe en français, lui aussi assez long.
]
