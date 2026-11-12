// Decision T1-D11: a `disable iff` condition sees a change at a check, not at
// each write. The condition is evaluated like a process sensitive to what it
// reads: a check that sees a change wakes it, and it reads the current values
// when it runs, after the writing process has moved on. So a value undone
// before then disables nothing, whatever checks ran in between.
//
// IEEE 1800-2009 16.13 (SystemVerilog-1800-2009.txt L24455-24456): "If prior
// to the completion of that evaluation the disable condition becomes true,
// then the overall evaluation of the property results in disabled." L24470-
// 24471: "The values of variables used in the disable condition are those in
// the current simulation cycle, i.e., not sampled." The text does not say
// whether a value that holds only between two writes of one process counts;
// llg follows industry-standard behaviour.
//
// Every window of 30 starts one attempt at 5 + 30 * i that fails at
// 25 + 30 * i unless the condition holds in between. Expected output:
// - 25: no change of r: the attempt fails.
// - 55: `r = 1; r = 0;` in one process disables nothing.
// - 85 (no line): `r = 1; #0 r = 0;` holds r over a check and the
//   evaluation reads 1 before the `#0` resumes.
// - 115: `r <= 1; r <= 0;` is one NBA batch: no change.
// - 145: a print between the writes is a check, but the evaluation it wakes
//   runs after the process suspends and reads 0.
// - 175 (no line): r holds for 1 ns.
// - 205: the same with a random call between the writes.
`timescale 1ns / 1ns
module tb;
  bit clk, a, b, r;
  assert property (@(posedge clk) disable iff (r) a |-> ##2 b)
    else $display("%0t fail", $time);
  always #5 clk = ~clk;
  always begin
    #1 a = 1'b1;
    #5 a = 1'b0;
    #24;
  end
  initial begin
    #40 r = 1'b1; r = 1'b0;
    #30 r = 1'b1; #0 r = 1'b0;
    #60 r = 1'b1; $display("%0t print", $time); r = 1'b0;
    #30 r = 1'b1;
    #1 r = 1'b0;
    #29 r = 1'b1; void'($urandom); r = 1'b0;
    #20 $finish;
  end
  initial #100 begin
    r <= 1'b1;
    r <= 1'b0;
  end
endmodule
