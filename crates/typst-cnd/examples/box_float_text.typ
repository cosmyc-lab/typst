// A float written in another file is deferred into a later paragraph that is
// still open. Its text must survive even when that paragraph happens to
// contain the same sentence, or a longer word that starts with it.
#set document(title: "Floats beside matching text", author: "typst-cnd")
#import "_box_float_text_lib.typ": disclaimer, stamp
#set page(width: 160pt, height: 120pt, margin: 10pt)
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#disclaimer()
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#pagebreak()
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#stamp()
Long paragraph that spans pages about the drafting rules: one two three four five six seven eight nine ten Drafting eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.
