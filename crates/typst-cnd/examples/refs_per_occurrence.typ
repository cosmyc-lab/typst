// Every reference marker is an edge of its own: a text naming one target
// several times, by a reference or by a link, has one edge per marker, each
// with the span of its own marker.
#set document(title: "One edge per reference", author: "typst-cnd")
#set heading(numbering: "1.")

= Target <a>

See #link(<a>)[the target] and @a, then @a once more.

- An item with #link(<a>)[label link] and @a.

== Back to @a and @a
