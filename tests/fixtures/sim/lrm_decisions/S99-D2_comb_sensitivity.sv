// Decision S99-D2: the implicit sensitivity of an always_comb, always_latch
// or @* procedure does not include the operands of the procedural concurrent
// assertions in it. The procedure only queues the assertion; the attempt
// samples its operands at the assertion clock, so the procedure reads none
// of them. (llg rejects the const casts and automatic variables whose values
// would be taken by the procedure when it queues the instance.)
//
// IEEE 1800-2009 9.2.2.2.1 (SystemVerilog-1800-2009.txt L11184-11186): "The
// implicit sensitivity list of an always_comb includes the expansions of the
// longest static prefix of each variable or select expression that is read
// within the block". 16.15.6.1 L26881-26884: "Procedural concurrent
// assertions shall also use the sampled values of their arguments, with the
// following exception: a procedural concurrent assertion shall not sample any
// const expression or automatic variable, but shall instead save the value of
// the expression or variable at the time the assertion evaluation attempt is
// added to the procedural assertion queue." The LRM examples of 16.15.6.2 and
// 16.15.6.3 only re-run always_comb blocks for const' operands; the text
// does not say whether sampled operands are read by the procedure. llg
// reading: they are not.
// b1 runs at time 0 and when a changes at 12, not when b changes at 32.
`timescale 1ns/1ns
module tb;
  logic clk = 0;
  logic a = 0, b = 0, x;
  always #5 clk = ~clk;
  always_comb begin : b1
    x = a;
    c1: assert property (@(posedge clk) a == b) $display("%0t c1 pass", $time); else $display("%0t c1 fail", $time);
  end
  initial begin
    #12 a = 1;
    #20 b = 1;
    #10 $finish;
  end
endmodule
