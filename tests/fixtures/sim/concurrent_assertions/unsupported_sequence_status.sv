// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/unsupported_sequence_status.sv
// IEEE 1800-2009 §16.11: `.triggered` has event-like persistence semantics
// distinct from the bounded `.matched` endpoint and remains fail-closed until
// that status can be captured without guessing its time-slot lifetime.
module tb;
    logic clk;
    logic value;
    sequence s;
        value;
    endsequence

    bad: assert property (@(posedge clk) s.triggered);

    initial begin
        clk = 1'b0;
        value = 1'b0;
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
