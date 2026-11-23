// A template applied with `show:` sets the document's language before the
// content it lays out, even when most of the text is in another one.
#let conf(doc) = {
  set document(title: "Language set by a template", author: "typst-cnd")
  set text(lang: "de")
  doc
}
#show: conf

Ein kurzer Satz.

#[
  #set text(lang: "en")
  A long English paragraph, with many more words than the German one.
]
