// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/extra_event.sv
// IEEE 1800-2009 §9.2.2.4: an always_ff process has exactly one event
// control, including controls nested in its body.
module tb;
    logic clk;
    logic enable;
    logic q;

    always_ff @(posedge clk) begin
        @(posedge enable);
        q <= 1'b1;
    end
endmodule
