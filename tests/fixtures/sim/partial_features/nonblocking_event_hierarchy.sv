// llg-test-fixture: tests/fixtures/sim/partial_features/nonblocking_event_hierarchy.sv
`timescale 1ns/1ps
module source;
  event pulse;
  initial begin
    #1 -> pulse;
  end
endmodule

module tb;
  source u0();
  event target;

  initial begin
    ->> @u0.pulse target;
    $display("CHECK: caller=running");
  end

  initial begin
    @target;
    $display("CHECK: hierarchy time=%0t", $time);
  end

  initial begin
    #2 $finish(0);
  end
endmodule
