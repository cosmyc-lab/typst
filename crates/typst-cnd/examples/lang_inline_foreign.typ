// A foreign phrase set with `text(lang:)` inside a paragraph is local: it
// does not make the document's language. Nothing sets one, so the document
// is in the language most of its text is set in, Typst's default here.
#set document(title: "Inline foreign phrase", author: "typst-cnd")

Hello world paragraph.

With #text(lang: "de")[ein Wort] inline.
