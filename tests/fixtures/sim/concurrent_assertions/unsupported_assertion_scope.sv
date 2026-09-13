// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_assertion_scope.sv
// IEEE 1800-2009 §20.11: assertion-control scope arguments must resolve to
// an assertion or hierarchy; a value expression is rejected fail-closed.
module tb;
    logic clk;

    check: assert property (@(posedge clk) 1'b1);

    initial begin
        clk = 1'b0;
        $assertoff(0, 1'b0);
    end
endmodule
