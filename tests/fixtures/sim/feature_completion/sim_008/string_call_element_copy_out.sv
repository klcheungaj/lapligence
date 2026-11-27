// SIM-008, decision EO-D3: output and inout actuals of a string-returning or
// real-returning function called inside an expression. An actual naming an
// element of a queue, dynamic or associative array (integral and string
// keys) copies out when the call returns, before the rest of the expression
// reads on: packed, real and 4-state elements, a static function, a missing
// associative entry created by the copy-out and an index outside a queue that
// drops it. A packed variable or fixed-array element bound to such a formal
// copies out at the return too, and an input with an effect before an inout
// actual is evaluated before that actual is copied in. The call is a
// concatenation item, a display argument, a condition, a `$sformatf`
// argument or a real operand.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19185-19187): "an
//   inout argument is copied twice: once from the actual into the argument
//   when the subroutine is called and once from the argument into the actual
//   when the subroutine returns." 13.5 (L19070): "The order of evaluation of
//   the expressions in the argument list is undefined."
module tb;
  int q[$];
  int d[];
  int aa[string];
  real rq[$];
  logic [3:0] lq[$];
  int fa[3];
  int k, v;
  real r;
  string s;
  function int bump();
    k = 1;
    return 10;
  endfunction
  function automatic string f(output int x);
    x = 4;
    return "r";
  endfunction
  function automatic string g(int p, inout int x);
    x = x + p;
    return $sformatf("g%0d", x);
  endfunction
  function automatic string h(output real r, output logic [3:0] l);
    r = 2.5;
    l = 4'b10x1;
    return "h";
  endfunction
  function static string st(inout int x);
    x++;
    return "st";
  endfunction
  function automatic real rf(output int x);
    x = 6;
    return 1.5;
  endfunction
  function automatic real rr(inout real x);
    x = x * 2.0;
    return x + 0.25;
  endfunction
  initial begin
    q = '{0, 1, 2};
    d = new[2];
    aa["a"] = 7;
    rq = '{0.0};
    lq = '{4'b0};
    s = {f(q[0]), "!"}; $display("1 %s %p", s, q);
    k = 0; s = {g(bump(), q[k]), "!"}; $display("2 %s %p k=%0d", s, q, k);
    k = 0; s = {g(5, d[k]), g(bump(), d[k])}; $display("3 %s %p", s, d);
    s = {g(3, aa["a"]), g(1, aa["b"])}; $display("4 %s %0d %0d %0d", s, aa["a"], aa["b"], aa.num());
    s = {h(rq[0], lq[0]), "."}; $display("5 %s %p %b", s, rq, lq[0]);
    s = {st(q[2]), "-"}; $display("6 %s %p", s, q);
    s = {g(1, q[9]), "?"}; $display("7 %s %p", s, q);
    v = 5; s = {g(1, v), "+"}; $display("8 %s %0d", s, v);
    fa = '{1, 2, 3}; k = 0; s = {g(bump(), fa[k]), "#"}; $display("9 %s %p", s, fa);
    $display("10 %s %p", f(q[1]), q);
    if (f(q[2]) == "r") $display("11 %p", q);
    s = $sformatf("%s/%0d", g(2, q[0]), q[0]); $display("12 %s", s);
    rq = '{1.0, 2.0};
    r = rf(q[1]) + 1.0; $display("13 %0.2f %p", r, q);
    k = 0; r = rr(rq[k]) + rr(rq[k]); $display("14 %0.2f %p", r, rq);
    $finish(0);
  end
endmodule
