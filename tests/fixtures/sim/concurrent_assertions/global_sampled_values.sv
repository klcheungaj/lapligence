// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/global_sampled_values.sv
// IEEE 1800-2009 16.9.4/20.13: global-clock sampled-value functions use the
// declared global clock domain and never sample a future live value.
module tb;
    logic clk;
    logic [1:0] value;
    global clocking @(posedge clk); endclocking

    initial begin
        clk = 1'b0;
        value = 2'b00;
        #1 begin value = 2'b01; clk = 1'b1; end
        #1 clk = 1'b0;
        #1 begin value = 2'b10; clk = 1'b1; end
        #1 $finish(0);
    end

    always @(posedge clk)
        $display("GLOBAL %b %b", $past_gclk(value), $rose_gclk(value));
endmodule
