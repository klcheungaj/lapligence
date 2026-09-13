// llg-test-fixture: tests/fixtures/sim/process_semantics/delayed_nba_ff.sv
// IEEE 1800-2009 Section 9.2.2.4: an always_ff process permits a delayed
// nonblocking data assignment while still rejecting blocking timing controls.
module tb;
    logic clk;
    logic d;
    logic q;

    always_ff @(posedge clk) q <= #1 d;

    initial begin
        clk = 1'b0;
        d = 1'b1;
        #1 clk = 1'b1;
        #0 $display("edge q=%b", q);
        #2 $display("delayed q=%b", q);
        $finish(0);
    end
endmodule
