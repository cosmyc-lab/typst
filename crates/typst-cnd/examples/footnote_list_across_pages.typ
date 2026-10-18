// The same broken note on 6 cm pages: here the note's list itself breaks,
// its second item printed on the next page while a flow list is open across
// the same break. That item is still the second item of the note's list,
// and the flow list keeps its own nine items.
#set document(title: "Footnote list across pages", author: "typst-cnd")
#set page(height: 6cm)

BA1 body line one.

BA2 body line two carries a long note.#footnote[SN1 first note paragraph is long enough to take a line or two of the area.

  SN2 second note paragraph is long enough to take a line or two as well.

  SN3 third note paragraph keeps going to fill the space.

  SN4 fourth note paragraph takes room at the bottom of the page.

  - SN5 note item one
  - SN6 note item two

  SN7 last note paragraph ends the note.]

- LA1 flow item one
- LA2 flow item two
- LA3 flow item three
- LA4 flow item four
- LA5 flow item five
- LA6 flow item six
- LA7 flow item seven
- LA8 flow item eight
- LA9 flow item nine

BA3 after the list.
