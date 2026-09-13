// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/assert_sampled_values.sv
// IEEE 1800-2009 16.9.3/16.15.1: sampled-value calls in a clocked property use
// the property's sampled clock and retain the preponed value for its action.
module tb;
    logic clk;
    logic value;

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #1 value = 1'b1;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 value = 1'b0;
        #1 clk = 1'b1;
        #1 $finish(0);
    end

    sampled: assert property (@(posedge clk) $rose(value) |->
                              ($past(value) === 1'bx))
        $display("ASSERT_SAMPLED PASS %b", $sampled(value));
endmodule
