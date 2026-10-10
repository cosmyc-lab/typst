// Regression: a list inside a float or a footnote is never lost, and
// never emitted twice.
#set document(title: "Floats beside short lists", author: "typst-cnd")
#set page(height: 10cm)
Intro paragraph. #place(bottom, float: true, figure(block[- FLB1 bottom float list
- FLB2 bottom float list], caption: [FCB bottom caption]))

- PA0 start
#for i in range(1, 3) [
- PA#i item
]

Middle paragraph.

#place(top, float: true, figure(block[- FLT1 top float list], caption: [FCT top caption]))
- QA0 start
#for i in range(1, 3) [
- QA#i item
]
