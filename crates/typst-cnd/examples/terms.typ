#set document(title: "Definition lists", author: "typst-cnd")

= Glossary

A tight definition list (items directly following each other):

/ manifest: The serialized document tree.
/ pool: Out-of-tree referenceable entities.
/ node: A single element in the reading flow.

== Wide terms with nested content

A wide definition list (items separated by blank lines), where one
description carries a nested bullet list:

/ producer: A compiler that emits a CND manifest.

/ consumer: A reader of the manifest. Consumers include:
  - the search indexer
  - the agent orchestrator
  - export tooling

/ schema: The versioned contract both sides agree on.
