// A float whose body uses content defined earlier in the file must not claim
// every paragraph written between that definition and the float — one of
// them is where another float, written in another file, lands.
#set document(title: "Float body from an earlier let", author: "typst-cnd")
#import "_box_float_text_lib.typ": disclaimer
#let wide = [Wide note WZ.]
#set page(width: 160pt, height: 120pt, margin: 10pt)
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#disclaimer()
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#place(bottom, float: true)[#wide]
