// A float held by another float only by its words is still a float's body:
// the preamble helper's range reaches over the long paragraph, and its
// body quotes the chapter's note. That note, landing in the long paragraph,
// must survive.
#set document(title: "Float held by text", author: "typst-cnd")
#set page(width: 160pt, height: 120pt, margin: 10pt)
#let aside(x) = place(bottom, float: true)[#x]
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#include "_float_text_chapter.typ"
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#aside[Quoted: Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so.]
