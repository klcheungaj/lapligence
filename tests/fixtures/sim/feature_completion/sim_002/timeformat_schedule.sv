// SIM-002: $timeformat changes only %t reporting. Events scheduled before,
// between and after format changes keep their simulation times; a delay
// entered after a change still uses the module unit. SV2009 20.4.2,
// 4.4; FND-002 L-F10-11-02.
`timescale 1ns/1ps
module tb;
  reg x = 0;
  initial begin
    x <= #3 1;
    fork
      #5 $display("A time=%0d realtime=%0.3f", $time, $realtime);
    join_none
    $timeformat(-15, 0, "", 0);
    #1 $timeformat(0, 0, " s", 0);
    #1.5 $display("B time=%0d realtime=%0.3f t=%t", $time, $realtime, $realtime);
    $timeformat(-12, 0, " ps", 0);
    #3 $display("C time=%0d t=%t", $time, $realtime);
    $finish(0);
  end
  always @(x) $display("x=%b time=%0d realtime=%0.3f t=%t", x, $time, $realtime, $realtime);
endmodule
