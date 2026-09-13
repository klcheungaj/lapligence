// llg-test-fixture: tests/fixtures/sim/process_semantics/ff_forbidden_fork.sv
// IEEE 1800-2009 Section 9.2.2.4: an always_ff process may not contain a
// fork, even when the fork uses join_none and does not block the parent.
module tb;
    logic clk;
    logic q;

    always_ff @(posedge clk) begin
        fork
            q <= 1'b1;
        join_none
    end

    initial begin
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
