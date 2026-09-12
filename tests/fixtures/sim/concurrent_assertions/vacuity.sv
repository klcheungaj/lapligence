// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/vacuity.sv
// IEEE 1800-2009 16.6/16.15: a false implication antecedent succeeds
// vacuously, keeps its explicit pass action, and contributes to accounting.
module tb;
    logic clk;
    logic antecedent;
    logic consequent;

    always #1 clk = ~clk;

    initial begin
        clk = 1'b0;
        antecedent = 1'b0;
        consequent = 1'b0;
        #2 $finish(2);
    end

    vacuous: assert property (@(posedge clk) antecedent |-> consequent)
        $display("VACUOUS_PASS");
endmodule
