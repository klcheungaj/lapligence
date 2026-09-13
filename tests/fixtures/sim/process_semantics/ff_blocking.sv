// llg-test-fixture: tests/fixtures/sim/process_semantics/ff_blocking.sv
// IEEE 1800-2009 9.2.2.4 prohibits blocking timing controls,
// not blocking data assignments.
module tb;
    logic clk;
    logic q;

    always_ff @(posedge clk) q = 1'b1;

    initial begin
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 $display("q=%b", q);
        $finish(0);
    end
endmodule
