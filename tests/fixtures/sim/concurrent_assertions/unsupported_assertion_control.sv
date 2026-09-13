// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_assertion_control.sv
// IEEE 1800-2009 §20.12: pass/fail action controls are outside the bounded
// H26 API and must fail closed instead of becoming an unrelated VPI call.
module tb;
    logic clk;

    check: assert property (@(posedge clk) 1'b1);

    initial begin
        clk = 1'b0;
        $assertpasson(0, check);
        $finish(0);
    end
endmodule
