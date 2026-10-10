// Helper for `float_holds_by_range*.typ`: a float whose `place` is written in
// a file of its own, while its body comes from the call site.
#let note(x) = place(top, float: true)[#x]
