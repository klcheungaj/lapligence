// SIM-030: solve...before (IEEE 1800-2009 18.5.9, decision S30-D2). Lines
// print relations every legal solution satisfies and frequencies checked
// against bounds fixed in advance (five standard deviations).
class sd_c;
  rand bit s;
  rand bit [31:0] d;
  constraint c { s -> d == 0; }
  constraint order { solve s before d; }
endclass

class chain_c;
  rand bit [1:0] a, b, c;
  constraint mono { a <= b; b <= c; }
  constraint order { solve a before b; solve b before c; }
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

  sd_c sd;
  chain_c ch;
  int ns, nbad, na3, nc3, nb, n;
  bit s;
  bit [31:0] d;

  initial begin
    // The LRM example: s is chosen first, so it is 1 half the time and the
    // order never changes the solution space.
    sd = new;
    for (int i = 0; i < 4000; i++) begin
      void'(sd.randomize());
      if (sd.s) ns = ns + 1;
      if (sd.s && sd.d != 0) nbad = nbad + 1;
    end
    $display("ordered: bad=%0d s=%0b", nbad, near(ns, 4000, 1, 2));

    // A disabled block's order does not apply: s == 1 is then one legal
    // combination of 2**32 + 1.
    sd.order.constraint_mode(0);
    ns = 0;
    for (int i = 0; i < 2000; i++) begin
      void'(sd.randomize());
      if (sd.s) ns = ns + 1;
    end
    $display("order disabled: s=%0d", ns);

    // An order in an inline constraint and in std::randomize.
    ns = 0;
    for (int i = 0; i < 4000; i++) begin
      void'(sd.randomize() with { solve s before d; });
      if (sd.s) ns = ns + 1;
    end
    $display("inline order: s=%0b", near(ns, 4000, 1, 2));
    ns = 0;
    nbad = 0;
    for (int i = 0; i < 4000; i++) begin
      void'(std::randomize(s, d) with { s -> d == 0; solve s before d; });
      if (s) ns = ns + 1;
      if (s && d != 0) nbad = nbad + 1;
    end
    $display("scope order: bad=%0d s=%0b", nbad, near(ns, 4000, 1, 2));

    // A chain: a uniform over 0..3, then b over a..3, then c over b..3.
    // Unordered, a == 3 would be one combination of 20.
    ch = new;
    nbad = 0;
    for (int i = 0; i < 8000; i++) begin
      void'(ch.randomize());
      if (!(ch.a <= ch.b && ch.b <= ch.c)) nbad = nbad + 1;
      if (ch.a == 3) na3 = na3 + 1;
      if (ch.a == 0) begin
        n = n + 1;
        if (ch.b == 0) nb = nb + 1;
      end
      if (ch.b == 2 && ch.c == 3) nc3 = nc3 + 1;
    end
    // P(b == 2 and c == 3) = sum over a <= 2 of 1/4 * 1/(4 - a) * 1/2 = 13/96.
    $display("chain: bad=%0d a3=%0b b0|a0=%0b b2c3=%0b", nbad, near(na3, 8000, 1, 4),
             near(nb, n, 1, 4), near(nc3, 8000, 13, 96));
    $finish;
  end
endmodule
