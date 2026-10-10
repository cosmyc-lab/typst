// Raw blocks inside figures, plain and under show rules that wrap them in a
// block, with and without `context`.
#set document(title: "Figure raw", author: "typst-cnd")
#figure(```yaml
ra1: 1
```, caption: [RA2 caption])

#show raw.where(block: true): it => context block(fill: luma(230), it)
#figure(```yaml
rb1: 1
```, caption: [RB2 caption styled])

#show raw.where(block: true): it => block(fill: luma(230), it)
#figure(```yaml
rc1: 1
```, caption: [RC2 caption styled no context])
