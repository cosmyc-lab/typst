// An inline box's body is laid out as a paragraph of its own, inside the line
// of the paragraph that holds it. Its text must appear once, whatever file
// its spans come from — and a floated body must still appear.
#set document(title: "Inline box identity", author: "typst-cnd")
#import "_box_identity_lib.typ": pill, see
#set heading(numbering: "1.")

= Detached spans <detached>

// A string turned into content by a styling function has a detached span.
#let a(s) = box(text(s))
#let b(s) = box(text(fill: red, s))
#let c(s) = box(text(fill: red)[#s])
#let d(s) = box(text(size: 8pt, s))
#let e(s) = box(upper(s))
#let f(s) = box(s)
#let g(s) = box(text(weight: 700, upper(s)))
#let h(s) = box(strong(s))
#let i(s) = box([#s])
#let j(s) = box(emph(s))

QA1 x #a("QA2") y.

QB1 x #b("QB2") y.

QC1 x #c("QC2") y.

QD1 x #d("QD2") y.

QE1 x #e("QE2") y.

QF1 x #f("QF2") y.

QG1 x #g("QG2") y.

QH1 x #h("QH2") y.

QI1 x #i("QI2") y.

QJ1 x #j("QJ2") y.

#b("QK1 alone styled")

#a("QL1 alone plain")

#b[QM1 alone styled content]

= Another file <other>

#let local-pill(b) = box(fill: luma(230), b)

RA1 inline imported pill #pill[RA2 pill] tail.

RB1 inline local pill #local-pill[RB2 pill] tail.

#pill[RC1 imported pill alone]

#local-pill[RD1 local pill alone]

RE1 see #pill[RE2 as in @detached] tail.

RF1 then #see(<detached>) tail.

#set page(width: 170pt, height: 70pt, margin: 10pt)
SA1 start of a paragraph #box[SA2 boxed] text.
#place(bottom, float: true)[SA3 FLOATED BODY]
SA4 Filler prose after the float, long enough to push the float onto a page of its own and to keep the two paragraphs apart and more and more words.
