// A float helper defined in the preamble and called later must not claim
// every paragraph written in between — one of them is where another float,
// written in another file, lands.
#set document(title: "Float helper from the preamble", author: "typst-cnd")
#import "_box_float_text_lib.typ": disclaimer
#let aside(x) = place(bottom, float: true)[Aside: #x]
#set page(width: 160pt, height: 120pt, margin: 10pt)
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#disclaimer()
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#aside[later note AZ]
