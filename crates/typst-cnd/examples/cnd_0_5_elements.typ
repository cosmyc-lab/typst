// One document that exercises every node type and every mark kind of the
// 0.5 format, and puts every edge family (reference, citation, footnote,
// link) inside a list item, a table cell and a footnote.
#import "vendor/cnd.typ"
#set text(lang: "en")
#set document(title: "Every element", author: "typst-cnd")

#cnd.document(status: "Approved", version: "1.0", extra: (client: "ACME"))

= Every element <sec-all>

Plain prose with *strong*, _emphasis_, `inline code`, #strike[struck],
#underline[underlined], #highlight[highlighted], 4 m#super[2], H#sub[2]O
and the formula $v = d/t$, then a note.#footnote[A plain note.]

#quote(block: true, attribution: [An author])[A quoted sentence.]

#cnd.code(title: "main.py")[```python
print("hello")
```]

$ E = m c^2 $

#cnd.admonition(kind: "warning", title: [Careful])[Mind the gap.]

#figure(
  image("newsletter/newsletter-cover.png", width: 3cm),
  alt: "A cover illustration",
  caption: [The cover],
) <fig-cover>

== Lists, terms and tables

- A list item with a reference @fig-cover, a citation @smith2024, a note#footnote[Noted in an item.] and a #link("https://example.com")[link].

/ Term: Its definition.

#cnd.table(kind: "data", table(
  columns: 2,
  table.header([Name], [Detail]),
  [A], [A cell with a reference @fig-cover, a citation @smith2024, a note#footnote[Noted in a cell.] and a #link("https://example.com")[link].],
))

#grid(columns: 2, gutter: 8pt, [Left column.], [Right column.])

A note that carries the other families.#footnote[It cites @jones2022, points to @fig-cover and links to #link("https://example.org")[a site].]

#bibliography("refs.yml", style: "ieee")
