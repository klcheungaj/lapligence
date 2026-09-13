// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_assertion_level.sv
// IEEE 1800-2009 §20.11: the bounded assertion-control subset accepts the
// level-0 selector and rejects unimplemented hierarchy-depth policies.
module tb;
    logic clk;

    check: assert property (@(posedge clk) 1'b1);

    initial begin
        clk = 1'b0;
        $assertoff(1);
    end
endmodule
