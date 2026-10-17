// A footnote's body made of content defined earlier by a `#let`, or shown
// in the flow as well, is the note's content once and the flow's content
// once: nothing written between the definition and the note moves into it.
#set document(title: "Footnote with reused content", author: "typst-cnd")
#let source = [RS1 source: annual report.]
#let shared = [RS2 shared sentence used twice.]
#let disclaimer = [
  - RL1 shared list item
  - RL2 shared list item
]

= Section

RA1 first flow paragraph.

- RP1 flow item
- RP2 flow item

RA2 second flow paragraph.#footnote[#source See page 4.]

RA3 introduces a note.#footnote(shared)

#shared

RA4 introduces a list note.#footnote(disclaimer)

#disclaimer

RA5 after.
