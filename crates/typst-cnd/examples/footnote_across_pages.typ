// A footnote too long for its page is broken: the rest of it is printed at
// the bottom of the next page, with no entry around it. That rest is still
// the note's content, on the page where it is printed, even while a list of
// the flow is open across the same page break.
#set document(title: "Footnote across pages", author: "typst-cnd")
#set page(height: 5.5cm)

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
