//! The readable linear form of a formula: the `text` of a `math` node and
//! the `value` a `math` mark covers (spec §4.1, §6.6). The formula's Typst
//! source is the node's or the mark's `raw`; this form is what a reader or
//! a search index sees instead.
//!
//! It is read from Typst's math elements, not from the source text, so a
//! symbol is the Unicode character Typst renders (`alpha` is `α`, `<=` is
//! `≤`, `-` is `−`) and an argument a function filled in is the text it was
//! filled with. Letters keep their plain form: no italic, bold or
//! double-struck code point (`bb(R)` is `R`). The same body always gives the
//! same text.
//!
//! | Construct | Typst | Linear form |
//! |---|---|---|
//! | symbol, letter, number, text | `alpha <= x "if" 3.14` | `α ≤ x if 3.14` |
//! | spacing (source space, `quad`, `thin`…) | `a   b`, `x quad y` | `a b`, `x y` |
//! | attachment | `x^2`, `x_1^2`, `sum_(i=1)^n` | `x^2`, `x_1^2`, `∑_(i=1)^n` |
//! | primes | `f''(x)`, `f'^2` | `f″(x)`, `f′^2` |
//! | fraction | `1/3`, `(a+b)/(c+d)`, `x^2/2` | `1/3`, `(a+b)/(c+d)`, `(x^2)/2` |
//! | root | `sqrt(x)`, `root(3, x)` | `sqrt(x)`, `root(3, x)` |
//! | binomial | `binom(n, k)` | `binom(n, k)` |
//! | operator | `sin x`, `lim_(x -> 0)` | `sin x`, `lim_(x → 0)` |
//! | differential | `f(x) dif x` | `f(x) dx` |
//! | delimiters | `abs(x)`, `( a + b )` | `\|x\|`, `(a + b)` |
//! | matrix | `mat(1, 2; 3, 4)` | `[[1, 2], [3, 4]]` |
//! | vector | `vec(1, 2, 3)` | `(1, 2, 3)` |
//! | cases | `cases(x &"if" x > 0, 0 &"else")` | `{x if x > 0; 0 else}` |
//! | accent, over/underline | `hat(x)`, `overline(z)` | `x̂`, `z̅` (base, then the combining mark) |
//! | brace, bracket… with annotation | `underbrace(a + b, n)` | `(a + b)_n` (over: `^n`) |
//! | line break | `a &= b \ &= c` | `a = b; = c` |
//! | reference, citation | `#ref(<eq>)`, `#cite(<key>)` | `@eq`, `@key` |
//!
//! Rules:
//!
//! - **Parentheses.** A base, script, numerator, denominator or annotation
//!   is wrapped in parentheses unless it is atomic: one symbol, a number, a
//!   word, a delimited group, a matrix, vector, cases, binomial or root, an
//!   operator, or an accent over an atomic base. So `x^(2n)`, `e^(x^2)` and
//!   `(a+b)/(c+d)`, but `x^10`, `(a+b)^2` and `sqrt(x)/2`.
//! - **Spaces.** A space in the source and explicit spacing become one
//!   space; runs collapse, and a group never starts or ends with one. A weak
//!   space (the one before `dif`) and an alignment point (`&`) add none.
//!   After a unary operator there is none, so `dif x` is `dx`. Inside a
//!   delimited group, none follows the opening delimiter or precedes the
//!   closing one.
//! - **Calls** separate their arguments with `, `, cases and line breaks
//!   separate rows with `; `.

use typst_library::foundations::{
    Content, PlainText, SequenceElem, StyleChain, StyledElem, SymbolElem, Value,
};
use typst_library::layout::HElem;
use typst_library::math::{
    AccentElem, AlignPointElem, AttachElem, BinomElem, CancelElem, CasesElem, ClassElem,
    EquationElem, FracElem, LimitsElem, LrElem, MatElem, MidElem, OpElem, OverbraceElem,
    OverbracketElem, OverlineElem, OverparenElem, OvershellElem, PrimesElem, RootElem,
    ScriptsElem, StretchElem, UnderbraceElem, UnderbracketElem, UnderlineElem,
    UnderparenElem, UndershellElem, VecElem,
};
use typst_library::model::{CiteElem, RefElem};
use typst_library::text::{LinebreakElem, SpaceElem, TextElem};
use unicode_math_class::MathClass;

/// The linear form of an equation body (see the module documentation).
pub fn math_linear(body: &Content) -> String {
    let mut line = Line::default();
    line.content(body);
    line.out
}

/// What separates the next token from the text before it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gap {
    /// Nothing: the tokens touch.
    #[default]
    None,
    /// One space.
    Space,
    /// A row separator, `; `.
    Row,
}

/// A linear form being written, token by token.
#[derive(Debug, Default)]
struct Line {
    out: String,
    /// The gap the next token opens with, unless it is the first.
    gap: Gap,
    /// The last token binds to the next one: no space between them (a
    /// unary operator, an opening delimiter). A row separator still goes.
    glued: bool,
}

impl Line {
    /// Append a token, after the pending gap.
    fn token(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if !self.out.is_empty() {
            match self.gap {
                Gap::Row => self.out.push_str("; "),
                Gap::Space if !self.glued => self.out.push(' '),
                _ => {}
            }
        }
        self.gap = Gap::None;
        self.glued = false;
        self.out.push_str(text);
    }

    fn gap(&mut self, gap: Gap) {
        self.gap = self.gap.max(gap);
    }

    fn content(&mut self, content: &Content) {
        let styles = StyleChain::default();
        if let Some(sequence) = content.to_packed::<SequenceElem>() {
            for child in &sequence.children {
                self.content(child);
            }
        } else if let Some(styled) = content.to_packed::<StyledElem>() {
            self.content(&styled.child);
        } else if let Some(equation) = content.to_packed::<EquationElem>() {
            self.content(&equation.body);
        } else if content.is::<SpaceElem>() {
            self.gap(Gap::Space);
        } else if let Some(h) = content.to_packed::<HElem>() {
            if !h.weak.get(styles) {
                self.gap(Gap::Space);
            }
        } else if content.is::<LinebreakElem>() {
            self.gap(Gap::Row);
        } else if content.is::<AlignPointElem>() {
            // An alignment point places the text; it adds none.
        } else if let Some(symbol) = content.to_packed::<SymbolElem>() {
            self.token(&symbol.text);
        } else if let Some(text) = content.to_packed::<TextElem>() {
            self.token(text.text.trim());
        } else if let Some(class) = content.to_packed::<ClassElem>() {
            self.content(&class.body);
            if class.class == MathClass::Unary {
                self.glued = true;
            }
        } else if let Some(lr) = content.to_packed::<LrElem>() {
            self.delimited(&lr.body);
        } else if let Some(attach) = content.to_packed::<AttachElem>() {
            self.token(&attach_text(attach, styles));
        } else if let Some(primes) = content.to_packed::<PrimesElem>() {
            self.token(&primes_text(primes.count));
        } else if let Some(frac) = content.to_packed::<FracElem>() {
            self.token(&format!("{}/{}", part(&frac.num), part(&frac.denom)));
        } else if let Some(root) = content.to_packed::<RootElem>() {
            let radicand = math_linear(&root.radicand);
            self.token(&match root.index.get_cloned(styles) {
                Some(index) => format!("root({}, {radicand})", math_linear(&index)),
                None => format!("sqrt({radicand})"),
            });
        } else if let Some(binom) = content.to_packed::<BinomElem>() {
            let args = std::iter::once(&binom.upper).chain(&binom.lower);
            self.token(&format!("binom({})", joined(args, ", ")));
        } else if let Some(mat) = content.to_packed::<MatElem>() {
            let rows: Vec<_> = mat
                .rows
                .iter()
                .map(|row| format!("[{}]", joined(row, ", ")))
                .collect();
            self.token(&format!("[{}]", rows.join(", ")));
        } else if let Some(vec) = content.to_packed::<VecElem>() {
            self.token(&format!("({})", joined(&vec.children, ", ")));
        } else if let Some(cases) = content.to_packed::<CasesElem>() {
            self.token(&format!("{{{}}}", joined(&cases.children, "; ")));
        } else if let Some(op) = content.to_packed::<OpElem>() {
            self.content(&op.text);
        } else if let Some(accent) = content.to_packed::<AccentElem>() {
            self.token(&format!("{}{}", part(&accent.base), accent.accent.0));
        } else if let Some(line) = content.to_packed::<OverlineElem>() {
            self.token(&format!("{}\u{305}", part(&line.body)));
        } else if let Some(line) = content.to_packed::<UnderlineElem>() {
            self.token(&format!("{}\u{332}", part(&line.body)));
        } else if let Some((body, annotation, mark)) = annotated(content, styles) {
            match annotation {
                Some(annotation) => {
                    self.token(&format!("{}{mark}{}", part(body), part(&annotation)));
                }
                None => self.content(body),
            }
        } else if let Some(body) = transparent_body(content) {
            self.content(body);
        } else if let Some(reference) = content.to_packed::<RefElem>() {
            self.token(&format!("@{}", reference.target.resolve().as_str()));
        } else if let Some(cite) = content.to_packed::<CiteElem>() {
            self.token(&format!("@{}", cite.key.resolve().as_str()));
        } else if let Some(textable) = content.with::<dyn PlainText>() {
            let mut text = ecow::EcoString::new();
            textable.plain_text(&mut text);
            self.token(text.trim());
        } else {
            for (_, value) in content.fields() {
                self.value(value);
            }
        }
    }

    fn value(&mut self, value: Value) {
        match value {
            Value::Content(content) => self.content(&content),
            Value::Array(array) => {
                for value in array {
                    self.value(value);
                }
            }
            _ => {}
        }
    }

    /// A delimited group: no space after its opening delimiter, none before
    /// its closing one.
    fn delimited(&mut self, body: &Content) {
        let children =
            body.to_packed::<SequenceElem>().map(|seq| seq.children.as_slice());
        if let Some([open, middle @ .., close]) = children
            && let Some(open) = open.to_packed::<SymbolElem>()
            && let Some(close) = close.to_packed::<SymbolElem>()
        {
            self.token(&open.text);
            self.glued = true;
            for child in middle {
                self.content(child);
            }
            self.gap = Gap::None;
            self.token(&close.text);
        } else {
            self.content(body);
        }
    }
}

/// An attachment: the prescripts, the base, its primes, then its scripts.
fn attach_text(attach: &AttachElem, styles: StyleChain) -> String {
    let script = |field: Option<Content>, mark: char| {
        field
            .map(|content| format!("{mark}{}", part(&content)))
            .unwrap_or_default()
    };
    let (primes, tr) = match attach.tr.get_cloned(styles) {
        Some(tr) => match tr.to_packed::<PrimesElem>() {
            Some(primes) => (primes_text(primes.count), None),
            None => (String::new(), Some(tr)),
        },
        None => (String::new(), None),
    };
    [
        script(attach.bl.get_cloned(styles), '_'),
        script(attach.tl.get_cloned(styles), '^'),
        part(&attach.base),
        primes,
        script(attach.b.get_cloned(styles), '_'),
        script(attach.t.get_cloned(styles), '^'),
        script(attach.br.get_cloned(styles), '_'),
        script(tr, '^'),
    ]
    .concat()
}

/// Primes as Typst renders them: ′ ″ ‴ ⁗, then repeated ′.
fn primes_text(count: usize) -> String {
    match count {
        1 => "′".into(),
        2 => "″".into(),
        3 => "‴".into(),
        4 => "⁗".into(),
        count => "′".repeat(count),
    }
}

/// The linear forms of `items`, joined by `separator`.
fn joined<'a>(items: impl IntoIterator<Item = &'a Content>, separator: &str) -> String {
    items.into_iter().map(math_linear).collect::<Vec<_>>().join(separator)
}

/// A brace, bracket, parenthesis or shell over or under a body: the body,
/// the annotation and the script mark it reads with.
fn annotated<'a>(
    content: &'a Content,
    styles: StyleChain,
) -> Option<(&'a Content, Option<Content>, char)> {
    macro_rules! annotated {
        ($($elem:ty => $mark:literal),* $(,)?) => {
            $(if let Some(elem) = content.to_packed::<$elem>() {
                return Some((&elem.body, elem.annotation.get_cloned(styles), $mark));
            })*
        };
    }
    annotated!(
        UnderbraceElem => '_',
        UnderbracketElem => '_',
        UnderparenElem => '_',
        UndershellElem => '_',
        OverbraceElem => '^',
        OverbracketElem => '^',
        OverparenElem => '^',
        OvershellElem => '^',
    );
    None
}

/// The body of an element that changes how a formula looks, not what it
/// says: its text is its body's.
fn transparent_body(content: &Content) -> Option<&Content> {
    if let Some(elem) = content.to_packed::<MidElem>() {
        Some(&elem.body)
    } else if let Some(elem) = content.to_packed::<CancelElem>() {
        Some(&elem.body)
    } else if let Some(elem) = content.to_packed::<LimitsElem>() {
        Some(&elem.body)
    } else if let Some(elem) = content.to_packed::<ScriptsElem>() {
        Some(&elem.body)
    } else if let Some(elem) = content.to_packed::<StretchElem>() {
        Some(&elem.body)
    } else {
        None
    }
}

/// One operand of an attachment, fraction, accent or annotation: its linear
/// form, in parentheses unless it is atomic ([`is_atomic`]).
fn part(content: &Content) -> String {
    let text = math_linear(content);
    if text.is_empty() || text.chars().count() == 1 || is_atomic(content) {
        text
    } else {
        format!("({text})")
    }
}

/// Whether an operand reads as one unit without parentheses: one symbol, a
/// number, a word, a delimited group, a matrix, vector, cases, binomial or
/// root, an operator, primes, a reference, or an accent over such a unit.
fn is_atomic(content: &Content) -> bool {
    let content = unwrapped(content);
    if let Some(text) = content.to_packed::<TextElem>() {
        let text = text.text.trim();
        let number = text.chars().all(|c| c.is_ascii_digit() || c == '.');
        let word = text.chars().all(char::is_alphabetic);
        return !text.is_empty() && (number || word);
    }
    if let Some(accent) = content.to_packed::<AccentElem>() {
        return is_atomic(&accent.base);
    }
    if let Some(class) = content.to_packed::<ClassElem>() {
        return is_atomic(&class.body);
    }
    if let Some(body) = transparent_body(content) {
        return is_atomic(body);
    }
    content.is::<SymbolElem>()
        || content.is::<LrElem>()
        || content.is::<MatElem>()
        || content.is::<VecElem>()
        || content.is::<CasesElem>()
        || content.is::<BinomElem>()
        || content.is::<RootElem>()
        || content.is::<OpElem>()
        || content.is::<PrimesElem>()
        || content.is::<RefElem>()
        || content.is::<CiteElem>()
}

/// `content` without what holds one element and adds no text: styles, an
/// equation around its body, a sequence of one element besides spaces.
fn unwrapped(content: &Content) -> &Content {
    if let Some(styled) = content.to_packed::<StyledElem>() {
        return unwrapped(&styled.child);
    }
    if let Some(equation) = content.to_packed::<EquationElem>() {
        return unwrapped(&equation.body);
    }
    if let Some(sequence) = content.to_packed::<SequenceElem>() {
        let mut kept = sequence.children.iter().filter(|child| !child.is::<SpaceElem>());
        if let (Some(only), None) = (kept.next(), kept.next()) {
            return unwrapped(only);
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use typst_library::foundations::{Dict, NativeElement};
    use typst_library::introspection::Introspector;
    use typst_library::math::EquationElem;

    use super::math_linear;
    use crate::CndDocument;
    use crate::world::CndWorld;

    /// The linear form of every equation in `source`, in document order.
    fn linear_forms(source: &str) -> Vec<String> {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("main.typ");
        std::fs::write(&path, source).expect("write the source");
        let world =
            CndWorld::new_with_options(&path, Dict::default(), false).expect("a world");
        let document = typst::compile::<CndDocument>(&world)
            .output
            .unwrap_or_else(|errors| panic!("{source:?} does not compile: {errors:?}"));
        document
            .introspector()
            .query(&EquationElem::ELEM.select())
            .iter()
            .map(|equation| {
                math_linear(&equation.to_packed::<EquationElem>().expect("equation").body)
            })
            .collect()
    }

    /// Each formula (written inline, between `$`) has the given linear form.
    #[track_caller]
    fn assert_linear(cases: &[(&str, &str)]) {
        let source: String =
            cases.iter().map(|(src, _)| format!("${src}$\n\n")).collect();
        let expected: Vec<_> =
            cases.iter().map(|(_, linear)| linear.to_string()).collect();
        let actual = linear_forms(&source);
        let pairs: Vec<_> = cases.iter().map(|(src, _)| *src).zip(actual).collect();
        let wanted: Vec<_> = cases.iter().map(|(src, _)| *src).zip(expected).collect();
        assert_eq!(pairs, wanted);
    }

    #[test]
    fn symbols_are_the_characters_typst_renders() {
        assert_linear(&[
            ("alpha beta", "α β"),
            ("x <= y", "x ≤ y"),
            ("x != y", "x ≠ y"),
            ("a -> b", "a → b"),
            ("integral f", "∫ f"),
            ("3.14 + oo", "3.14 + ∞"),
        ]);
    }

    #[test]
    fn spacing_is_one_space_where_the_source_or_typst_puts_one() {
        assert_linear(&[
            ("a   b", "a b"),
            ("x quad y", "x y"),
            ("x thin y", "x y"),
            ("a+b", "a+b"),
            ("x \"if\" y", "x if y"),
        ]);
    }

    #[test]
    fn attachments_are_carets_and_underscores() {
        assert_linear(&[
            ("x^2", "x^2"),
            ("x_1^2", "x_1^2"),
            ("x^10", "x^10"),
            ("x^(2n)", "x^(2n)"),
            ("x^(-1)", "x^(\u{2212}1)"),
            ("e^(x^2)", "e^(x^2)"),
            ("(a+b)^2", "(a+b)^2"),
            ("x_\"max\"", "x_max"),
            ("sum_(i=1)^n i", "∑_(i=1)^n i"),
        ]);
    }

    #[test]
    fn primes_follow_their_base() {
        assert_linear(&[("f'(x)", "f′(x)"), ("f''", "f″"), ("f'^2", "f′^2")]);
    }

    #[test]
    fn fractions_parenthesize_compound_operands() {
        assert_linear(&[
            ("1/3", "1/3"),
            ("(a+b)/(c+d)", "(a+b)/(c+d)"),
            ("x^2/2", "(x^2)/2"),
            ("1/2 x", "1/2 x"),
        ]);
    }

    #[test]
    fn roots_are_calls() {
        assert_linear(&[
            ("sqrt(x)", "sqrt(x)"),
            ("root(3, x)", "root(3, x)"),
            ("sqrt(x^2 + 1)", "sqrt(x^2 + 1)"),
            ("sqrt(x)/2", "sqrt(x)/2"),
        ]);
    }

    #[test]
    fn operators_are_words() {
        assert_linear(&[
            ("sin x", "sin x"),
            ("log_2 n", "log_2 n"),
            ("lim_(x->0) f(x)", "lim_(x→0) f(x)"),
        ]);
    }

    #[test]
    fn a_differential_binds_to_its_variable() {
        assert_linear(&[
            ("integral_0^1 x^2 dif x = 1/3", "∫_0^1 x^2 dx = 1/3"),
            ("f(x) dif x", "f(x) dx"),
            ("Dif y", "Dy"),
        ]);
    }

    #[test]
    fn delimiters_keep_their_characters() {
        assert_linear(&[
            ("abs(x)", "|x|"),
            ("norm(v)", "‖v‖"),
            ("floor(x)", "⌊x⌋"),
            ("( a + b )", "(a + b)"),
            ("[a, b]", "[a, b]"),
        ]);
    }

    #[test]
    fn matrices_vectors_cases_and_binomials() {
        assert_linear(&[
            ("mat(1, 2; 3, 4)", "[[1, 2], [3, 4]]"),
            ("vec(1, 2, 3)", "(1, 2, 3)"),
            ("cases(x &\"if\" x > 0, 0 &\"otherwise\")", "{x if x > 0; 0 otherwise}"),
            ("binom(n, k)", "binom(n, k)"),
        ]);
    }

    #[test]
    fn accents_and_lines_combine_with_their_base() {
        assert_linear(&[
            ("hat(x)", "x\u{302}"),
            ("arrow(v)", "v\u{20d7}"),
            ("overline(z)", "z\u{305}"),
            ("underbrace(a + b, n)", "(a + b)_n"),
            ("cancel(x)", "x"),
        ]);
    }

    #[test]
    fn styles_leave_the_letters_plain() {
        assert_linear(&[("upright(d)", "d"), ("bold(x) + italic(y)", "x + y")]);
    }

    #[test]
    fn line_breaks_become_semicolons() {
        assert_eq!(linear_forms("$ a &= b \\ &= c $"), ["a = b; = c"]);
    }

    #[test]
    fn references_are_their_labels() {
        assert_eq!(
            linear_forms(
                "#set heading(numbering: \"1.\")\n= Part <part>\n\n$a #ref(<part>)$"
            ),
            ["a @part"]
        );
    }
}
