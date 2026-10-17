// A quote whose body uses content defined earlier by a `#let` keeps its
// attribution apart from its body. Footnote entries numbered in roman
// numerals, after a counter update, or shown by a custom entry rule all
// keep their text without their number.
#set document(title: "Quote attribution with reused content", author: "typst-cnd")
#let intro = [HI1 intro shared.]
Before.

#quote(block: true, attribution: [HA1 Ada])[#intro HB1 body.]

#quote(block: true, attribution: [HA2 Bob])[HB2 first.

HB3 second paragraph.]

#quote(block: true, attribution: [HA3 Cy])[#pdf.artifact[DQ1 decorative] HB4 kept.]

#set footnote(numbering: "i")
Roman.#footnote[FR1 roman note.] More.#footnote[FR2 second.]

#counter(footnote).update(9)
Ten.#footnote[FT1 tenth note.]

#show footnote.entry: it => [#it.note.body]
Custom.#footnote[FC1 custom entry.]
