// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/match_native.sv
`timescale 1ns / 1ns
// SIM-039 A01: a DPI-C import attached to a sequence match (IEEE 1800-2009
// 16.11, 35.5). Cycle n is the posedge at 10n+5; a = 1 in cycles 1-3 and
// d = 1, 2, 4 in cycles 1-3. Each match of `(a, x = x + d)[*2]` passes its
// own attempt's local, copied when the match is evaluated, to the C
// companion match_native.c, which runs in the Reactive region and calls the
// export `mn_now` to read the current d. Derivation: readme.md.
module tb;
  logic clk = 1'b0, a = 1'b0;
  logic [7:0] d = 8'd0;

  import "DPI-C" context function void mn_note(input int t, input int x);
  export "DPI-C" function mn_now;

  function int mn_now();
    return int'(d);
  endfunction

  always #5 clk = ~clk;

  sequence twice;
    int x = 0;
    (a, x = x + int'(d)) [*2] ##0 (1'b1, mn_note(int'($time), x));
  endsequence

  c_twice: cover sequence (@(posedge clk) twice);

  initial begin
    @(negedge clk) a = 1'b1; d = 8'd1;     // cycle 1
    @(negedge clk) d = 8'd2;               // cycle 2
    @(negedge clk) d = 8'd4;               // cycle 3
    @(negedge clk) a = 1'b0; d = 8'd8;     // cycle 4
    @(negedge clk) $finish;                // 50
  end
endmodule
