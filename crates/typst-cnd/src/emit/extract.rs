use ecow::EcoString;
use typst_library::foundations::{Content, PlainText, Value};

/// Extract plain text from content without duplicating inline code.
///
/// Unlike `Content::plain_text()`, this stops at elements that implement
/// `PlainText` and does not recurse into their synthesized children. This
/// prevents `RawElem` text from appearing three times (once from `RawElem`,
/// once from the synthesized `RawLine`, once from the `TextElem` inside
/// `RawLine.body`).
pub fn extract_text(content: &Content) -> EcoString {
    let mut out = EcoString::new();
    walk(content, &mut out);
    out
}

fn walk(content: &Content, out: &mut EcoString) {
    if let Some(textable) = content.with::<dyn PlainText>() {
        textable.plain_text(out);
        return;
    }
    for (_, value) in content.fields() {
        walk_value(value, out);
    }
}

fn walk_value(value: Value, out: &mut EcoString) {
    match value {
        Value::Content(c) => walk(&c, out),
        Value::Array(arr) => {
            for v in arr {
                walk_value(v, out);
            }
        }
        _ => {}
    }
}
