// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/overlap_order.sv
// IEEE 1800-2009 16.6/16.17: overlapping non-overlapped attempts complete in
// FIFO order on later sampled clock edges.
module tb;
    logic clk;
    logic antecedent;
    logic consequent;

    initial begin
        clk = 1'b0;
        antecedent = 1'b1;
        consequent = 1'b0;
        #1 clk = 1'b1;
        #1 begin
            consequent = 1'b1;
            clk = 1'b0;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end

    overlap: assert property (@(posedge clk) antecedent |=> consequent)
        $display("OVERLAP_PASS");
endmodule
