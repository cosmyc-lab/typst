#set document(title: "Citations", author: "typst-cnd")

= Related work

Pipelines drift without manifests @smith2024. A prose citation:
#cite(<jones2022>, form: "prose") argues the same. A page-specific
reference @smith2024[p. 104] appears here, and a suppressed citation
#cite(<jones2022>, form: none) contributes no marker.

A multi-citation group @smith2024 @jones2022 closes the section.

#bibliography("refs.yml", style: "ieee")
