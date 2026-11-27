// Decision EO-D4, record element members read whole as operands after a
// call: a queue or dynamic-array member of a record element of a queue or
// dynamic array (`rq[k].q`) that an equality compares, or that an
// alternative of a conditional operator assigned to a queue or dynamic array
// gives, after a call that changes `k` names the element selected after that
// call: `x = bump() + (rq[k].q == t)` and `t = bump() > 0 ? rq[k].q : t`
// read `rq[1].q`. A read before the call keeps the element the statement
// selects first.
//
// IEEE 1800-2009 11.4.2 (SystemVerilog-1800-2009.txt L14500): "The ordering
//   of assignment operations relative to any other operation within an
//   expression is undefined."
module tb;
  typedef struct { int q[$]; int d[]; } r_t;
  r_t rq[$];
  r_t rd[];
  int k, x;
  int t[$];
  int u[];
  function int bump();
    k = 1;
    return 10;
  endfunction
  initial begin
    r_t r0, r1;
    r0.q = '{1, 2}; r0.d = new[1];
    r1.q = '{3}; r1.d = new[2];
    rq.push_back(r0); rq.push_back(r1);
    rd = new[2]; rd[0] = r0; rd[1] = r1;
    t = '{3};
    u = new[2];
    k = 0; x = bump() + (rq[k].q == t); $display("eq x=%0d", x);
    k = 0; x = bump() + (t != rq[k].q); $display("ne x=%0d", x);
    k = 0; x = bump() + (rq[k].q === t); $display("case eq x=%0d", x);
    k = 0; x = bump() + (rq[k].q !== t); $display("case ne x=%0d", x);
    k = 0; x = bump() + (rd[k].d == u); $display("dynamic eq x=%0d", x);
    k = 0; x = (rq[k].q == t) + bump(); $display("eq before x=%0d", x);
    k = 0; x = bump() + (3 inside {rq[k].q}); $display("inside x=%0d", x);
    k = 0; t = bump() > 0 ? rq[k].q : t; $display("cond t=%p", t);
    t = '{3};
    k = 0; t = bump() < 0 ? t : rq[k].q; $display("cond else t=%p", t);
    t = '{3};
    k = 0; t = bump() > 0 ? (k > 0 ? rq[k].q : t) : t; $display("cond nested t=%p", t);
    k = 0; u = bump() > 0 ? rd[k].d : u; $display("cond dynamic u=%p", u);
    t = '{3};
    k = 0; t = rq[k].q.size() > 1 ? rq[k].q : t; $display("cond no call t=%p k=%0d", t, k);
    $finish(0);
  end
endmodule
