// llg-test-fixture: tests/fixtures/sim/concurrent_assertions/h24_local_match.sv
// IEEE 1800-2009 16.10/16.13: a sequence local is assigned by a match item
// and consumed by a later sampled element.
module tb;
    logic clk;
    logic first;
    logic second;
    logic [3:0] value;
    logic [3:0] expected;

    function automatic void record(input logic [3:0] observed);
        if (observed == 4'h2)
            $display("H24_CALL_PASS");
    endfunction

    sequence captured(local input logic [3:0] source);
        logic [3:0] saved;
        (first, saved = source, saved++, record(saved)) ##1
            (second && saved == expected);
    endsequence

    check: cover property (@(posedge clk) captured(value))
        $display("H24_LOCAL_PASS");

    initial begin
        clk = 1'b0;
        first = 1'b0;
        second = 1'b0;
        value = 4'h1;
        expected = 4'h2;
        #1 begin first = 1'b1; second = 1'b0; end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b1;
            second = 1'b1;
            value = 4'h2;
            expected = 4'h2;
        end
        #1 clk = 1'b1;
        #1 begin
            clk = 1'b0;
            first = 1'b0;
            second = 1'b1;
            expected = 4'h3;
        end
        #1 clk = 1'b1;
        #1 $finish(0);
    end
endmodule
