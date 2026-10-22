// llg policy S14-D1: a negative real repeat count rounds like any real count
// (6.12.2 L5531-5533) and, being less than or equal to 0, waits for nothing
// (9.4.5 L12140-12141), in a repeat loop, a repeated event control and an
// intra-assignment repeat. The portable case
// lrm_decisions/S14-D1_real_repeat_count leaves negative counts out, because
// simulators differ on them.
`timescale 1ns / 1ns
module tb;
  event e;
  real r;
  int a, n;

  initial forever #2 ->e;

  initial begin
    #1 r = -1.5;
    n = 0;
    repeat (r) n++;
    $display("%0t loop -1.5 n=%0d", $time, n);
    repeat (r) @e;
    $display("%0t control -1.5", $time);
    r = -0.4;
    a = repeat (r) @e 5;
    $display("%0t intra -0.4 a=%0d", $time, a);
    r = 0.6;
    repeat (r) @e;
    $display("%0t control 0.6", $time);
    $finish;
  end
endmodule
