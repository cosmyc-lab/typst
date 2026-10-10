// As in `float_held_by_text_whole.typ`, but the helper's body also holds
// text written in the preamble, so the body itself reaches, in the source,
// over the long paragraph quoted at the call.
#set document(title: "Float held by text, prefixed", author: "typst-cnd")
#set page(width: 160pt, height: 120pt, margin: 10pt)
#let aside(x) = place(bottom, float: true)[Aside: #x]
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#include "_float_text_chapter.typ"
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#aside[Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.]
