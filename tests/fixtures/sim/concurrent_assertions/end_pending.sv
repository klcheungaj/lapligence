// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/end_pending.sv
// IEEE 1800-2009 16.6/16.17: a non-overlapped attempt still pending when
// simulation ends is accounted for without inventing a consequent result.
module tb;
    logic clk;
    logic antecedent;
    logic consequent;

    initial begin
        clk = 1'b0;
        antecedent = 1'b1;
        consequent = 1'b0;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end

    pending: assert property (@(posedge clk) antecedent |=> consequent)
        $display("PENDING_PASS");
endmodule
