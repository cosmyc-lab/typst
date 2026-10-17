// Hand-written `<cnd-document>` markers: a later marker overrides an earlier
// key, a non-string `extra` value is dropped, and a key naming a document
// field is dropped.
#set document(title: "Marker rules", author: "typst-cnd")

#[#metadata((status: "Draft", version: "0.1", extra: (client: "ACME", team: "Docs"))) <cnd-document>]
#[#metadata((status: "Final", version: none, extra: (client: "Globex", count: 3, title: "Hijack", lang: "fr"))) <cnd-document>]

A paragraph.
