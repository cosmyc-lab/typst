// A float written in another file is deferred into a later paragraph that is
// still open. Its text must survive even when that paragraph happens to
// contain the same sentence, or a longer word that starts with it — also when
// the float holds two paragraphs, or a reference that only realizes to text
// once laid out.
#set document(title: "Floats beside matching text", author: "typst-cnd")
#import "_box_float_text_lib.typ": disclaimer, refnote, stamp, twopar
#set heading(numbering: "1")
#set page(width: 160pt, height: 120pt, margin: 10pt)
= Floats <sec>
#pagebreak()
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#disclaimer()
Long paragraph that spans pages: one two three four five six seven eight nine ten. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#pagebreak()
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#stamp()
Long paragraph that spans pages about the drafting rules: one two three four five six seven eight nine ten Drafting eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#pagebreak()
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#twopar()
Long paragraph that spans pages: one two three four five six seven eight nine ten. Kept one QA. Kept one QA. Kept one QA. Kept two QB. Kept two QB. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.

#pagebreak()
Filler alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.
#refnote()
Long paragraph that spans pages: one two three four five six seven eight nine ten. See Section 1 for QR details in full. See Section 1 for QR details in full. See Section 1 for QR details in full. See Section 1 for QR details in full. See Section 1 for QR details in full. eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo twentythree twentyfour twentyfive twentysix twentyseven twentyeight twentynine thirty.
