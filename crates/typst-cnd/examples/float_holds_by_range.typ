// A paragraph written in another file is not a float's body merely because
// the float repeats its words: the second note quotes the included chapter in
// full, and the first note, landing in that chapter, must survive.
#set document(title: "Float holds by range", author: "typst-cnd")
#import "_float_holds_lib.typ": note
#set page(width: 160pt, height: 120pt, margin: 10pt)
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#note[Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so.]
#include "_float_holds_chapter.typ"

#note[Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.]
