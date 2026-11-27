// Decision EO-D4, record element members read whole or as strings after a
// call: a queue or dynamic-array member of a record element of a queue or
// dynamic array (`rq[k].q`) that a statement reads whole in place (an item of
// an unpacked array concatenation, a `%p` argument, an actual of a container
// formal) or whose string element it reads (`rq[k].tags[0]`) after a call that
// changes `k` names the element selected after that call: `t = {bump(),
// rq[k].q}` and `x = total(bump(), rq[k].q)` read `rq[1].q`. A read before
// the call keeps the element the statement selects first, and an index
// effect inside the select does not move the element it selects.
//
// IEEE 1800-2009 11.4.2 (SystemVerilog-1800-2009.txt L14500): "The ordering
//   of assignment operations relative to any other operation within an
//   expression is undefined."
// IEEE 1800-2009 13.5 (L19070): "The order of evaluation of the expressions
//   in the argument list is undefined."
module tb;
  typedef struct { int q[$]; string tags[$]; int d[]; } r_t;
  r_t rq[$];
  r_t rd[];
  int k, x;
  int t[$];
  string s;
  function int bump();
    k = 1;
    return 10;
  endfunction
  function string bumps();
    k = 1;
    return "<";
  endfunction
  function int total(int a, int b[$]);
    return a + b.sum();
  endfunction
  function int dtotal(int a, int b[]);
    return a + b.size();
  endfunction
  function int stotal(int a, string b);
    return a + b.len();
  endfunction
  function string stotal_s(int a, int b[$]);
    return $sformatf("%0d", a + b.sum());
  endfunction
  task tshow(int a, int b[$]);
    $display("tshow %0d %p", a, b);
  endtask
  initial begin
    r_t r0, r1;
    r0.q = '{1}; r0.tags = '{"a"}; r0.d = new[1];
    r1.q = '{1, 2, 3}; r1.tags = '{"bb", "cc"}; r1.d = new[4];
    rq.push_back(r0); rq.push_back(r1);
    rd = new[2]; rd[0] = r0; rd[1] = r1;
    k = 0; t = {bump(), rq[k].q}; $display("concat k=%0d t=%p", k, t);
    k = 0; t = {bump(), rd[k].d}; $display("concat dynamic t=%p", t);
    k = 0; t = {rq[k].q, bump()}; $display("concat before t=%p", t);
    k = 0; t = {bump(), rq[k].q, rq[0].q}; $display("concat two t=%p", t);
    k = 0; $display("p %0d %p", bump(), rq[k].q);
    k = 0; $write("w %0d %p\n", bump(), rq[k].q);
    k = 0; s = $sformatf("%0d %p", bump(), rq[k].q); $display("sformatf %s", s);
    k = 0; $display("p before %p %0d", rq[k].q, bump());
    k = 0; x = total(bump(), rq[k].q); $display("actual x=%0d", x);
    k = 0; x = dtotal(bump(), rd[k].d); $display("dynamic actual x=%0d", x);
    k = 0; tshow(bump(), rq[k].q);
    k = 0; s = stotal_s(bump(), rq[k].q); $display("string function s=%s", s);
    k = 0; x = total(bump(), rq[k].q) + total(0, rq[k].q); $display("two calls x=%0d", x);
    k = 0; x = bump() + total(1, rq[k].q); $display("outside x=%0d", x);
    k = 0; s = {bumps(), rq[k].tags[0]}; $display("string concat s=%s", s);
    k = 0; $display("s %0d %s", bump(), rq[k].tags[0]);
    k = 0; x = (bump() > 0 && rq[k].tags[0] == "bb"); $display("compare x=%0d", x);
    k = 0; x = stotal(bump(), rq[k].tags[0]); $display("string actual x=%0d", x);
    k = 0; x = bump() + rq[k].tags[0].len(); $display("len x=%0d", x);
    k = 0; s = rq[k].tags[(bump() - 10)]; $display("index s=%s k=%0d", s, k);
    $finish(0);
  end
endmodule
