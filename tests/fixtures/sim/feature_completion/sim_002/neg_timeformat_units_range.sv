// SIM-002: units_number must be in 0 through -15 (SV2009 20.4.2, Table
// 20-2). The value is checked when the call executes: the earlier format
// stays in effect, and the run fails.
`timescale 1ns/1ps
module tb;
  integer units = -16;
  initial begin
    $timeformat(-9, 0, " ns", 0);
    #1 $display("before [%t]", $realtime);
    $timeformat(units, 0, "", 0);
    $display("after [%t]", $realtime);
  end
endmodule
