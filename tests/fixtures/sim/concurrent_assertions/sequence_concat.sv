// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sequence_concat.sv
// IEEE 1800-2009 16.7/16.8: a ranged sequence concatenation retains the
// endpoint one sampled tick after its first element.
module tb;
    logic clk;
    logic first;
    logic second;
    logic result;

    concat: assert property (@(posedge clk) (first ##1 second) |-> result)
        $display("CONCAT_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        result = 1'b0;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b0;
        end
        #1 begin
            second = 1'b1;
            result = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
initial $assertvacuousoff(0);
endmodule
