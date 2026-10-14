// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/endpoints_procedural.sv
`timescale 1ns / 1ns
// SIM-039 A03: sequence end points observed from procedural code (IEEE
// 1800-2009 9.4.2.4, 9.4.4, 16.14.6). `abc` matches at 35 (started at 15)
// and at 45 (started at 25); `de` matches at the negedge at 50. The
// triggered status is set in the Observed region and lasts for the rest of
// the time step: an Active-region read at the matching posedge sees 0, a
// program (Reactive) read and every process resumed by the match see 1, and
// one time unit later it is 0 again. Derivation: readme.md.
module tb;
  logic clk = 1'b0;
  logic a = 1'b0, b = 1'b0, c = 1'b0, d = 1'b0, e = 1'b0;

  sequence abc;
    @(posedge clk) a ##1 b ##1 c;
  endsequence

  sequence de;
    @(negedge clk) d ##[2:5] e;
  endsequence

  always #5 clk = ~clk;

  initial begin
    #12 a = 1'b1;
    #10 b = 1'b1;              // 22
    #10 a = 1'b0; c = 1'b1;    // 32
    #10 b = 1'b0;              // 42
    #10 c = 1'b0;              // 52
  end

  initial begin
    #17 d = 1'b1;              // sampled at the negedge at 20
    #5 d = 1'b0;               // 22
    #25 e = 1'b1;              // 47, sampled at 50
    #5 e = 1'b0;               // 52
  end

  initial begin
    wait (abc.triggered || de.triggered);
    $display("%0t P1 woke abc=%0d de=%0d", $time, abc.triggered, de.triggered);
    #0 $display("%0t P1 inactive abc=%0d", $time, abc.triggered);
    #1 $display("%0t P1 later abc=%0d", $time, abc.triggered);
  end

  initial forever begin
    @(abc) $display("%0t P2 @abc", $time);
    wait (abc.triggered) $display("%0t P2 wait abc", $time);
  end

  always @(posedge clk)
    if ($time == 35 || $time == 45)
      $display("%0t P3 active abc=%0d", $time, abc.triggered);

  initial begin
    @(de) $display("%0t P5 @de", $time);
  end

  program prg;
    initial forever @(posedge clk)
      if ($time == 35 || $time == 45)
        $display("%0t P4 reactive abc=%0d", $time, abc.triggered);
  endprogram

  initial #70 $finish;
endmodule
