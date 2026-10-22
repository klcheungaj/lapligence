// IEEE 1800-2009 18.5.4 L29675-29678: "Absent any other constraints, the
// probability that the expression matches any value in the list is
// proportional to its specified weight. If there are constraints on some
// expressions that cause the distribution weights on these expressions to be
// not satisfiable, implementations are only required to satisfy the
// constraints. An exception to this rule is a weight of zero, which is
// treated as a constraint." L29684-29687: "The := operator assigns the
// specified weight to the item or, if the item is a range, to every value in
// the range. The :/ operator assigns the specified weight to the item or, if
// the item is a range, to the range as a whole." The text is silent on how a
// dist weighs the other random variables, on a dist under an implication or
// if, and on values that several items cover.
// Decision S30-D1 (llg choice): a dist that applies unconditionally is drawn
// before the other variables of its set, by item: an item is chosen with
// probability proportional to its mass (w * n for `:=` over n values, w for
// `:/`) among the items that still have a value with a legal completion,
// then a value of that item uniformly among those values; the other
// variables are drawn afterwards, given that value. Hence (1) other
// variables' completions do not weigh the expression's values; (2) an item
// keeps its whole mass when constraints remove some of its values;
// (3) overlapping items each keep their mass. A dist under a guard that
// depends on random variables weighs combinations by density instead (the
// docs row); the case checks only that the guard's false branch is
// unweighted.
// Each line prints whether a frequency lies within five standard deviations
// of the value the decision predicts.
class guard_c;
  rand bit g;
  rand bit [3:0] y;
  constraint k { if (g) y dist { 0 := 15, [1:15] := 1 }; }
endclass

class overlap_c;
  rand bit [1:0] v;
  constraint k { v dist { [0:1] := 1, [1:2] := 1 }; }
endclass

class completion_c;
  rand bit [1:0] x, y;
  constraint k { x dist { 1 := 1, 3 := 1 }; y <= x; }
endclass

class partial_c;
  rand bit [1:0] x, y;
  constraint k { x dist { [0:1] := 1, [2:3] := 1 }; x + y == 3'd4; }
endclass

module tb;
  function automatic bit near(int hits, int total, int num, int den);
    longint h, t, a, b, diff;
    h = longint'(hits);
    t = longint'(total);
    a = longint'(num);
    b = longint'(den);
    diff = h * b - t * a;
    return diff * diff <= 64'sd25 * t * a * (b - a);
  endfunction

  guard_c gc;
  overlap_c oc;
  completion_c cc;
  partial_c pc;
  int ng0, ny0, n1, n3, nx1;

  initial begin
    gc = new;
    for (int i = 0; i < 6000; i++) begin
      void'(gc.randomize());
      if (!gc.g) begin
        ng0 = ng0 + 1;
        if (gc.y == 4'd0) ny0 = ny0 + 1;
      end
    end
    $display("guard false y0 sixteenth=%0b", near(ny0, ng0, 1, 16));
    oc = new;
    for (int i = 0; i < 4000; i++) begin
      void'(oc.randomize());
      if (oc.v == 2'd1) n1 = n1 + 1;
    end
    $display("overlap one half=%0b", near(n1, 4000, 1, 2));
    cc = new;
    for (int i = 0; i < 6000; i++) begin
      void'(cc.randomize());
      if (cc.x == 2'd3) n3 = n3 + 1;
    end
    $display("completions x3 half=%0b", near(n3, 6000, 1, 2));
    pc = new;
    for (int i = 0; i < 6000; i++) begin
      void'(pc.randomize());
      if (pc.x == 2'd1) nx1 = nx1 + 1;
    end
    $display("partial item x1 half=%0b", near(nx1, 6000, 1, 2));
    $finish;
  end
endmodule
