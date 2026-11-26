// Decision EO-D1, targets: an assignment operator nested in an expression
// runs first, as decision EO-D1 states, whatever its target: a fixed-array
// element, slice or whole array, a record or a record member, a queue,
// dynamic or associative array or an element of one, a class property or an
// element of a class property array, a string, a string element and a class
// handle. An element, bit or part target of a value with a call or an
// assignment is located after the value, as the target of an assignment
// statement is (decision EO-D4), and one whose full expression evaluates it
// first is located before the other operands. The expression reads the value
// that the assignment stacked, of the target's type: a string, a handle or
// an unpacked value where the target is one.
//
// IEEE 1800-2009 11.4.2 (SystemVerilog-1800-2009.txt L14500): "The ordering
//   of assignment operations relative to any other operation within an
//   expression is undefined."
// IEEE 1800-2009 11.3.6 (L14463-14465): "The semantics of such an assignment
//   expression is that of a function that evaluates the right-hand side,
//   casts the right-hand side to the left-hand data type, stacks it, updates
//   the left-hand side, and returns the stacked value. The data type of the
//   value that is returned is the data type of the left-hand side."
// IEEE 1800-2009 10.4.1 (L13025-13027): "The order of evaluation of the
//   variable_lvalue and the expression on the right-hand side is undefined if
//   a timing control is not specified."
class C;
  int v;
  int a[2];
  int q[$];
  string s;
  C n;
endclass
module tb;
  typedef struct { int a; int b; } st_t;
  int arr[2], arr2[2], arr4[4], sl[2], fa[3];
  int qi[$], qi2[$], d[], d2[], aa[string];
  st_t st, st2, st3;
  bit [7:0] b8;
  bit bb;
  logic [7:0] l8;
  real r;
  string s, s2, sa[2], sq[$];
  string sm[string];
  C h, h2, h3, hq[$];
  int x, k;
  function automatic int bump();
    k = k + 1;
    return k;
  endfunction
  function automatic string gs(int i);
    return $sformatf("g%0d", i);
  endfunction
  function automatic void show(string a, string b);
    $display("show %s %s", a, b);
  endfunction
  initial begin
    h = new; h3 = new; h3.v = 5;
    arr2 = (arr = '{3, 4}); $display("A %p %p", arr, arr2);
    x = (arr[1] = 7) + 1; $display("B %0d %p", x, arr);
    sl = (arr4[1:2] = '{5, 6}); $display("C %p %p", sl, arr4);
    st3.a = 7; st3.b = 8;
    st2 = (st = st3); $display("D %p %p", st, st2);
    x = (st.a = 5) + 1; $display("E %0d %p", x, st);
    qi = '{1, 2, 3};
    x = (qi[0] = 9) + 1; $display("F %0d %p", x, qi);
    x = (qi[1] += 5) + 1; $display("G %0d %p", x, qi);
    qi2 = (qi = '{4, 5}); $display("H %p %p", qi, qi2);
    d2 = (d = '{6, 7}); $display("I %p %p", d, d2);
    d = new[3];
    x = (d[1] = 5) + 1; $display("J %0d %p", x, d);
    x = (aa["k"] = 4) + 1; $display("K %0d %p", x, aa);
    x = (aa["z"] += 3) + 1; $display("L %0d %p", x, aa);
    x = (h.v = 3) + 1; $display("M %0d %0d", x, h.v);
    x = (h.a[1] = 8) + 1; $display("N %0d %p", x, h.a);
    arr2 = (h.a = '{11, 12}); $display("O %p %p", arr2, h.a);
    x = (h.q[0] = 4) + 1; $display("P %0d %p", x, h.q);
    h.q.push_back(6);
    x = (h.q[1] = 7) + bump(); $display("Q %0d %p", x, h.q);
    x = (b8 = 300) + 1; $display("R %0d %0d", x, b8);
    x = (bb = 1'bx) + 1; $display("S %0d %0d", x, bb);
    x = (l8 = 9'h1ff) + 0; $display("T %0d %0d", x, l8);
    x = (r = 2.75) + 1; $display("U %0d %f", x, r);
    k = 0; fa = '{0, 0, 0};
    x = (fa[k] = bump()) + fa[0] + fa[1]; $display("V %0d %p", x, fa);
    x = (h.a[0] = 3) + (h.a[1] = 4) + h.a[0]; $display("W %0d %p", x, h.a);
    k = 0; qi = '{1, 7, 3};
    x = (qi[k] = 10) + bump() + qi[0]; $display("X %0d %p %0d", x, qi, k);
    $display("Y %0d %p", (qi[2] = 11), qi);
    s = "x0";
    s2 = (s = "abc"); $display("Z %s %s", s, s2);
    $display("a %s %s", (s = "pq"), s);
    k = 0;
    s2 = {(sa[k] = gs(bump())), sa[0], sa[1]}; $display("b %s %p %0d", s2, sa, k);
    sq.push_back("q0");
    s2 = {(sq[0] = "qq"), sq[0]}; $display("c %s %p", s2, sq);
    show((s = "u"), s);
    x = ((s = "xy") == "xy"); $display("d %0d %s", x, s);
    sm["a"] = "m0";
    s2 = {(sm["a"] = "mm"), sm["a"]}; $display("e %s %p", s2, sm);
    s2 = {(h.s = "hs"), h.s}; $display("f %s %s", s2, h.s);
    s = "z";
    s2 = {s, (s = "w")}; $display("g %s %s", s2, s);
    h.v = 1;
    h2 = (h = h3); $display("h %0d %0d", h.v, h2.v);
    x = ((h = h3) == h3); $display("i %0d", x);
    h.n = new; h.n.v = 9;
    h2 = (h.n = h3); $display("j %0d %0d", h.n.v, h2.v);
    hq.push_back(h);
    h2 = (hq[0] = h3); $display("k %0d", h2.v);
    $finish;
  end
endmodule
