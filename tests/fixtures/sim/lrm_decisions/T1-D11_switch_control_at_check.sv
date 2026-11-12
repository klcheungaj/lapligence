// Decision T1-D11: a switch control sees a change at a check, not at each
// write. The check that sees a change of a `tranif` control schedules the
// switch solve, which reads the control's current value, so a pulse within
// one process turn or one nonblocking batch does not reach the far side.
//
// IEEE 1800-2009 4.9.5 (SystemVerilog-1800-2009.txt L3553-3555): "The
// simulator can process tran at any time. It can process a subset of
// tran-connected events at a particular time, intermingled with the
// execution of other active events." The text leaves open whether a pulse
// of a control within one process is seen; llg follows industry-standard
// behaviour.
`timescale 1ns / 1ns
module tb;
  reg d, en1, en2, en3;
  wire a, b1, b2, b3;
  assign a = d;
  tranif1 t1(a, b1, en1);
  tranif1 t2(a, b2, en2);
  tranif1 t3(a, b3, en3);
  always @(b1) if ($time > 0) $display("%0t b1=%b", $time, b1);
  always @(b2) if ($time > 0) $display("%0t b2=%b", $time, b2);
  always @(b3) if ($time > 0) $display("%0t b3=%b", $time, b3);
  initial begin
    en2 <= 1'b0;
    #2 en2 <= 1'b1;
    en2 <= 1'b0;
  end
  initial begin
    d = 1'b1;
    en1 = 1'b0;
    en3 = 1'b0;
    #1 en1 = 1'b1;
    en1 = 1'b0;
    #2 en3 = 1'b1;
    #1 $display("%0t b1=%b b2=%b b3=%b", $time, b1, b2, b3);
    en3 = 1'b0;
    #1 en3 = 1'b0;
    en3 = 1'b1;
    #1 $display("%0t b1=%b b2=%b b3=%b", $time, b1, b2, b3);
    $finish;
  end
endmodule
