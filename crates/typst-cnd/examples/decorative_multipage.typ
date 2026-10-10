// A long decorative block spans several pages, and a kept float written
// after it lands at the top of a page while the artifact is still open in
// the frames: it is not decorative. Content written inside a decorative
// float but shown in the flow — a `metadata` value queried, a `state` read
// in `context` — is kept where it is shown.
#set document(title: "Decorative multipage", author: "typst-cnd tests")
#set page(width: 200pt, height: 200pt)
#let s = state("s", none)

= Kept heading

Kept intro.

#pdf.artifact(place(top, float: true)[Ornament float #metadata([Kept via metadata query.]) <m> #s.update([Kept via state.])])

#context query(<m>).first().value

#context s.get()

#pdf.artifact[#for i in range(25) [Ornament line #i.

]]
#place(top, float: true)[Kept float paragraph written after.]

Kept end.
