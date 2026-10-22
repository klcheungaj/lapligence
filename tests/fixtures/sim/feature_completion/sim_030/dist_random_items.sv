// S30-D10: dist item values, range bounds and weights that read random
// variables. Those variables are solved first, over the values that leave
// the constraints satisfiable, and the dist then applies with them known.
class bound_c;
  rand bit [3:0] x;
  rand bit [1:0] w;
  constraint d { x dist { [0:w] :/ 1, 9 := 1 }; }
endclass

class both_c;
  rand bit [3:0] x;
  rand bit [1:0] w;
  constraint d { x dist { 1 := w, 2 := w }; }
endclass

class expr_c;
  rand bit [3:0] x;
  rand bit [1:0] w;
  constraint d { x dist { 1 := w * 2, 2 := 2 }; w inside {[1:2]}; }
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

  bound_c bc;
  both_c tc;
  expr_c ec;
  int legal, n9, w3, x0_w3, wzero, n1_w2, n_w2, w1;

  initial begin
    bc = new;
    legal = 1;
    for (int i = 0; i < 8000; i++) begin
      void'(bc.randomize());
      if (bc.x != 4'd9 && bc.x > {2'b00, bc.w}) legal = 0;
      if (bc.x == 4'd9) n9 = n9 + 1;
      if (bc.w == 2'd3) begin
        w3 = w3 + 1;
        if (bc.x == 4'd0) x0_w3 = x0_w3 + 1;
      end
    end
    $display("bound: legal=%0d 9 half=%0b w3 quarter=%0b x0|w3 eighth=%0b", legal,
             near(n9, 8000, 1, 2), near(w3, 8000, 1, 4), near(x0_w3, w3, 1, 8));
    tc = new;
    for (int i = 0; i < 4000; i++) begin
      if (!tc.randomize()) legal = 0;
      if (tc.w == 2'd0) wzero = wzero + 1;
    end
    $display("all-zero weights avoided: ok=%0d w0=%0d", legal, wzero);
    ec = new;
    for (int i = 0; i < 8000; i++) begin
      void'(ec.randomize());
      if (ec.w == 2'd1) w1 = w1 + 1;
      if (ec.w == 2'd2) begin
        n_w2 = n_w2 + 1;
        if (ec.x == 4'd1) n1_w2 = n1_w2 + 1;
      end
    end
    $display("expression: w1 half=%0b x1|w2 two thirds=%0b", near(w1, 8000, 1, 2),
             near(n1_w2, n_w2, 2, 3));
    $finish;
  end
endmodule
