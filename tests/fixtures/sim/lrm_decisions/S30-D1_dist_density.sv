// IEEE 1800-2009 18.5.4 L29675-29678: "Absent any other constraints, the
// probability that the expression matches any value in the list is
// proportional to its specified weight. If there are constraints on some
// expressions that cause the distribution weights on these expressions to be
// not satisfiable, implementations are only required to satisfy the
// constraints. An exception to this rule is a weight of zero, which is
// treated as a constraint." The text is silent on how a dist weighs the other
// random variables, on a dist under an implication or if, and on values that
// several items cover.
// Decision S30-D1 (llg choice): legal value combinations are weighted. Where
// a dist applies (its guard holds), a combination weighs |S| * w(v) / W: the
// density of the weights relative to uniform over the values S the items
// cover, where w(v) sums the per-value weights of the items holding v and W
// sums w over S; elsewhere it weighs 1. Hence (1) a guard variable keeps its
// distribution when the dist covers its expression's whole range, whatever
// the weights; (2) overlapping items add their weights; (3) other variables'
// legal completions also weigh the expression's values.
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
  int ng, ny0, n1, n3;

  initial begin
    gc = new;
    for (int i = 0; i < 6000; i++) begin
      void'(gc.randomize());
      if (gc.g) begin
        ng = ng + 1;
        if (gc.y == 4'd0) ny0 = ny0 + 1;
      end
    end
    $display("guard uniform=%0b y0 half=%0b", near(ng, 6000, 1, 2), near(ny0, ng, 1, 2));
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
    $display("completions x3 two thirds=%0b", near(n3, 6000, 2, 3));
    $finish;
  end
endmodule
