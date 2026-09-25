# typst + typst-cnd — Project Context

Reference map for contributors and AI agents working on the **typst-cnd**
exporter inside this fork of the Typst compiler.

## What this fork is

A fork of the Typst compiler that adds **`typst-cnd`**, an exporter crate that
compiles `.typ` sources into **CND** JSON, a structured document format defined
by the CND standard and its SDK (`cnd-sdk`). Everything else is upstream Typst,
merged regularly.

| Piece | Location | Role |
|---|---|---|
| Typst compiler (upstream) | `crates/typst*`, `crates/typst-cli`, … | General-purpose typesetting |
| **typst-cnd** | `crates/typst-cnd/` | Typst → CND JSON exporter |
| CND standard and SDK | `cnd-sdk` (separate repository) | Format definition, validation, consumers |

**Do not** patch `typst-layout` / `typst-eval` unless a public API gap forces
it. Exporter logic lives in `crates/typst-cnd/`, a peer of `typst-html` and
`typst-pdf`; `typst-cli` only wires it in as an output format.

## Entry point

```plain text
Typst source (.typ)
      │  typst compile --format cnd (typst-cli + typst-cnd)
      ▼
CND (JSON)
      │  any CND consumer (see cnd-sdk)
      ▼
Validation, chunking, indexing, rendering, …
```

`typst-cnd` is the exporter library. The supported entry point is
`typst compile <file> --format cnd` (or an output path ending in `.cnd`),
which brings every standard CLI option (`--root`, `--font-path`,
`--ignore-system-fonts`, `--deps`) plus two generic additions:
`--inputs-file` (a JSON object of string values for `sys.inputs`) and
`--fallback-dir` (missing project files are looked up by file name there).
The standalone `typst-cnd` binary remains for existing callers.

## Where typst-cnd hooks into Typst

CND nodes are **not** built from the syntax AST (`typst_syntax::SyntaxNode`).
They come from the **typed content tree after evaluation and realization** —
the same layer `typst-html` uses.

Reference implementations to study:

```plain text
crates/typst-html/src/document.rs   → html_document(), realize()
crates/typst-html/src/convert.rs    → convert_to_nodes() — walk realized elements
crates/typst-pdf/src/lib.rs         → export after PagedDocument layout
crates/typst-cli/src/compile.rs     → World + typst::compile::<PagedDocument>
```

### Pipeline inside typst-cnd

```plain text
1. eval(main)                → Content (HeadingElem, ParElem, TableElem, …)
2. realize(Document)         → structured element pairs (see typst-html)
3. walk / convert            → CndNode tree (heading children, tables, …)
4. compile::<PagedDocument>  → layout + stable Introspector
5. join locations            → NodeLocation (starting page) per CND node
6. resolve refs              → refs_to / refs_from as NodeRef { id, label? }
7. serialize                 → CND JSON
```

| CND field | Typst source |
|---|---|
| `heading`, `paragraph`, `table` | `HeadingElem`, `ParElem`, `TableElem` after `realize` |
| `label` | Element labels (`<label>`) on the node itself |
| `refs_to` / `refs_from` | `NodeRef { id, label? }` — `@label` resolved via Introspector; label kept on the edge |
| `state_metadata` | Typst `State` / CND authoring flags; serialized in a stable key order |
| `location` | `Introspector` + `PagedDocument` (starting page) |
| `doc` | `DocumentInfo` (title, authors, lang, …) |
| `doc_hash` | SHA-256 of the source |
| `heading_path` | Precomputed while walking the heading tree |

The field-level contract is the CND specification in `cnd-sdk`; this crate
must not redefine it. Cross-reference edges are `{ "id": "<uuid>", "label":
"<typst-label>" }`: consumers use `id`, display and debugging use `label`.

## Commands

```bash
# Compile a document to CND
cargo run -p typst-cli -- compile path/to/doc.typ out.cnd

# Tests
cargo test -p typst-cnd
cargo test -p typst-cli        # includes tests/cnd.rs (format, parity, options)
cargo test --workspace         # what CI runs

# Lint
cargo clippy -p typst-cli -p typst-cnd --all-targets -- -D warnings
cargo fmt --all -- --check
```

## For agents

1. Read this file first.
2. Study `typst-html` before inventing a new walk — reuse `realize` + convert
   patterns.
3. CND nodes = **realized library elements**, not syntax tree nodes.
4. Keep CND changes in `crates/typst-cnd/` (and the small `typst-cli` wiring);
   avoid drive-by edits to upstream crates so upstream merges stay clean.
5. Output must stay deterministic: two compilations of the same input differ
   only in `built_at` and node UUIDs (the parity test in
   `crates/typst-cli/tests/cnd.rs` enforces it).
6. Validate emitted CND with `cnd-sdk`.
7. Use `cargo` for Rust; do not add Python dependencies to this repository.
