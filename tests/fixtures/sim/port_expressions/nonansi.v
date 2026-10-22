// Verilog-2001 port expressions: a named concatenation, an implicit
// part-select and a named bit-select.
module m(.p({a, b}), c[1:0], .d(e[2]));
  output a;
  output [1:0] b;
  output [3:0] c;
  input [3:0] e;
  reg a;
  reg [1:0] b;
  reg [3:0] c;
  initial begin a = 1'b1; b = 2'b01; c = 4'b0110; end
endmodule

module tb;
  wire [2:0] pm;
  wire [2:0] pn;
  wire [1:0] cm;
  m c1(.p(pm), .d(1'b1));
  m c2(pn, cm, 1'b0);
  initial begin
    #2 $display("pm=%b pn=%b cm=%b e1=%b e2=%b", pm, pn, cm, c1.e, c2.e);
    $finish;
  end
endmodule
