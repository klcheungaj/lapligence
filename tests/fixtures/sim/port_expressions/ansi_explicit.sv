// Explicitly named ANSI ports whose expressions select and concatenate
// variables declared in the module body.
module m(output .P1(r[3:0]), output .P2(r[7:4]), input .Y({x, y}), input R);
  logic [7:0] r;
  logic [1:0] x;
  logic y;
  always_comb r = {R, x, y, R, x, y};
endmodule

module tb;
  wire [3:0] lo, hi;
  logic [2:0] d;
  logic rr;
  m c1(.P1(lo), .P2(hi), .Y(d), .R(rr));
  initial begin
    d = 3'b101; rr = 0;
    #1 $display("1 lo=%b hi=%b", lo, hi);
    rr = 1;
    #1 $display("2 lo=%b hi=%b", lo, hi);
    $finish;
  end
endmodule
