// Helpers for `box_float_text.typ`: floats written in a file of their own, so
// that their spans are in another file than the paragraph they land in.
#let disclaimer() = place(top, float: true)[Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so. Yes indeed it is so.]
#let stamp() = place(top, float: true, block(height: 60pt)[Draft])
