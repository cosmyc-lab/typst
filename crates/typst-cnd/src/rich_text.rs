//! Rich text: prose with standoff marks and its own link families
//! (cnd-sdk `cnd/core/text.py`).
//!
//! Every `[start, end)` span, a mark or an edge's `text_span`, indexes the
//! `value` of the `RichText` that holds it, in Unicode code points
//! (ADR 0013).

use serde::{Deserialize, Serialize};

/// The producer's verbatim source for a node, with its language. A bare
/// string dropped the one thing a consumer needs in order to decide whether
/// to attempt a parse: which language the content is in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct RawSource {
    pub format: String,
    pub value: String,
}

impl RawSource {
    /// Every `raw` this producer emits is Typst source, by construction.
    pub fn typst(value: impl Into<String>) -> Self {
        Self { format: "typst".into(), value: value.into() }
    }
}

/// Forward cross-reference edge, keyed by the target's label (ADR 0017,
/// superseding the `{id, label}` form of ADR 0002). `label` resolves in the
/// `nodes` tree.
///
/// An edge keys on the label rather than the id because an id is not
/// durable across builds (ADR 0015) — this producer mints a fresh `uuid4`
/// on every run, so an edge carrying one would expire immediately. The
/// label lives in the Typst source and survives.
///
/// `text_span` is an optional `[start, end)` codepoint offset marking where
/// the reference marker sits in the containing node's rendered text; named
/// `text_span` to pin the coordinate space (ADR 0013).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct NodeRef {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_span: Option<Vec<i64>>,
}

impl NodeRef {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), text_span: None }
    }
}

/// Forward citation edge; `label` resolves in the `bibliography` pool
/// (proposal 0004). `form` mirrors Typst's citation form; `text_span` as on
/// `NodeRef` (ADR 0013).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CiteRef {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_span: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplement: Option<String>,
}

/// Forward footnote edge; `label` resolves in the `footnotes` pool.
/// `text_span` as on `NodeRef` (ADR 0013).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct FootnoteRef {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_span: Option<Vec<i64>>,
}

/// Forward hyperlink edge (ADR 0024, cnd-sdk): `href` is the URL string the
/// marker points at. Unlike `NodeRef`/`CiteRef`/`FootnoteRef`, `links` has
/// no resolution domain — an `href` is never invalid for naming something
/// the CND does not carry. `text_span` as on `NodeRef` (ADR 0013).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct LinkRef {
    pub href: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_span: Option<Vec<i64>>,
}

/// Mark kinds the format defines. `Mark.kind` is an open vocabulary.
pub const STRONG: &str = "strong";
pub const EMPH: &str = "emph";
pub const CODE: &str = "code";
pub const STRIKE: &str = "strike";
pub const SUP: &str = "sup";
pub const SUB: &str = "sub";
pub const UNDERLINE: &str = "underline";
pub const HIGHLIGHT: &str = "highlight";
pub const MATH: &str = "math";

/// A standoff span of inline formatting inside a `RichText.value`, in
/// Unicode code points. `raw` is the source of the covered text (`math`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Mark {
    pub start: i64,
    pub end: i64,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawSource>,
}

/// Prose: a text, its language, its marks and its forward edges. Every
/// list is omitted when empty, so `{"value": "..."}` is a complete text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct RichText {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<NodeRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<CiteRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footnotes: Vec<FootnoteRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<LinkRef>,
}

impl RichText {
    /// A text with no language, marks or edges.
    pub fn plain(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            lang: None,
            marks: Vec::new(),
            refs: Vec::new(),
            cites: Vec::new(),
            footnotes: Vec::new(),
            links: Vec::new(),
        }
    }
}

/// A resolved counter as displayed: `value` ("3", "2.1.1", "(1)") and the
/// counter word in the document's language (`name`, "Figure").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct Numbering {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn is_atomic(kind: &str) -> bool {
    kind == CODE || kind == MATH
}

/// Whether `[start, end)` overlaps `atom` without covering it, that is,
/// reaches partway into it.
fn reaches_into(start: i64, end: i64, atom: &Mark) -> bool {
    start < atom.end && end > atom.start && !(start <= atom.start && end >= atom.end)
}

/// Put `marks` in canonical form (spec 4.1) for a text of `len` code points:
/// drop empty and out-of-bounds marks, merge same-kind marks that overlap or
/// touch (two `math` marks join their `raw` with a space), enforce the atomic rule
/// for `code` and `math` (a non-atomic mark reaching partway into one is
/// dropped, a span doing so becomes `None`), then sort by `(start, end, kind)`.
pub fn canonical_marks(
    len: i64,
    marks: Vec<Mark>,
    spans: &mut [&mut Option<(i64, i64)>],
) -> Vec<Mark> {
    let mut marks: Vec<Mark> = marks
        .into_iter()
        .filter(|m| m.start >= 0 && m.start < m.end && m.end <= len)
        .collect();

    // Merge per kind, in source order within the kind.
    marks.sort_by(|a, b| (&a.kind, a.start, a.end).cmp(&(&b.kind, b.start, b.end)));
    let mut merged: Vec<Mark> = Vec::with_capacity(marks.len());
    for m in marks {
        match merged.last_mut() {
            Some(last) if last.kind == m.kind && m.start <= last.end => {
                last.end = last.end.max(m.end);
                if let (Some(a), Some(b)) = (last.raw.as_mut(), m.raw) {
                    // `$a$$b$` is two identifiers, not the one `ab` names.
                    let apart = a.value.ends_with(char::is_whitespace)
                        || b.value.starts_with(char::is_whitespace);
                    if !apart && !a.value.is_empty() && !b.value.is_empty() {
                        a.value.push(' ');
                    }
                    a.value.push_str(&b.value);
                }
            }
            _ => merged.push(m),
        }
    }

    let atoms: Vec<Mark> =
        merged.iter().filter(|m| is_atomic(&m.kind)).cloned().collect();
    merged.retain(|m| {
        is_atomic(&m.kind) || !atoms.iter().any(|a| reaches_into(m.start, m.end, a))
    });
    for span in spans.iter_mut() {
        if let Some((s, e)) = **span
            && atoms.iter().any(|a| reaches_into(s, e, a))
        {
            **span = None;
        }
    }

    merged.sort_by(|a, b| (a.start, a.end, &a.kind).cmp(&(b.start, b.end, &b.kind)));
    merged
}

/// Trim Unicode whitespace at both ends of `value` and re-base `marks` and
/// `spans` onto the result: offsets shift by the code points removed in
/// front and clip to the new length. A mark left empty is dropped; a span
/// left empty becomes a collapsed `Some((n, n))` at its clipped position,
/// for the caller to null under its own rule.
pub fn trim_rebase(
    value: &str,
    marks: Vec<Mark>,
    spans: &mut [&mut Option<(i64, i64)>],
) -> (String, Vec<Mark>) {
    let trimmed = value.trim();
    let lead = value[..value.len() - value.trim_start().len()].chars().count() as i64;
    let len = trimmed.chars().count() as i64;
    let rebase = |s: i64, e: i64| ((s - lead).clamp(0, len), (e - lead).clamp(0, len));

    let marks = marks
        .into_iter()
        .filter_map(|mut m| {
            let (s, e) = rebase(m.start, m.end);
            (s < e).then(|| {
                m.start = s;
                m.end = e;
                m
            })
        })
        .collect();
    for span in spans.iter_mut() {
        if let Some((s, e)) = **span {
            **span = Some(rebase(s, e));
        }
    }
    (trimmed.to_owned(), marks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(kind: &str, start: i64, end: i64) -> Mark {
        Mark { start, end, kind: kind.into(), raw: None }
    }

    fn math(start: i64, end: i64, src: &str) -> Mark {
        Mark {
            start,
            end,
            kind: MATH.into(),
            raw: Some(RawSource::typst(src)),
        }
    }

    fn span(start: i64, end: i64) -> Option<(i64, i64)> {
        Some((start, end))
    }

    #[test]
    fn plain_text_serializes_as_value_only() {
        let json = serde_json::to_value(RichText::plain("a")).unwrap();
        assert_eq!(json, serde_json::json!({"value": "a"}));
    }

    #[test]
    fn same_kind_touching_marks_merge() {
        let out =
            canonical_marks(5, vec![mark(STRONG, 0, 3), mark(STRONG, 3, 5)], &mut []);
        assert_eq!(out, vec![mark(STRONG, 0, 5)]);
    }

    #[test]
    fn different_kinds_may_overlap() {
        let out = canonical_marks(5, vec![mark(EMPH, 2, 4), mark(STRONG, 0, 3)], &mut []);
        assert_eq!(out, vec![mark(STRONG, 0, 3), mark(EMPH, 2, 4)]);
    }

    #[test]
    fn marks_sort_by_start_end_kind() {
        // The brief's `strong 0..1` sits inside `code 0..2`, which the atomic
        // rule drops (spec 4.1 rule 4), so the strong spans the whole text.
        let out = canonical_marks(
            4,
            vec![
                mark(STRONG, 0, 4),
                mark(EMPH, 0, 2),
                mark(CODE, 0, 2),
                mark(STRIKE, 1, 3),
            ],
            &mut [],
        );
        assert_eq!(out, vec![mark(CODE, 0, 2), mark(EMPH, 0, 2), mark(STRONG, 0, 4)]);
    }

    #[test]
    fn empty_and_out_of_bounds_marks_drop() {
        let out = canonical_marks(4, vec![mark(STRONG, 2, 2), mark(EMPH, 3, 9)], &mut []);
        assert!(out.is_empty());
    }

    #[test]
    fn mark_reaching_into_code_drops() {
        let out = canonical_marks(8, vec![mark(CODE, 2, 6), mark(STRONG, 0, 3)], &mut []);
        assert_eq!(out, vec![mark(CODE, 2, 6)]);
        let out = canonical_marks(8, vec![mark(CODE, 2, 6), mark(STRONG, 0, 8)], &mut []);
        assert_eq!(out, vec![mark(STRONG, 0, 8), mark(CODE, 2, 6)]);
    }

    #[test]
    fn span_reaching_into_math_is_nulled() {
        let mut partial = span(3, 5);
        let mut covering = span(2, 6);
        canonical_marks(8, vec![math(2, 6, "x")], &mut [&mut partial, &mut covering]);
        assert_eq!(partial, None);
        assert_eq!(covering, span(2, 6));
    }

    #[test]
    fn touching_math_marks_join_raw_with_a_space() {
        let out = canonical_marks(6, vec![math(0, 3, "a"), math(3, 6, "b")], &mut []);
        assert_eq!(out, vec![math(0, 6, "a b")]);
        // No second space where one already separates them.
        let out = canonical_marks(6, vec![math(0, 3, "a "), math(3, 6, "b")], &mut []);
        assert_eq!(out, vec![math(0, 6, "a b")]);
    }

    #[test]
    fn trim_rebases_marks_and_spans() {
        let mut s = span(3, 6);
        let (value, marks) =
            trim_rebase("  é*x* ", vec![mark(STRONG, 3, 6)], &mut [&mut s]);
        assert_eq!(value, "é*x*");
        assert_eq!(marks, vec![mark(STRONG, 1, 4)]);
        assert_eq!(s, span(1, 4));
    }

    #[test]
    fn trim_drops_emptied_marks_and_empties_spans() {
        let mut s = span(0, 2);
        let (value, marks) = trim_rebase("  ab", vec![mark(STRONG, 0, 2)], &mut [&mut s]);
        assert_eq!(value, "ab");
        assert!(marks.is_empty());
        assert_eq!(s, span(0, 0));
    }
}
