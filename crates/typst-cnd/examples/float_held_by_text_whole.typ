// As in `float_held_by_text.typ`, but the preamble helper's float quotes the
// whole long paragraph: that paragraph says nothing the float does not, yet
// it is not the float's body, and the chapter's note landing in it survives.
#set document(title: "Float held by text, whole quote", author: "typst-cnd")
#set page(width: 160pt, height: 120pt, margin: 10pt)
#let aside(x) = place(bottom, float: true)[#x]
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#include "_float_text_chapter.typ"
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#aside[Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.]
