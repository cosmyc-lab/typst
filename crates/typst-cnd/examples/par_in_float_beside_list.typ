// Regression: a paragraph in a floating `place`, drawn while a long list
// written elsewhere is still open, is not that list's content.
#set document(title: "Float paragraph beside a long list", author: "typst-cnd")
#set page(height: 10cm)
Intro paragraph. #place(bottom, float: true)[FPB1 bottom float paragraph]

- PA0 start
#for i in range(1, 30) [
- PA#i item
]
