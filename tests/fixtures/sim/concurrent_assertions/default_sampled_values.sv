// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/default_sampled_values.sv
// IEEE 1800-2009 16.9/16.12: a sampled-value function without an explicit
// event uses the enclosing default clocking declaration.
module tb;
    logic clk;
    logic value;
    default clocking cb @(posedge clk); endclocking

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

    always @(posedge clk)
        $display("DEFAULT %b %b", $past(value), $rose(value));
endmodule
