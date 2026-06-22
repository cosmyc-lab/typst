use ecow::eco_vec;
use std::ops::ControlFlow;
use typst_library::foundations::{Content, Label, NativeElement, Selector};
use typst_library::introspection::Introspector;
use typst_library::math::EquationElem;
use typst_library::model::{EnumElem, FigureElem, HeadingElem, ListElem, ParElem, QuoteElem, RefElem};
use typst_library::text::RawElem;
use uuid::Uuid;

use crate::emit::convert::ConvertContext;
use crate::manifest::{CndNode, NodeRef};

fn doc_selector() -> Selector {
    Selector::Or(eco_vec![
        HeadingElem::ELEM.select(),
        ParElem::ELEM.select(),
        typst_library::model::TableElem::ELEM.select(),
        FigureElem::ELEM.select(),
        QuoteElem::ELEM.select(),
        RawElem::ELEM.select(),
        EquationElem::ELEM.select(),
        ListElem::ELEM.select(),
        EnumElem::ELEM.select(),
    ])
}

/// Resolve cross-references and populate `refs_to` / `refs_from` bidirectionally.
pub fn resolve_refs(
    ctx: &mut ConvertContext,
    introspector: &dyn Introspector,
    content: &Content,
) {
    let mut edges: Vec<(Uuid, Uuid, Option<String>)> = Vec::new();
    let _selector = doc_selector();

    for (source_id, record) in &ctx.records {
        for label in &record.ref_targets {
            if let Some(target_id) = resolve_label(label, ctx, introspector) {
                edges.push((
                    *source_id,
                    target_id,
                    Some(label.resolve().as_str().into()),
                ));
            }
        }
    }

    let ref_edges = ref_edges_from_content(content);
    let paragraph_ids = paragraph_ids_from_nodes(&ctx.roots);
    let heading_ids = heading_ids_from_nodes(&ctx.roots);
    for (source_kind, index, label) in ref_edges {
        let source_id = match source_kind {
            RefSourceKind::Paragraph => paragraph_ids.get(index).copied(),
            RefSourceKind::Heading => heading_ids.get(index).copied(),
        };
        let Some(source_id) = source_id else { continue };
        if let Some(target_id) = resolve_label(&label, ctx, introspector) {
            edges.push((
                source_id,
                target_id,
                Some(label.resolve().as_str().into()),
            ));
            if matches!(source_kind, RefSourceKind::Heading) {
                if let Some(paragraph_id) = last_paragraph_under_heading(&ctx.roots, source_id) {
                    edges.push((
                        paragraph_id,
                        target_id,
                        Some(label.resolve().as_str().into()),
                    ));
                }
            }
        }
    }

    for (source, target, target_label) in edges {
        let source_label = node_label(ctx, source);
        let resolved_target_label = target_label.or_else(|| node_label(ctx, target));
        set_ref_to(ctx, source, target, resolved_target_label);
        set_ref_from(ctx, target, source, source_label);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefSourceKind {
    Heading,
    Paragraph,
}

fn ref_edges_from_content(content: &Content) -> Vec<(RefSourceKind, usize, Label)> {
    let mut out = Vec::new();
    let mut current: Option<(RefSourceKind, usize)> = None;
    let mut par_index = 0usize;
    let mut heading_index = 0usize;
    let _ = content.traverse(&mut |element| {
        if element.to_packed::<ParElem>().is_some() {
            current = Some((RefSourceKind::Paragraph, par_index));
            par_index += 1;
        } else if element.to_packed::<HeadingElem>().is_some() {
            current = Some((RefSourceKind::Heading, heading_index));
            heading_index += 1;
        } else if let Some(reference) = element.to_packed::<RefElem>() {
            if let Some((kind, index)) = current {
                out.push((kind, index, reference.target));
            }
        }
        ControlFlow::<()>::Continue(())
    });
    out
}

fn paragraph_ids_from_nodes(nodes: &[CndNode]) -> Vec<Uuid> {
    let mut out = Vec::new();
    walk_paragraph_ids(nodes, &mut out);
    out
}

fn heading_ids_from_nodes(nodes: &[CndNode]) -> Vec<Uuid> {
    let mut out = Vec::new();
    walk_heading_ids(nodes, &mut out);
    out
}

fn walk_paragraph_ids(nodes: &[CndNode], out: &mut Vec<Uuid>) {
    for node in nodes {
        match node {
            CndNode::Paragraph(n) => out.push(n.base.id),
            CndNode::Heading(n) => walk_paragraph_ids(&n.children, out),
            CndNode::Table(_)
            | CndNode::Quote(_)
            | CndNode::Code(_)
            | CndNode::Math(_)
            | CndNode::Figure(_)
            | CndNode::List(_) => {}
        }
    }
}

fn walk_heading_ids(nodes: &[CndNode], out: &mut Vec<Uuid>) {
    for node in nodes {
        if let CndNode::Heading(n) = node {
            out.push(n.base.id);
            walk_heading_ids(&n.children, out);
        }
    }
}

fn resolve_label(
    label: &Label,
    ctx: &ConvertContext,
    introspector: &dyn Introspector,
) -> Option<Uuid> {
    if let Some(id) = ctx.label_to_id.get(label).copied() {
        return Some(id);
    }

    let content = introspector.query_label(*label).ok()?;
    if let Some(loc) = content.location() {
        if let Some(id) = ctx.location_to_id.get(&loc).copied() {
            return Some(id);
        }
    }

    for elem in introspector.query(&FigureElem::ELEM.select()) {
        if elem.label() != Some(*label) {
            continue;
        }
        if let Some(loc) = elem.location() {
            if let Some(id) = ctx.location_to_id.get(&loc).copied() {
                return Some(id);
            }
        }
    }

    if content.to_packed::<FigureElem>().is_some() {
        for (id, record) in &ctx.records {
            if record.label.as_ref() == Some(label) {
                return Some(*id);
            }
        }
        return find_labeled_table(ctx);
    }

    for (id, record) in &ctx.records {
        if record.label.as_ref() == Some(label) {
            return Some(*id);
        }
    }

    None
}

fn find_labeled_table(ctx: &ConvertContext) -> Option<Uuid> {
    fn walk(nodes: &[CndNode]) -> Option<Uuid> {
        for node in nodes {
            match node {
                CndNode::Table(n) => return Some(n.base.id),
                CndNode::Figure(n) => return Some(n.base.id),
                CndNode::Heading(n) => {
                    if let Some(id) = walk(&n.children) {
                        return Some(id);
                    }
                }
                CndNode::Paragraph(_)
                | CndNode::Quote(_)
                | CndNode::Code(_)
                | CndNode::Math(_)
                | CndNode::List(_) => {}
            }
        }
        None
    }
    walk(&ctx.roots)
}

fn last_paragraph_under_heading(nodes: &[CndNode], heading_id: Uuid) -> Option<Uuid> {
    fn walk(nodes: &[CndNode], target: Uuid, last: &mut Option<Uuid>) -> bool {
        for node in nodes {
            match node {
                CndNode::Heading(n) if n.base.id == target => {
                    find_last_paragraph(&n.children, last);
                    return true;
                }
                CndNode::Heading(n) => {
                    if walk(&n.children, target, last) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn find_last_paragraph(nodes: &[CndNode], last: &mut Option<Uuid>) {
        for node in nodes {
            match node {
                CndNode::Paragraph(n) => *last = Some(n.base.id),
                CndNode::Heading(n) => find_last_paragraph(&n.children, last),
                CndNode::Table(_)
                | CndNode::Quote(_)
                | CndNode::Code(_)
                | CndNode::Math(_)
                | CndNode::Figure(_)
                | CndNode::List(_) => {}
            }
        }
    }

    let mut last = None;
    walk(nodes, heading_id, &mut last);
    last
}

fn node_label(ctx: &ConvertContext, id: Uuid) -> Option<String> {
    fn walk(nodes: &[CndNode], id: Uuid) -> Option<String> {
        for node in nodes {
            if node.id() == id {
                return node.base().label.clone();
            }
            if let CndNode::Heading(h) = node {
                if let Some(label) = walk(&h.children, id) {
                    return Some(label);
                }
            }
        }
        None
    }
    walk(&ctx.roots, id)
}

fn set_ref_to(ctx: &mut ConvertContext, source: Uuid, target: Uuid, label: Option<String>) {
    if let Some(node) = find_node_mut(&mut ctx.roots, source) {
        let refs = node.refs_to_mut();
        if !refs.iter().any(|reference| reference.id == target) {
            refs.push(NodeRef::new(target, label));
        }
    }
}

fn set_ref_from(ctx: &mut ConvertContext, target: Uuid, source: Uuid, label: Option<String>) {
    if let Some(node) = find_node_mut(&mut ctx.roots, target) {
        let refs = node.refs_from_mut();
        if !refs.iter().any(|reference| reference.id == source) {
            refs.push(NodeRef::new(source, label));
        }
    }
}

fn find_node_mut(nodes: &mut [CndNode], id: Uuid) -> Option<&mut CndNode> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.children_mut() {
            if let Some(found) = find_node_mut(children, id) {
                return Some(found);
            }
        }
    }
    None
}

/// Build label lookup from records and labelled introspector elements.
pub fn rebuild_label_index(ctx: &mut ConvertContext, introspector: &dyn Introspector) {
    ctx.label_to_id.clear();
    for (id, record) in &ctx.records {
        if let Some(label) = record.label {
            ctx.label_to_id.insert(label, *id);
        }
    }

    for elem in introspector.query_labelled() {
        let Some(label) = elem.label() else { continue };
        let Some(id) = resolve_label(&label, ctx, introspector) else { continue };
        ctx.label_to_id.insert(label, id);
        if let Some(record) = ctx.records.get_mut(&id) {
            record.label = Some(label);
        }
    }
}
