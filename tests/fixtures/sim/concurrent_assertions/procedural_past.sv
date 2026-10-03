// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/procedural_past.sv
// IEEE 1800-2009 16.9.3: $past returns the value sampled in the k-th time step
// strictly before the evaluating one in which the clocking event occurred.
// Procedural code between clock edges counts the latest edge as k = 1; code
// running in an edge's own time step does not count that edge.
module tb;
    logic clk = 1'b0;
    logic [3:0] v = 4'h0;

    always #5 clk = ~clk;
    always @(posedge clk) v <= v + 4'h1;

    initial begin
        #1 $display("A %h", $past(v, 1, , @(posedge clk)));
        #25 $display("B %h %h %h %h", v, $past(v, 1, , @(posedge clk)),
                     $past(v, 2, , @(posedge clk)), $past(v, 3, , @(posedge clk)));
        @(posedge clk);
        $display("C %h %h", v, $past(v, 1, , @(posedge clk)));
        $finish;
    end
endmodule
