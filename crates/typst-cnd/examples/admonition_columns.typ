// A callout flowing through two columns and across pages with a running
// header and footer, and a footnote in its body.
#import "vendor/cnd.typ"
#set document(title: "Callout in columns", author: "typst-cnd")
#set page(
  height: 8cm,
  columns: 2,
  header: [Running header text],
  footer: context [Page #counter(page).display()],
)

Opening.

#cnd.admonition(kind: "note", title: [Flowing])[
  #for i in range(1, 25) [
    Body #i of the callout#if i == 5 [#footnote[Note five.]].

  ]
]

Closing.
