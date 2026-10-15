// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/endpoints_sequence.sv
`timescale 1ns / 1ns
// SIM-039: sequence end points read inside sequences (IEEE 1800-2009
// 16.9.11, 16.14.5, 16.14.6). `e1(ready, proc1, proc2)` is clocked by `clk`
// (posedges 5, 15, ..., 95) and matches at 25, 35, 75 and 85 whatever tick
// its match started on. `.triggered` is read on the same clock; `.matched`
// is read on `sysclk` (posedges 7, 21, 35, 49, 63, 77, 91) and holds each
// match until the first `sysclk` tick at or after it (S39-D5: the match at
// 35 is visible at the coincident tick 35). `e3` itself reads
// `e1(...).triggered`. Derivation: readme.md.
module tb;
  logic clk = 1'b0, sysclk = 1'b0;
  logic ready = 1'b0, proc1 = 1'b0, proc2 = 1'b0;
  logic inst = 1'b0, go = 1'b0, done = 1'b0;

  sequence e1(r, p, q);
    @(posedge clk) r ##1 p ##1 q;
  endsequence

  sequence e3;
    @(posedge clk) e1(ready, proc1, proc2).triggered ##1 !proc2;
  endsequence

  always #5 clk = ~clk;
  always #7 sysclk = ~sysclk;

  // Values sampled at the clk posedges 5..95.
  initial begin
    #2  ready = 1'b1;                              // 5, 15
    #10 proc1 = 1'b1;                              // 12: 15, 25
    #10 ready = 1'b0; proc2 = 1'b1;                // 22: 25, 35
    #10 proc1 = 1'b0;                              // 32
    #10 proc2 = 1'b0;                              // 42
    #10 ready = 1'b1;                              // 52: 55, 65
    #10 proc1 = 1'b1;                              // 62: 65, 75
    #10 ready = 1'b0; proc2 = 1'b1;                // 72: 75, 85
    #10 proc1 = 1'b0;                              // 82
    #10 proc2 = 1'b0;                              // 92
  end

  initial begin
    #12 inst = 1'b1;                               // 15
    #10 inst = 1'b0;
    #20 inst = 1'b1;                               // 45
    #10 inst = 1'b0;
    #10 inst = 1'b1;                               // 65
    #10 inst = 1'b0;
  end

  initial begin
    #40 go = 1'b1;                                 // sysclk 49
    #10 go = 1'b0;
    #35 done = 1'b1;                               // sysclk 91
  end

  t1: cover property (@(posedge clk) inst ##1 e1(ready, proc1, proc2).triggered)
    $display("%0t t1", $time);

  pm: cover property (@(posedge sysclk) e1(ready, proc1, proc2).matched)
    $display("%0t pm", $time);

  m2: cover property (@(posedge sysclk)
      go ##1 e1(ready, proc1, proc2).matched [->1] ##1 done)
    $display("%0t m2", $time);

  initial forever @(e3) $display("%0t e3", $time);

  initial #100 $finish;
endmodule
