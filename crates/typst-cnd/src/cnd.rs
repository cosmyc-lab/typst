//! CND Typst module for metadata flags.

use typst_library::Category;
use typst_library::foundations::{Dict, Module, Scope, Value};
use typst_library::introspection::State;

/// Creates the module with CND-specific Typst definitions.
pub fn module() -> Module {
    let mut scope = Scope::deduplicating();
    scope.start_category(Category::Introspection);
    scope.define("metadata", metadata_state());
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
