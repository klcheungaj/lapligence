// Decision EO-D4, record element members read after a call: a queue or
// dynamic-array member of a record element of a queue or dynamic array
// (`rq[k].q`) that a statement reads as a packed value through a method, a
// query or a select after a call or an assignment operator that changes `k`
// names the element selected after that effect (`x = bump() +
// rq[k].q.size()` reads `rq[1].q`),
// in assignments, conditions, system-task arguments and actuals, also in the
// value of an element target located after it (`rq[k].q[0] = bump() +
// rq[k].q.size()`); a read before the effect keeps the element the statement
// selects first.
//
// IEEE 1800-2009 10.4.1 (SystemVerilog-1800-2009.txt L13026-13027): "The
//   order of evaluation of the variable_lvalue and the expression on the
//   right-hand side is undefined if a timing control is not specified."
// IEEE 1800-2009 11.4.2 (L14500): "The ordering of assignment operations
//   relative to any other operation within an expression is undefined."
// IEEE 1800-2009 13.5 (L19070): "The order of evaluation of the expressions
//   in the argument list is undefined."
module tb;
  typedef struct { int q[$]; int v; int d[]; } r_t;
  r_t rq[$];
  r_t rd[];
  int k, x;
  bit c = 1;
  function int bump();
    k = 1;
    return 10;
  endfunction
  function int two(int a, int b);
    return a * 100 + b;
  endfunction
  function int total(int a, int b[$]);
    return a + b.sum();
  endfunction
  task automatic show(string tag);
    $display("%s k=%0d x=%0d q0=%p q1=%p", tag, k, x, rq[0].q, rq[1].q);
  endtask
  initial begin
    r_t r0, r1;
    r0.q = '{1}; r0.v = 5; r0.d = new[1];
    r1.q = '{1, 2, 3}; r1.v = 6; r1.d = new[4];
    rq.push_back(r0); rq.push_back(r1);
    rd = new[2]; rd[0] = r0; rd[1] = r1;
    k = 0; x = bump() + rq[k].q.size(); show("size");
    k = 0; x = bump() + rq[k].q[2]; show("select");
    k = 0; x = rq[k].q.size() + bump(); show("before");
    k = 0; x = bump() + rd[k].d.size(); show("dynamic");
    k = 0; x = bump() + rq[k].q.sum() + rq[k].q.size(); show("twice");
    k = 0; $display("display %0d %0d", bump(), rq[k].q.size());
    k = 0; x = bump() + rq[k].v; show("packed");
    k = 0; x = c ? bump() + rq[k].q.size() : 0; show("conditional");
    k = 0; x = two(bump(), rq[k].q.size()); show("actual");
    k = 0; x = (k = 1) + rq[k].q.size(); show("nested");
    k = 0; x = bump() + rq[k].q.size() + rq[0].q.size(); show("constant");
    k = 0; x = rq[k].q.size() + bump() + rq[k].q.size(); show("around");
    k = 0; if (bump() + rq[k].q.size() == 13) x = 1; else x = 2; show("if");
    k = 0; x = bump() + $size(rq[k].q); show("query");
    k = 0; x = bump() + rq[k].q[$]; show("last");
    k = 0; x = bump() + (rq[k].q.size() > 2 ? 5 : 6); show("operand");
    k = 0; x = bump() + total(1, rq[k].q); show("whole actual");
    k = 0; rq[k].q[0] = bump() + rq[k].q.size(); show("target");
    $finish(0);
  end
endmodule
