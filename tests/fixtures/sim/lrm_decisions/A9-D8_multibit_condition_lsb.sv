// Decision A9-D8: a multi-bit `&&&` condition is decided by its least
// significant bit: 2'b01 and 2'bx1 enable the check, 2'b10 and 2'b0x do not
// (a plain expression is deterministic, so x disables).
//
// IEEE 1800-2009 31.7 (SystemVerilog-1800-2009.txt L52196-52197): "if a
// vector net or an expression resulting in a multibit value is used, then
// the least significant bit of the vector net or the expression value is
// used."
`timescale 1ns/1ns
module k(input d, input c, input [1:0] en); reg n;
  specify $setup(d, posedge c &&& en, 10, n); endspecify
endmodule
module tb;
  reg c, d;
  reg [7:0] en;                  // per instance: 01, 10, x1, 0x
  k a0(d, c, en[1:0]); k a1(d, c, en[3:2]); k a2(d, c, en[5:4]); k a3(d, c, en[7:6]);
  initial $monitor("%0t n=%b%b%b%b", $time, a3.n, a2.n, a1.n, a0.n);
  initial begin
    c = 0; d = 0; en = 8'b0x_x1_10_01;
    #95 d = 1;
    #5 c = 1;
    #10 $finish;
  end
endmodule
