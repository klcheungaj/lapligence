// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/inferred_sampled_values.sv
// IEEE 1800-2009 16.9.3: a single direct edge control on a procedural block
// supplies the sampled-value clock when no call-site event is written.
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

    always @(posedge clk)
        $display("INFERRED %b %b", $past(value), $rose(value));
endmodule
