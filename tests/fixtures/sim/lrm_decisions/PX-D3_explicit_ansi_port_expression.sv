// Decision PX-D3: an explicitly named ANSI port may select and concatenate
// variables declared in the module body; the port connects to exactly the
// bits its expression names.
//
// IEEE 1800-2009 23.2.2.2 (SystemVerilog-1800-2009.txt L40678-40680):
//   "ANSI style port declarations can be explicitly named, allowing elements
//   of arrays and structures, concatenations of elements, and assignment
//   pattern expressions of elements declared in a module, interface, or
//   program to be specified on the port list."
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
    #1 $display("R=0 lo=%b hi=%b", lo, hi);
    rr = 1;
    #1 $display("R=1 lo=%b hi=%b", lo, hi);
    $finish;
  end
endmodule
