// A formula's text is a readable linear form of what Typst renders; its
// source is the `raw` of the node (block) or of the `math` mark (inline).
#set document(title: "Linear math", author: "typst-cnd")

$ integral_0^1 x^2 dif x = 1/3 $ <eq-integral>

$ mat(1, 2; 3, 4) vec(x, y) = vec(5, 6) $ <eq-matrix>

$ f(x) = cases(x &"if" x > 0, 0 &"otherwise") $ <eq-cases>

#math.equation(block: true, alt: "a squared plus b squared", $ a^2 + b^2 $) <eq-alt>

Greek: $alpha beta <= gamma$ in a paragraph.

A root: $sqrt(x^2 + 1)/2$ here.

With alt: #math.equation(alt: "x squared", $x^2$) here.
