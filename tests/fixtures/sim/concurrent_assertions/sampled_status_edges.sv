// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sampled_status_edges.sv
// IEEE 1800-2009 16.9.3: $rose/$fell use only the sampled expression LSB;
// unknown high bits and X/Z transitions must not be treated as whole-vector
// edge predicates.
module tb;
    logic clk;
    logic [3:0] value;

    initial begin
        clk = 1'b0;
        value = 4'b0000;
        #1 clk = 1'b1;
        #1 begin value = 4'bx001; clk = 1'b0; end
        #1 clk = 1'b1;
        #1 begin value = 4'bz000; clk = 1'b0; end
        #1 clk = 1'b1;
        #1 begin value = 4'b000x; clk = 1'b0; end
        #1 clk = 1'b1;
        #1 begin value = 4'b0001; clk = 1'b0; end
        #1 clk = 1'b1;
        #1 $finish(0);
    end

    always @(posedge clk)
        $display("EDGE %b %b %b %b", $rose(value, @(posedge clk)),
                 $fell(value, @(posedge clk)),
                 $stable(value, @(posedge clk)),
                 $changed(value, @(posedge clk)));
endmodule
