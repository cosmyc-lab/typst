// As in `float_held_by_text_whole.typ`, but the long paragraph is one piece
// of content, shown once in the flow and once more through the preamble
// helper. The flow showing is then built from the float body's own spans,
// and the chapter's note, held by its float only by its words, lands in it.
#set document(title: "Float held by text, reused content", author: "typst-cnd")
#set page(width: 160pt, height: 120pt, margin: 10pt)
#let aside(x) = place(bottom, float: true)[#x]
#let long = [Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.]
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#include "_float_text_chapter.typ"
#long

#aside(long)
