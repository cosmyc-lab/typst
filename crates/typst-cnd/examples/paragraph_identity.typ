// Paragraphs are deduplicated by element identity, not by comparing texts:
// a paragraph whose text happens to start like a heading, a list item or a
// callout title is still the author's own paragraph. Inline PDF artifacts
// are decoration and never part of a paragraph's text.
#set document(title: "Paragraph identity", author: "typst-cnd tests")

= Decision

Decision taken by the team, without a spec.

- First item of a list that is long enough

First item of a list that is long enough, followed by more text.

= Approach

A

#block[#strong[Decision] #linebreak() We keep option 3.]

Before #pdf.artifact[★] after.

#pdf.artifact[decoration]

Closing paragraph of the document.

// A quote made a block quote by a set rule is emitted as its own node, even
// when a show rule boxes it into the surrounding paragraph.
#show quote: it => box(it.body)
#set quote(block: true)

Lead words #quote[Quoted aside] trailing words.
