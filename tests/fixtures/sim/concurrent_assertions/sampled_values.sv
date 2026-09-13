// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sampled_values.sv
// IEEE 1800-2009 16.9.3 (sampled-value functions): explicit clock domains keep
// current sampled values separate from live Active-region writes, retain an
// initial history value, and gate history entries at the sampling edge.
module tb;
    logic clk;
    logic enable;
    logic [3:0] value;

    initial begin
        clk = 1'b0;
        enable = 1'b1;
        value = 4'b0000;
        #1 begin value = 4'b0001; clk = 1'b1; end
        #1 begin enable = 1'b0; clk = 1'b0; end
        #1 begin value = 4'b0000; clk = 1'b1; end
        #1 begin enable = 1'b1; clk = 1'b0; end
        #1 begin value = 4'b0010; clk = 1'b1; end
        #1 $finish(0);
    end

    always @(posedge clk) begin
        $display("SAMPLED %b %b %b %b", $sampled(value),
                 $rose(value, @(posedge clk)),
                 $past(value, 1, enable, @(posedge clk)),
                 $stable(value, @(posedge clk)));
    end
endmodule
