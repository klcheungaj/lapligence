// IEEE 1800-2009 18.5.9 L30079-30082: "Variables that are not explicitly
// ordered shall be solved with the last set of ordered variables. These
// values are deferred until as late as possible to assure a good distribution
// of values. Variables that are partially ordered shall be solved with the
// latest set of ordered variables so that all ordering constraints are met."
// The text does not say how an order between variables that share no
// constraint affects the others.
// Decision S30-D2 (llg reading): the orders of the enabled blocks form one
// graph over the whole problem; a variable's layer is its longest order path
// to a sink, larger first, and unordered variables join the last layer. Each
// layer is drawn uniformly (or by its dist weights) over the values that have
// a legal completion, given the earlier layers.
// Case 1: a before c, b unordered: a is uniform over 0..3; (b, c) are then
// uniform over the pairs a <= b <= c. Case 2: p before r, where r shares no
// constraint with p: p still comes first and is uniform over 0..3, although
// q, which p constrains, is unordered.
class chain_c;
  rand bit [1:0] a, b, c;
  constraint k { a <= b; b <= c; }
  constraint o { solve a before c; }
endclass

class apart_c;
  rand bit [1:0] p, q;
  rand bit r;
  constraint k { p <= q; }
  constraint o { solve p before r; }
endclass

module tb;
  function automatic bit near(int hits, int total, int num, int den);
    longint h, t, x, y, diff;
    h = longint'(hits);
    t = longint'(total);
    x = longint'(num);
    y = longint'(den);
    diff = h * y - t * x;
    return diff * diff <= 64'sd25 * t * x * (y - x);
  endfunction

  chain_c ch;
  apart_c ap;
  int na3, na0, nb0, np3;

  initial begin
    ch = new;
    for (int i = 0; i < 8000; i++) begin
      void'(ch.randomize());
      if (ch.a == 2'd3) na3 = na3 + 1;
      if (ch.a == 2'd0) begin
        na0 = na0 + 1;
        if (ch.b == 2'd0) nb0 = nb0 + 1;
      end
    end
    $display("a3 quarter=%0b b0|a0 two fifths=%0b", near(na3, 8000, 1, 4), near(nb0, na0, 2, 5));
    ap = new;
    for (int i = 0; i < 8000; i++) begin
      void'(ap.randomize());
      if (ap.p == 2'd3) np3 = np3 + 1;
    end
    $display("p3 quarter=%0b", near(np3, 8000, 1, 4));
    $finish;
  end
endmodule
