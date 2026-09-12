// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/sampling_nba.sv
// IEEE 1800-2009 16.5/16.6: predicates use Preponed values even when an
// assertion clock updates a design signal in the same time slot's NBA region.
module tb;
    logic clk;
    logic antecedent;
    logic consequent;

    always #1 clk = ~clk;

    always @(posedge clk) consequent <= 1'b1;

    initial begin
        clk = 1'b0;
        antecedent = 1'b1;
        consequent = 1'b0;
        #3 $finish(0);
    end

    sampled: assert property (@(posedge clk) antecedent |-> !consequent)
        $display("SAMPLED_PREPONED");
endmodule
