// llg-test-fixture: tests/fixtures/sim/partial_features/timeformat_mixed_scopes.sv
// IEEE 1800-2009 §20.4.2; Verilog-2001 §17.3.2.
`timescale 1us/1ns
module timeformat_child;
  initial begin
    #1.234;
    $display("child=[%t] childreal=[%t]", $time, $realtime);
  end
endmodule

`timescale 1ns/1ps
module tb;
  timeformat_child child();
  initial begin
    $timeformat(-9, 3, " ns", 0);
    #1.234;
    $display("parent=[%t] parentreal=[%t]", $time, $realtime);
    #2us;
    $finish(0);
  end
endmodule
