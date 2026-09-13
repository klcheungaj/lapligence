// llg-test-fixture: tests/fixtures/sim/partial_features/timeformat_q05.sv
// IEEE 1800-2009 §20.4.2; Verilog-2001 §17.3.2.
`timescale 1ns/1ps
module tb;
  integer units, precision, width;
  string suffix;
  logic [127:0] wide;
  reg changed;
  initial begin
    #1.234;
    $display("default=[%t]", $time);
    units = -6;
    precision = 2;
    width = 0;
    suffix = " us";
    $timeformat(units, precision, suffix, width);
    $display("dynamic=[%t]", $time);
    $timeformat(-9, 2, " ns", 8);
    $display("changed=[%t] now=[%t] real=[%t]", $time, $time, $realtime);
    $display("integer=[%t]", 1234);
    $display("signed=[%t]", -1234);
    $display("rounded=[%t %t %t]", 1, 2, 3);
    $timeformat(-6, 2, " us", 0);
    $display("half=[%t %t %t %t %t %t]", 115, 125, 135, -115, -125, -135);
    $display("realhalf=[%t %t]", 115.0, -115.0);
    $timeformat(-9, 2, " ns", 0);
    wide = 128'd123456789012345678901234;
    $display("wide=[%t]", wide);
    $timeformat(-9, 2, " ns", 8);
    $display("explicit=[%10t]", 1);
    $display("small=[%3t]", 1);
    $display("zero=[%0t]", 1);
    $timeformat(-9, 14, " ns", 0);
    $display("high=[%t]", 1);
    $timeformat(-12, 3, " ps", 0);
    $display("ps=[%t]", $time);
    $timeformat(-9, 1, " ns", 5);
    $write("write=[%t]", $realtime);
    $display("");
    $strobe("strobe=[%t]", $time);
    $monitor("monitor=[%t] changed=%0d", $time, changed);
    changed = 1;
    $timeformat();
    $display("reset=[%t]", $time);
    #1;
    $finish(0);
  end
  final begin
    $display("final=[%t]", $time);
  end
endmodule
