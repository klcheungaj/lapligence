// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h24_formal_default.sv
// IEEE 1800-2009 16.8/16.10: a typed local input formal may use its
// declaration default when the sequence is invoked without an actual.
module tb;
    logic clk;
    logic first;
    logic second;
    logic [3:0] value;
    logic [3:0] expected;

    sequence captured(local input logic [3:0] source = value);
        logic [3:0] saved;
        (first, saved = source, saved++) ##1
            (second && saved == expected);
    endsequence

    check: assert property (@(posedge clk) captured())
        $display("H24_DEFAULT_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        value = 4'h1;
        expected = 4'h2;
        #1 first = 1'b1;
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            second = 1'b1;
        end
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        #1 $finish(0);
    end
endmodule
