// Lists and term lists whose bodies are blocks: an item's paragraphs and
// nested lists are its children, and a marker written inside an item sits
// on that item's own paragraph.
#set document(title: "Containers: lists", author: "typst-cnd")
#set terms(separator: [: ])

= Lists

- First item.
- Second item, first paragraph.

  Second item, second paragraph.

  #set enum(start: 5)
  + Nested five.
  + Nested six.
- Third item ends with a note.#footnote[A note on the third item.]

+ Enum item with bullets:
  - Inner bullet one.
  - Inner bullet two.
+ Second enum item.

/ Latency: The time a request waits.

  It is measured end to end @smith2024.

/ Throughput: The work done per second.

#bibliography("refs.yml", style: "ieee")
