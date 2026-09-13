// llg-test-fixture: tests/fixtures/sim/process_semantics/latch_event.sv
// IEEE 1800-2009 Section 9.2.2.3: always_latch cannot contain an explicit
// event control.
module tb;
    logic enable;
    logic data;
    logic q;

    always_latch begin
        @(enable);
        if (enable)
            q = data;
    end

    initial begin
        enable = 1'b0;
        data = 1'b0;
        #1 $finish(0);
    end
endmodule
