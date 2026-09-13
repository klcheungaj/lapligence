// llg-test-fixture: tests/fixtures/sim/partial_features/nonblocking_event_direct_delay.sv
`timescale 1ns / 1ps
module tb;
    event direct_event, delayed_event;
    integer stage;
    initial begin
        stage = 0;
        ->> direct_event;
        stage = 1;
        @direct_event;
        $display("CHECK: direct stage=%0d", stage);
        ->> #2 delayed_event;
        @delayed_event;
        $display("CHECK: delayed time=%0d", $time);
        $finish(0);
    end
endmodule
