// References and citations written inside inline math are edges of the
// text holding the equation, with no span: the formula is one atomic mark.
// A block equation is a math node, which holds no edge at all. A reference
// written in a heading or a caption is an edge of that text.
#set document(title: "Edges in math and labels", author: "typst-cnd")
#set heading(numbering: "1.")
#set math.equation(numbering: "(1)")

= Target <sec-target>

Inline math $a #ref(<sec-target>)$ in a paragraph, and a citation
$e #cite(<knuth>)$ beside it.

$ b = c #ref(<sec-target>) $ <eq-block>

After the block equation.

#figure(rect[r], caption: [Caption with $d #ref(<sec-target>)$ math]) <fig-math>

/ Term $f #ref(<sec-target>)$: The term's definition.

= Section naming @sec-target <sec-naming>

#figure(rect[s], caption: [A caption naming @fig-math.]) <fig-naming>

#bibliography("refs.yml")
