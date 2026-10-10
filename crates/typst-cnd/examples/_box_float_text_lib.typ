// Helpers for `box_float_text.typ`: floats written in a file of their own, so
// that their spans are in another file than the paragraph they land in.
#let disclaimer() = place(top, float: true)[Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so.]
#let stamp() = place(top, float: true, block(height: 60pt)[Draft])
#let twopar() = place(top, float: true)[Kept one QA. Kept one QA. Kept one QA.

Kept two QB. Kept two QB.]
#let refnote() = place(top, float: true)[See @sec for QR details in full. See @sec for QR details in full. See @sec for QR details in full. See @sec for QR details in full. See @sec for QR details in full.]
