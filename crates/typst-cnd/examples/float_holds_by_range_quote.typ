// Like `float_holds_by_range.typ`, with a first note that says something
// else: the second note, which quotes the included chapter in full, is kept
// as well as the chapter.
#set document(title: "Float holds by range, quoted chapter", author: "typst-cnd")
#import "_float_holds_lib.typ": note
#set page(width: 160pt, height: 120pt, margin: 10pt)
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#note[Zed note here. Zed note here. Zed note here. Zed note here. Zed note here. Zed note here.]
#include "_float_holds_chapter.typ"

#note[Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.]
