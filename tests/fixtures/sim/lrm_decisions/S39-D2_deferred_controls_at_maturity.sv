// Decision S39-D2: a deferred assertion's report reads the action controls
// when it matures and executes in the Reactive region, so a $assertfailoff
// executed later in the same time step suppresses an already queued report.
//
// IEEE 1800-2009 20.12 (SystemVerilog-1800-2009.txt L35160): "These system
// tasks shall not affect the execution of pass or fail actions until the
// system task is executed." The report's action executes in the Reactive
// region (16.4.1), after the task; whether the task applies to a report
// queued before it is not stated (llg choice).
`timescale 1ns / 1ns
module tb;
  initial begin
    #1 assert #0 (1'b0) else $display("%0t first fail", $time);
    $assertfailoff;
    #1 $assertfailon;
    assert #0 (1'b0) else $display("%0t second fail", $time);
    #1 $finish;
  end
endmodule
