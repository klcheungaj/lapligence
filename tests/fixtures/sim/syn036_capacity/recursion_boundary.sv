// llg-test-fixture: tests/fixtures/sim/syn036_capacity/recursion_boundary.sv
// IEEE 1364-2001 §10.3.1 / IEEE 1800-2009 §13.4.2 allow automatic recursion.
// The simulator policy admits 256 active calls, then returns an X default.
module tb;
    function automatic integer descend(input integer remaining);
        if (remaining == 0)
            descend = 0;
        else
            descend = 1 + descend(remaining - 1);
    endfunction

    initial begin
        if (descend(254) !== 254 || descend(255) !== 255) begin
            $display("FAIL recursion below/at limit");
            $finish(1);
        end
        $display("PASS syn036 recursion below and at");
        $finish(0);
    end
endmodule
