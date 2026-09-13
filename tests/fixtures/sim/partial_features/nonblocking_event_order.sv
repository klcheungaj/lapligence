// llg-test-fixture: tests/fixtures/sim/partial_features/nonblocking_event_order.sv
// Covers same-slot NBA ordering, issue-order retention, and mixed sources.
`timescale 1ns/1ps
module tb;
  event source;
  event direct;
  event first;
  event second;
  event mixed_source;
  event mixed_target;
  event qualified_target;
  reg trigger_signal;
  reg qualifier;
  integer marker;

  initial begin
    @direct;
    $display("CHECK: direct marker=%0d", marker);
  end

  initial begin
    @first;
    $display("CHECK: first marker=%0d", marker);
  end

  initial begin
    @second;
    $display("CHECK: second marker=%0d", marker);
  end

  initial begin
    @mixed_target;
    $display("CHECK: mixed once=%0d", marker);
  end

  initial begin
    @qualified_target;
    $display("CHECK: qualified once=%0d", marker);
  end

  initial begin
    marker = 0;
    trigger_signal = 0;
    qualifier = 0;
    ->> direct;
    ->> @source first;
    ->> @source second;
    ->> @(trigger_signal or mixed_source) mixed_target;
    ->> @(source iff qualifier) qualified_target;
    marker = 1;
    #0 -> source;
    trigger_signal = 1;
    -> mixed_source;
    qualifier = 1;
    -> source;
    marker = 2;
    #1 $finish(0);
  end
endmodule
