// Regression: a list written straight inside a floating `place` (no figure
// around it) is drawn wherever the float lands, here while a long list
// written elsewhere is still open. It is kept once, with its own nesting.
#set document(title: "Bare floats beside long lists", author: "typst-cnd")
#set page(height: 10cm)
Intro paragraph. #place(bottom, float: true)[- FLB1 bottom float list
- FLB2 bottom float list]

- PA0 start
#for i in range(1, 30) [
- PA#i item
]

Middle paragraph.

#place(top, float: true)[
  - FLT1 top float list
    - FLT2 nested float child
]
- QA0 start
#for i in range(1, 30) [
- QA#i item
]
