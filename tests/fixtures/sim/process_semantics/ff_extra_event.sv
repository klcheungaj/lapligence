// llg-test-fixture: tests/fixtures/sim/process_semantics/ff_extra_event.sv
// IEEE 1800-2009 Section 9.2.2.4: always_ff has exactly one event control.
module tb;
    logic clk;
    logic enable;
    logic q;

    always_ff @(posedge clk) begin
        @(posedge enable);
        q <= 1'b1;
    end

    initial begin
        clk = 1'b0;
        enable = 1'b0;
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
