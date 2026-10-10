// Helpers for `box_identity.typ`, defined in a file of their own so that the
// box body's spans come from another file than the paragraph around it.
#let pill(b) = box(fill: luma(230), b)
// A box whose own text, written here, holds a reference: its realized text
// ("Section 1") is not in the unrealized paragraph around it.
#let see(target) = box[RF2 see #ref(target)]
