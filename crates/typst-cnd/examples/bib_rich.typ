// A bibliography entry's formatted reference is rich text: the italics a
// citation style puts on a journal title become an emph mark. The document
// sets no language, so its language is the default one.
#set document(title: "Rich bibliography", author: "typst-cnd")

= Sources

The pipeline design follows earlier work @smith2024.

/ Prior art @jones2022: A book on the subject.

#bibliography("refs.yml", style: "apa")
