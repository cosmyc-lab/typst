#set document(title: "Hyperlinks", author: "typst-cnd")
#set heading(numbering: "1")

= Overview <sec-overview>

The nested case: #link("https://example.com")[@sec-overview and more text]
pins the marker pairing (ADR 0024).

= Other cases

A plain hyperlink to #link("https://example.com/docs") in running text.

A link to a label with a custom body: #link(<sec-overview>)[see the overview].

An empty-bodied link carries no rendered text: #link("https://example.com/cover")[].

A position link is not durable: #link((page: 1, x: 0pt, y: 0pt))[Go to top].

- A list item linking #link("https://example.com/list")[here].
