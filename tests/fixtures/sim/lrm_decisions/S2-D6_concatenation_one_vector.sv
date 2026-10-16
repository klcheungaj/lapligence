// Decision S2-D6: a delayed continuous assignment to a concatenation, or to a
// vector net some of whose bits are aliased to another net, is one vector
// driver: the 10.3.3 rule picks one delay from the whole old and new value
// and every part changes together. llg does not split the target into
// separately delayed parts, so 01 -> 10 rises in both parts, and the aliased
// bit is delayed once.
//
// IEEE 1800-2009 11.4.12 (SystemVerilog-1800-2009.txt L15235-15236): "The
// concatenation is treated as a packed vector of bits. It can be used on the
// left-hand side of an assignment or in an expression." 10.3.3 (L12895-12903):
// "If the left-hand references a vector net, then up to three delays can be
// applied. The following rules determine which delay controls the
// assignment: - If the right-hand side makes a transition from nonzero to
// zero, then the falling delay shall be used. - If the right-hand side makes a
// transition to z, then the turn-off delay shall be used. - For all other
// cases, the rising delay shall be used." 10.11 (L13981): "An alias statement
// declares multiple names for the same physical net, or bits within a net."
`timescale 1ns/1ns
module tb;
  reg [1:0] a;
  wire p, q;
  wire [1:0] w;
  wire w0;
  alias w[0] = w0;
  assign #(10,5) {p, q} = a;
  assign #(10,5) w = a;
  initial begin
    $monitor("%0t p=%b q=%b w=%b w0=%b", $time, p, q, w, w0);
    a = 2'b01;
    #20 a = 2'b10;
    #20 a = 2'b00;
    #20 $finish;
  end
endmodule
