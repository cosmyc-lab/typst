// The language set before any content is the document's, even when most of
// the text is in another one: that text carries its own language.
#set document(title: "Language set first", author: "typst-cnd")
#set text(lang: "fr")

= Titre

#[
  #set text(lang: "en")
  A long English paragraph, with many more words than the French heading.

  A second English paragraph, quite long as well.
]
