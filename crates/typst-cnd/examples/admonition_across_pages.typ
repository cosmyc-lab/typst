// A callout too long for its page: its body continues on the next ones.
#import "vendor/cnd.typ"
#set document(title: "Long callout", author: "typst-cnd")
#set page(height: 12cm)

Opening paragraph.

#cnd.admonition(kind: "note", title: [Long])[
  #for i in range(1, 31) [
    Body paragraph #i of the long callout.

  ]
]

Closing paragraph.
