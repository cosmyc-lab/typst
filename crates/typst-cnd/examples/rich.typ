#set document(title: "Rich CND Elements", author: "typst-cnd")
#set math.equation(numbering: "(1)")

= Rich content

A paragraph before structured blocks.

#quote(attribution: [Ada Lovelace])[
  Programs must be written for people to read.
]

#list(
  tight: false,
  [First item],
  [Second item with nested list],
)

+ Ordered alpha
+ Ordered beta

```python
def hello():
    return "world"
```

$ E = m c^2 $ <energy>

#figure(
  rect(width: 4cm, height: 2cm, fill: blue.lighten(80%)),
  caption: [A colored rectangle.],
) <rich-figure>

#figure(
  grid(
    columns: 2,
    [A], [B],
    [C], [D],
  ),
  caption: [A simple grid layout.],
) <rich-grid>

See @energy, @rich-figure, and @rich-grid.
