// SIM-030: dist weights (IEEE 1800-2009 18.5.4, decision S30-D1). Each line
// prints relations every legal solution satisfies and frequencies checked
// against bounds fixed in advance (five standard deviations), so the output
// does not depend on the values drawn.
class weights_c;
  rand bit [7:0] x;
  int unsigned w200 = 3;
  constraint cx { x dist { 100 := 1, 200 := w200, [10:12] :/ 3, 0 := 0 }; }
endclass

class guard_c;
  rand bit g;
  rand bit [3:0] y;
  constraint cy { if (g) y dist { 0 := 5, [1:15] :/ 5 }; }
endclass

class overlap_c;
  rand bit [1:0] v;
  constraint cv { v dist { [0:1] := 1, [1:2] := 1 }; }
endclass

class signed_c;
  rand byte s;
  constraint cs { s dist { [-128:-1] :/ 1, [0:127] :/ 3 }; }
endclass

module tb;
  // |hits/n - num/den| within five standard deviations.
  function automatic bit near(int hits, int n, int num, int den);
    longint h, t, a, b, d;
    h = longint'(hits);
    t = longint'(n);
    a = longint'(num);
    b = longint'(den);
    d = h * b - t * a;
    return d * d <= 64'sd25 * t * a * (b - a);
  endfunction

  weights_c w;
  guard_c g;
  overlap_c o;
  signed_c sg;
  int n100, n200, nrange, n11, nbad, ng, ny0g, ny0n, nv[4], nneg;

  initial begin
    w = new;
    for (int i = 0; i < 7000; i++) begin
      if (!w.randomize()) nbad = nbad + 1;
      if (w.x == 100) n100 = n100 + 1;
      else if (w.x == 200) n200 = n200 + 1;
      else if (w.x >= 8'd10 && w.x <= 8'd12) nrange = nrange + 1;
      else nbad = nbad + 1;
      if (w.x == 11) n11 = n11 + 1;
    end
    $display("mixed: bad=%0d 100=%0b 200=%0b range=%0b 11=%0b", nbad,
             near(n100, 7000, 1, 7), near(n200, 7000, 3, 7), near(nrange, 7000, 3, 7),
             near(n11, 7000, 1, 7));

    // A weight read from state, now zero: a constraint excluding 200.
    w.w200 = 0;
    n100 = 0; n200 = 0; nrange = 0; nbad = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!w.randomize()) nbad = nbad + 1;
      if (w.x == 100) n100 = n100 + 1;
      else if (w.x == 200) n200 = n200 + 1;
      else if (w.x >= 8'd10 && w.x <= 8'd12) nrange = nrange + 1;
      else nbad = nbad + 1;
    end
    $display("zero weight: bad=%0d 200=%0d 100=%0b range=%0b", nbad, n200,
             near(n100, 4000, 1, 4), near(nrange, 4000, 3, 4));

    // Another constraint removes 100: the remaining weights keep their ratio.
    w.w200 = 3;
    n100 = 0; n200 = 0; nrange = 0; nbad = 0;
    for (int i = 0; i < 4000; i++) begin
      if (!w.randomize() with { x != 8'd100; }) nbad = nbad + 1;
      if (w.x == 100) n100 = n100 + 1;
      else if (w.x == 200) n200 = n200 + 1;
      else if (w.x >= 8'd10 && w.x <= 8'd12) nrange = nrange + 1;
      else nbad = nbad + 1;
    end
    $display("with constraint: bad=%0d 100=%0d 200=%0b", nbad, n100, near(n200, 4000, 1, 2));

    // The weights apply where the guard holds; g itself stays uniform.
    g = new;
    for (int i = 0; i < 8000; i++) begin
      void'(g.randomize());
      if (g.g) begin
        ng = ng + 1;
        if (g.y == 0) ny0g = ny0g + 1;
      end else if (g.y == 0) ny0n = ny0n + 1;
    end
    $display("guard: g=%0b y0|g=%0b y0|!g=%0b", near(ng, 8000, 1, 2), near(ny0g, ng, 1, 2),
             near(ny0n, 8000 - ng, 1, 16));

    // Overlapping items add their weights; 3 is in no item.
    o = new;
    for (int i = 0; i < 4000; i++) begin
      void'(o.randomize());
      nv[o.v] = nv[o.v] + 1;
    end
    $display("overlap: 3=%0d 0=%0b 1=%0b 2=%0b", nv[3], near(nv[0], 4000, 1, 4),
             near(nv[1], 4000, 1, 2), near(nv[2], 4000, 1, 4));

    // Signed ranges.
    sg = new;
    for (int i = 0; i < 4000; i++) begin
      void'(sg.randomize());
      if (sg.s < 0) nneg = nneg + 1;
    end
    $display("signed: negative=%0b", near(nneg, 4000, 1, 4));
    $finish;
  end
endmodule
