// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h24_branch_locals.sv
// IEEE 1800-2009 16.10/16.13: branch-local match-item writes retain distinct
// values when sequence threads merge at a common endpoint.
module tb;
    logic clk;
    logic first;
    logic second;
    logic [3:0] value;
    logic [3:0] expected;

    sequence choose(local input logic [3:0] source);
        logic [3:0] saved;
        ((first, saved = source) ##1 (second && saved == expected))
            or ((first, saved = source + 4'h1) ##1
                (second && saved == expected));
    endsequence

    branch: assert property (@(posedge clk) choose(value))
        $display("H24_BRANCH_PASS");

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
