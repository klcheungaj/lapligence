// SIM-002: IEEE 1364-2001 17.7.1-17.7.3: $time and $stime round to the unit
// of the module containing the call (halves upward), $realtime keeps the
// fraction, and %t uses the containing module's unit; a task called through
// a hierarchical name keeps its own module's unit. The default %t units are
// the finest precision (1ps).
`timescale 1ns/1ps
module child;
  task report;
    $display("child time=%0d stime=%0d realtime=%0.3f t=%0t", $time, $stime, $realtime,
             $time);
  endtask
endmodule
`timescale 10ns/1ns
module tb;
  child c();
  initial begin
    #2.25;
    $display("tb time=%0d stime=%0d realtime=%0.3f t=%0t", $time, $stime, $realtime, $time);
    c.report;
    #0.2;
    $display("tb time=%0d realtime=%0.3f t=%t", $time, $realtime, $realtime);
    tb.c.report;
    $printtimescale(c);
    $finish(0);
  end
endmodule
