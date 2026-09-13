// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_future_global.sv
// IEEE 1800-2009 16.9.4: future global sampled-value functions require a
// deferred property engine; they are rejected rather than reading live state.
module tb;
    logic clk;
    logic value;
    global clocking @(posedge clk); endclocking

    bad: assert property (@(posedge clk) $future_gclk(value));

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #1 $finish(0);
    end
endmodule
