// IEEE 1800-2009 18.5.4 L29681-29683: "The distribution set is a
// comma-separated list of integral expressions and ranges. ... The weight can
// be any integral SystemVerilog expression." The text does not say how a
// weight, item value or range bound that is itself random weighs the
// solutions.
// Decision S30-D10 (llg reading): the random variables that dist item
// values, range bounds and weights read are solved first, uniformly over the
// values that leave the constraints satisfiable (an item holds only where its
// weight is nonzero), as if ordered before the distributed expression; the
// dist then applies with those values known. A zero weight stays a
// constraint for every value of the weight.
// Each frequency line prints whether a frequency lies within five standard
// deviations of the value the decision predicts; the weight case orders w
// before x explicitly, so it does not depend on the implicit order.
class weight_c;
  rand bit [3:0] x;
  rand bit [1:0] w;
  constraint d { x dist { 1 := w, 2 := 1 }; }
  constraint o { solve w before x; }
endclass

class item_c;
  rand bit [3:0] x;
  rand bit [1:0] w;
  constraint d { x dist { w := 1, 9 := 1 }; }
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

  weight_c wc;
  item_c ic;
  int legal, zero_one, n3, x1_w3, n9;

  initial begin
    wc = new;
    legal = 1;
    for (int i = 0; i < 8000; i++) begin
      void'(wc.randomize());
      if (wc.x != 4'd1 && wc.x != 4'd2) legal = 0;
      if (wc.w == 2'd0 && wc.x == 4'd1) zero_one = zero_one + 1;
      if (wc.w == 2'd3) begin
        n3 = n3 + 1;
        if (wc.x == 4'd1) x1_w3 = x1_w3 + 1;
      end
    end
    $display("x=1 or 2: %0d, zero weight excludes 1: %0d", legal, zero_one == 0);
    $display("w=3: x1 three quarters=%0b", near(x1_w3, n3, 3, 4));
    ic = new;
    legal = 1;
    for (int i = 0; i < 8000; i++) begin
      void'(ic.randomize());
      if (ic.x != {2'b00, ic.w} && ic.x != 4'd9) legal = 0;
      if (ic.x == 4'd9) n9 = n9 + 1;
    end
    $display("x=w or 9: %0d, 9 half=%0b", legal, near(n9, 8000, 1, 2));
    $finish;
  end
endmodule
