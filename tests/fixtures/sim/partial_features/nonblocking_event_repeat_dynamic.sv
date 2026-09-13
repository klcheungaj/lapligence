// llg-test-fixture: tests/fixtures/sim/partial_features/nonblocking_event_repeat_dynamic.sv
`timescale 1ns/1ps
module tb;
  event source;
  event target;
  integer count;

  initial begin
    count = 2;
    ->> repeat (count) @source target;
  end

  initial begin
    @target;
    $display("CHECK: repeated count time=%0t", $time);
  end

  initial begin
    #1 -> source;
    #1 -> source;
    #3 $finish(0);
  end
endmodule
