// A term list, a block equation and a block raw in an inline `box` are
// laid out in the line of the paragraph that holds them, which renders
// their text: none is a node of its own.
#set document(title: "Boxed blocks", author: "typst-cnd")
#set text(lang: "en")

= Boxed blocks

A sentence holds #box[/ BT1 term: BT2 description] inline.

A formula #box[$ x + y $] sits inline.

A listing #box[```
BR1 boxed raw
```] sits inline.

/ Flow term: Flow description.

$ a + b $

```
BR2 flow raw
```
