// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/latch_event.sv
// IEEE 1800-2009 §9.2.2.3: always_latch has implicit sensitivity and cannot
// contain an explicit event or blocking timing control.
module tb;
    logic enable;
    logic data;
    logic q;

    always_latch begin
        @(enable);
        if (enable)
            q = data;
    end
endmodule
