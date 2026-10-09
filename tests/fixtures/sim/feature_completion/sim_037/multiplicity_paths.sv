// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/multiplicity_paths.sv
// SIM-037 multiplicity: `(a or a) ##1 ((a or a), x = x + 1)` has four
// matches (two ways per `or`, IEEE 1800-2009 16.9.7). Paths are counted, not
// enumerated, until a match item makes them observable; then every path runs
// the items on its own copy of the locals (16.10 c)), so each of the four calls
// sees x = 2, and the cover counts four matches (16.15.3).
module tb;
  logic clk = 1'b0;
  logic go = 1'b0;
  logic a = 1'b1;
  int t = 0;
  int hits = 0;

  function automatic void note(int value);
    $display("note x=%0d t=%0d", value, t);
  endfunction

  sequence paths;
    int x;
    (go, x = 1) ##0 (a or a) ##1 ((a or a), x = x + 1) ##0 (1'b1, note(x));
  endsequence

  c: cover sequence (@(posedge clk) paths) hits++;

  initial begin
    for (int k = 1; k <= 4; k++) begin
      t = k;
      go = k == 1;
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $display("hits=%0d", hits);
    $finish;
  end
endmodule
