// A table's kind comes from the author's hint only (`cnd.table`): "data",
// "content", any other string, or nothing at all. A non-string hint says
// nothing, and the hint never shows up in any node's metadata.
#import "vendor/cnd.typ"
#set document(title: "Table kinds", author: "typst-cnd")

= Tables

#cnd.metadata.update(it => it + (section: "tables"))

Readings from the test bench:

#cnd.table(kind: "data", table(
  columns: 2,
  table.header([Sensor], [Reading]),
  [A], [38],
  [B], [27],
))

A comparison to read:

#cnd.table(kind: "content", table(
  columns: 2,
  table.header([Plan], [Includes]),
  [Basic], [Search and export],
  [Team], [Shared folders],
))

A table nobody described:

#table(columns: 2, [North], [South])

A kind outside the known values:

#cnd.table(kind: "matrix", table(columns: 2, [1], [0], [0], [1]))

A hint that is not a string:

#cnd.metadata.update(it => it + ("table-kind": 3))
#table(columns: 1, [Numbered hint])
#cnd.metadata.update(it => {
  let d = it
  let _ = d.remove("table-kind", default: none)
  d
})

A data table in a figure:

#figure(
  cnd.table(kind: "data", table(columns: 2, [Year], [Count], [2024], [12])),
  caption: [Yearly counts],
) <tab-counts>

The yearly counts are in @tab-counts.
