// llg-test-fixture: tests/fixtures/sim/syn036_capacity/cell_limit_at.sv
// IEEE 1364-2001 §3.10 / IEEE 1800-2009 §7.4.2: 65,536 fixed cells.
module tb;
    reg cells [0:65535];
    initial begin
        cells[65535] = 1'b1;
        if (cells[65535] !== 1'b1) begin
            $display("FAIL last bounded cell");
            $finish(1);
        end
        $display("PASS syn036 cell limit at");
        $finish(0);
    end
endmodule
