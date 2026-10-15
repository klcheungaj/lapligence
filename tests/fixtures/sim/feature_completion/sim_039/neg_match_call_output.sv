// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_match_call_output.sv
// SIM-039 negative: match-item calls take no output or inout arguments (IEEE 1800-2009 16.11).
module tb;
  logic clk = 0, a = 0;
  logic [7:0] d = 0, g = 0;
  task automatic get(output logic [7:0] x);
    x = d;
  endtask
  sequence s;
    logic [7:0] v;
    (a, v = d, get(g));
  endsequence
  c1: cover property (@(posedge clk) s);
  initial #20 $finish;
endmodule
