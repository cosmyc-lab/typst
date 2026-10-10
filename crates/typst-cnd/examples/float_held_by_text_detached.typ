// As in `float_held_by_text.typ`, but the note's body is a string passed
// through `upper`, with no source range of its own: only its words tie it to
// its float, and the preamble helper's float quotes those words too.
#set document(title: "Float held by text, detached", author: "typst-cnd")
#set page(width: 160pt, height: 120pt, margin: 10pt)
#let note(s) = place(top, float: true, upper(s))
#let aside(x) = place(bottom, float: true)[#x]
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#note("YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO.")
Long paragraph that spans pages: one two three four five six seven eight nine ten. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#aside[Quoted: YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO. YES INDEED IT IS SO.]
