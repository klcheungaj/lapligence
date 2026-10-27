// ADV-015b: a checker formal passed to a let, inside or outside the checker,
// stands for the sampled value of its actual (17.3, 11.13): ordered, named
// and default arguments, a let whose body is one formal, a let whose body
// calls another let, two calls of one let in one expression, two instances.
`timescale 1ns / 1ns
let outer_add(x, y = 4'd0) = x + y;
let ident(x) = x;
let twice(x) = outer_add(x, x);
checker c(logic [3:0] a, logic [3:0] b, event clk);
  let inner_inc(x) = x + 4'd1;
  bit [3:0] v_pair, v_named, v_ident, v_nested, v_mixed, v_inner;
  always @(clk) begin
    v_pair <= outer_add(a, 4'd1) + outer_add(b);
    v_named <= outer_add(.y(b), .x(4'd2));
    v_ident <= ident(a);
    v_nested <= twice(b);
    v_inner <= inner_inc(a);
  end
  always @(clk) v_mixed <= ident(a) - ident(b);
  c1: cover property (@clk 1)
    $display("%0t %m pair=%0d named=%0d ident=%0d nested=%0d mixed=%0d inner=%0d act=%0d",
             $time, v_pair, v_named, v_ident, v_nested, v_mixed, v_inner, outer_add(a, b));
endchecker
module tb;
  logic clk = 1'b0;
  logic [3:0] d = 4'd3, e = 4'd1;
  c u(d, e, posedge clk);
  c w(.a(e), .b(d), .clk(posedge clk));
  // The actuals change in the Active region of each clock edge; the
  // checker reads their Preponed values.
  always @(posedge clk) begin
    d = d + 4'd5;
    e = e + 4'd2;
  end
  always #5 clk = ~clk;
  initial #22 $finish;
endmodule
