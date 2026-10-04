// SIM-002: $timeformat is design-wide: zero arguments restore the Table 20-3
// defaults (finest precision, 0 digits, empty suffix, width 20), four
// arguments are evaluated when the call executes, the suffix is copied,
// repeated calls replace the whole state, and every module and package
// converts %t from its own unit. SV2009 20.4.2 (Syntax 20-4, Tables 20-2 and
// 20-3), 21.2.1.3; FND-002 L-F10-11-02 (timeformat_zero/four/reset).
`timescale 10us/1us
package pkg;
  function automatic void show();
    $display("pkg [%t] [%0t]", $realtime, $time);
  endfunction
  function automatic void set_ns();
    $timeformat(-9, 2, "ns", 8);
  endfunction
endpackage
`timescale 1us/1ns
module other;
  task automatic show();
    $display("other [%t] [%0t]", $realtime, $time);
  endtask
endmodule
`timescale 1ns/1ps
module tb;
  other o();
  integer units, digits, width;
  string suffix;
  initial begin
    #1.25;
    $display("default [%t] [%0t]", $realtime, $time);
    o.show();
    $timeformat(-9, 2, " ns", 10);
    $display("tb [%t] [%0t] [%t]", $realtime, $time, 7);
    o.show();
    pkg::show();
    units = -12;
    digits = 1;
    suffix = "ps";
    width = 0;
    $timeformat(units, digits, suffix, width);
    suffix = "changed";
    units = -9;
    $display("variables [%t] [%t]", $realtime, 2.5);
    $timeformat(0, 12, " s", 0);
    $display("seconds [%t]", $realtime);
    $timeformat(-15, 0, "fs", 0);
    $display("femto [%t] [%0t]", $realtime, $time);
    $timeformat(-6, 5, " us", 12);
    $display("micro [%t] [%t]", $realtime, -1.5);
    $timeformat(-9, 3, " ns", 8);
    $display("exact width [%t]", $realtime);
    $timeformat(-9, 3, " ns", 7);
    $display("narrow width [%t]", $realtime);
    pkg::set_ns();
    $display("package call [%t]", $realtime);
    $timeformat;
    $display("reset [%t] [%0t]", $realtime, $realtime);
    $timeformat();
    $display("empty list [%0t]", $realtime);
    $finish(0);
  end
endmodule
