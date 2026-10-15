// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_match_call_timing.sv
// SIM-039 negative: a match-item task runs in the Reactive region without waiting (IEEE 1800-2009 16.11); a task with timing controls is rejected.
module tb;
  logic clk = 0, a = 0;
  logic [7:0] d = 0;
  task automatic slow(input logic [7:0] x);
    #1 $display("%0d", x);
  endtask
  sequence s;
    logic [7:0] v;
    (a, v = d, slow(v));
  endsequence
  c1: cover property (@(posedge clk) s);
  initial #20 $finish;
endmodule
