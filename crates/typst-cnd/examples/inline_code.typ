#set text(lang: "en")
#set heading(numbering: "1.")
#set document(
  title: "Inline Code Test",
  author: ("Test",),
  date: datetime(year: 2026, month: 6, day: 26),
)

= Inline code in paragraphs

Use the `typst-cnd` crate to compile documents.

Call `extract_text` instead of `plain_text` to avoid tripling.

The pattern `foo` appears once per mention.

= Inline code in lists

- Install `typst-cnd` as a dependency.
- Run `cargo test` to verify.
- Check that `inline code` is not duplicated.

= Inline code in quotes

#quote(block: true, attribution: [Author])[
  The function `extract_text` fixes the tripling bug.
]
