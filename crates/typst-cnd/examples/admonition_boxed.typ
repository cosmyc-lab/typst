// Callouts in an inline `box`, which the paragraph around the box renders
// whole, and callouts written inline, whose body is laid out on its own.
#import "vendor/cnd.typ"
#set document(title: "Boxed callouts", author: "typst-cnd")

Lead #box[#cnd.admonition(title: [Boxed title])[Boxed body.]] tail.

Second #box[#cnd.admonition(title: [Boxed list])[Para in box.

- box item]] end.

Third #box[#cnd.admonition[Untitled boxed body.]] end.

Lead text #cnd.admonition(title: [Inline title])[Inline body.] trailing text.

- Item text #cnd.admonition(kind: "tip", title: [Item title])[Item body.]
- Second item.

After.
