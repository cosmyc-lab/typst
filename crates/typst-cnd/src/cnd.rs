//! CND Typst module for metadata flags and authoring helpers.
//!
//! This module is the "CND authoring SDK" surface: native functions
//! injected into every document's global scope (see `world.rs`'s
//! `library.global.scope_mut().define("cnd", crate::cnd::module())`), for
//! Typst source that wants to produce a CND manifest with more than the
//! default node shape — without the document author hand-writing fragile
//! state-bracketing Typst code themselves. Grows by proven need, one
//! primitive at a time (today: `cnd.table`'s `content_kind`), not as a
//! speculative framework.

use typst_library::Category;
use typst_library::diag::SourceResult;
use typst_library::engine::Engine;
use typst_library::foundations::{Args, Construct, Content, Dict, Module, NativeFunc, Scope, Str, Value};
use typst_library::introspection::{State, StateUpdate};
use typst_library::model::TableElem;

/// Creates the module with CND-specific Typst definitions.
pub fn module() -> Module {
    let mut scope = Scope::deduplicating();
    scope.start_category(Category::Introspection);
    scope.define("metadata", metadata_state());
    scope.define("table", cnd_table::func());
    Module::new("cnd", scope)
}

/// Shared state key for per-node metadata accumulation.
///
/// In Typst sources, use:
/// ```typ
/// #cnd.metadata.update(it => it + (revision: "4.2"))
/// ```
pub fn metadata_state() -> State {
    State::new("cnd.metadata".into(), Value::Dict(Dict::new()))
}

/// Sets `content_kind` in a `cnd.metadata` dict snapshot — the "before"
/// half of `cnd.table`'s state bracketing (see below).
#[typst_macros::func]
fn cnd_set_content_kind(kind: Str, prev: Dict) -> Dict {
    let mut prev = prev;
    prev.insert(Str::from("content_kind"), Value::Str(kind));
    prev
}

/// Removes `content_kind` from a `cnd.metadata` dict snapshot — the
/// "after"/reset half of `cnd.table`'s state bracketing.
#[typst_macros::func]
fn cnd_unset_content_kind(prev: Dict) -> Dict {
    let mut prev = prev;
    let _ = prev.remove(Str::from("content_kind"), None);
    prev
}

/// `cnd.table`: Typst's built-in `table`, plus an optional `content_kind`
/// hint ("data" | "content" — see cnd-sdk's `TableNode.content_kind` /
/// proposal 0001) on the CND node this produces. Equivalent to hand-writing
/// (and exactly what a document previously had to hand-write, per-call,
/// error-prone — see `cnd.metadata`'s own doc comment):
///
/// ```typ
/// #cnd.metadata.update(it => (..it, content_kind: "content"))
/// #table(..)
/// #cnd.metadata.update(it => { let d = it; let _ = d.remove("content_kind"); d })
/// ```
///
/// `content_kind` defaults to `none` (unset) — matching the CND format's
/// own default (unset resolves to `"data"`, never guessed): this shared
/// primitive stays neutral, a document's own wrapper is the right place to
/// pick a different default for its own corpus (see `cosmyc_*.typ`'s local
/// `ctable`, which currently forwards straight through to `cnd.table`).
#[typst_macros::func(name = "table")]
fn cnd_table(engine: &mut Engine, args: &mut Args) -> SourceResult<Content> {
    let span = args.span;
    // Consumed here (named, not positional) so the rest of `args` — whatever
    // it is — forwards to `TableElem::construct` completely unchanged, same
    // as a plain `table(..)` call would receive.
    let content_kind: Option<Str> = args.named("content_kind")?;
    let table_content = TableElem::construct(engine, args)?;
    let Some(kind) = content_kind else {
        return Ok(table_content);
    };

    let state = metadata_state();
    let set_fn = cnd_set_content_kind::func().with(&mut Args::new(span, [Value::Str(kind)]));
    let unset_fn = cnd_unset_content_kind::func();

    let before = state.clone().update(span, StateUpdate::Func(set_fn));
    let after = state.update(span, StateUpdate::Func(unset_fn));
    Ok(before + table_content + after)
}
