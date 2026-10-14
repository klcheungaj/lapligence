// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/endpoints_dpi.sv
`timescale 1ns / 1ns
// SIM-039 A03: a sequence end point read through a DPI-C export called from
// an imported context function (IEEE 1800-2009 9.4.4, 16.14.6, 35.5.3). `ab`
// matches at 25 (started at 15). The C companion endpoints_dpi.c reads the
// status twice per call: it is 0 from the Active region at 15 and 25, 1
// after the match in the Observed region at 25 (the process resumed by
// `@(ab)` and the program), and 0 again at 26. Derivation: readme.md.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;

  sequence ab;
    @(posedge clk) a ##1 b;
  endsequence

  import "DPI-C" context function void ep_probe(input int t);
  export "DPI-C" function ep_status;

  function int ep_status();
    return int'(ab.triggered);
  endfunction

  always #5 clk = ~clk;

  initial begin
    #12 a = 1'b1;              // sampled at 15
    #10 b = 1'b1;              // 22, sampled at 25
    #10 a = 1'b0;              // 32
    b = 1'b0;
  end

  initial begin
    @(ab) ep_probe(int'($time));
    #1 ep_probe(int'($time));
  end

  always @(posedge clk)
    if ($time == 15 || $time == 25) ep_probe(int'($time));

  program prg;
    initial forever @(posedge clk)
      if ($time == 25) ep_probe(int'($time));
  endprogram

  initial #50 $finish;
endmodule
