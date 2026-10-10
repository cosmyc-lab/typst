// Regression: the same as `list_in_bare_float.typ`, with the floated list
// wrapped in a layout-only `block`.
#set document(title: "Block-wrapped float beside a long list", author: "typst-cnd")
#set page(height: 10cm)
Intro paragraph. #place(bottom, float: true, block[- FLB1 bottom float list])

- PA0 start
#for i in range(1, 30) [
- PA#i item
]
