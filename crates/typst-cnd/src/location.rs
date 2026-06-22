//! Page and span assignment for CND nodes.

use std::sync::Arc;

use ecow::eco_vec;
use typst_layout::PagedIntrospector;
use typst_library::foundations::{NativeElement, Selector};
use typst_library::introspection::{Introspector, Location};
use typst_library::math::EquationElem;
use typst_library::model::{
    EnumElem, FigureElem, HeadingElem, ListElem, ParElem, QuoteElem, TableElem,
};
use typst_library::text::RawElem;
use uuid::Uuid;

use crate::emit::convert::NodeRecord;
use crate::emit::reading_order;
use crate::manifest::{CndNode, NodeLocation};

/// Placeholder location filled in later by [`LocationAssigner`].
pub fn placeholder_location() -> NodeLocation {
    NodeLocation {
        page: 0,
        span: 0,
        page_span: 0,
        parent_span: 0,
        span_count: 1,
    }
}

fn doc_selector() -> Selector {
    Selector::Or(eco_vec![
        HeadingElem::ELEM.select(),
        ParElem::ELEM.select(),
        TableElem::ELEM.select(),
        FigureElem::ELEM.select(),
        QuoteElem::ELEM.select(),
        RawElem::ELEM.select(),
        EquationElem::ELEM.select(),
        ListElem::ELEM.select(),
        EnumElem::ELEM.select(),
    ])
}

/// Assigns page/span coordinates to emitted nodes using the paged introspector.
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

    pub fn assign_all(&mut self, nodes: &mut [CndNode]) {
        let selector = doc_selector();
        let mut ordered: Vec<(Uuid, Location)> = self
            .records
            .iter()
            .filter_map(|(id, record)| record.location.map(|loc| (*id, loc)))
            .collect();
        ordered.sort_by(|(_, loc_a), (_, loc_b)| {
            reading_order::compare_locations(
                self.introspector.as_ref(),
                &selector,
                *loc_a,
                *loc_b,
            )
        });

        let mut locations = rustc_hash::FxHashMap::<Uuid, NodeLocation>::default();
        let mut page_counts = rustc_hash::FxHashMap::<i32, i32>::default();

        for (span, (id, loc)) in ordered.iter().enumerate() {
            let page = self
                .introspector
                .page(*loc)
                .map(|p| p.get() as i32)
                .unwrap_or(1);
            let page_span = {
                let count = page_counts.entry(page).or_insert(0);
                let current = *count;
                *count += 1;
                current
            };
            locations.insert(
                *id,
                NodeLocation {
                    page,
                    span: span as i32,
                    page_span,
                    parent_span: 0,
                    span_count: 1,
                },
            );
        }

        assign_locations(nodes, &locations, None);
    }
}

fn assign_locations(
    nodes: &mut [CndNode],
    locations: &rustc_hash::FxHashMap<Uuid, NodeLocation>,
    parent_span: Option<i32>,
) {
    for node in nodes {
        if let Some(base) = locations.get(&node.id()) {
            let mut loc = base.clone();
            loc.parent_span = parent_span.unwrap_or(0);
            *node.location_mut() = loc;
        }

        match node {
            CndNode::Heading(n) => {
                let heading_span = locations.get(&n.base.id).map(|l| l.span);
                assign_locations(&mut n.children, locations, heading_span);
            }
            CndNode::Paragraph(_)
            | CndNode::Table(_)
            | CndNode::Quote(_)
            | CndNode::Code(_)
            | CndNode::Math(_)
            | CndNode::Figure(_)
            | CndNode::List(_) => {}
        }
    }
}
