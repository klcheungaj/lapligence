// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_assertion_argument.sv
// IEEE 1800-2009 §20.11 covers the assertion-control family; the post-2009
// `$assertcontrol` adapter accepts only ON, OFF, and KILL control types and
// rejects other integral arguments.
module tb;
    logic clk;

    check: assert property (@(posedge clk) 1'b1);

    initial begin
        clk = 1'b0;
        $assertcontrol(6);
    end
endmodule
