//! Page assignment for CND nodes.
//!
//! `NodeLocation` carries layout facts only (cnd-sdk ADR 0012): the page a
//! node begins on, straight from the paged introspector. All position
//! bookkeeping (document/reading-order index, per-page index, within-parent
//! index) is derived by consumers from the tree's normative reading order —
//! which `emit::reading_order` guarantees at emission time — and is never
//! serialized.

use std::sync::Arc;

use typst_layout::PagedIntrospector;
use typst_library::introspection::Introspector;
use uuid::Uuid;

use crate::emit::convert::NodeRecord;
use crate::model::{CndNode, Footnote, NodeLocation};

/// Placeholder location filled in later by [`LocationAssigner`].
pub fn placeholder_location() -> NodeLocation {
    NodeLocation { page: 0 }
}

/// Assigns the starting page to emitted nodes using the paged introspector.
pub struct LocationAssigner {
    introspector: Arc<PagedIntrospector>,
    records: rustc_hash::FxHashMap<Uuid, NodeRecord>,
}

impl LocationAssigner {
    pub fn new(
        introspector: Arc<PagedIntrospector>,
        records: rustc_hash::FxHashMap<Uuid, NodeRecord>,
    ) -> Self {
        Self { introspector, records }
    }

    /// Give every node of `nodes` and of the footnote pool its page.
    pub fn assign_all(&mut self, nodes: &mut [CndNode], footnotes: &mut [Footnote]) {
        let locations: rustc_hash::FxHashMap<Uuid, NodeLocation> = self
            .records
            .iter()
            .filter_map(|(id, record)| {
                let loc = record.location?;
                let page =
                    self.introspector.page(loc).map(|p| p.get() as i32).unwrap_or(1);
                Some((*id, NodeLocation { page }))
            })
            .collect();
        assign_locations(nodes, &locations);
        for footnote in footnotes {
            assign_locations(&mut footnote.children, &locations);
        }
    }
}

fn assign_locations(
    nodes: &mut [CndNode],
    locations: &rustc_hash::FxHashMap<Uuid, NodeLocation>,
) {
    crate::model::walk_mut(nodes, &mut |node| {
        if let Some(base) = locations.get(&node.id()) {
            *node.location_mut() = base.clone();
        }
    });
}
