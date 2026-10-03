// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_002/packed_at.v
// IEEE 1364-2001 §§3.3.1, 3.10 / IEEE 1800-2009 §§6.9, 7.4.2:
// a one-cell fixed array with a 1,048,575-bit packed element.
module tb;
    reg [1048574:0] cells [0:0];
    initial begin
        cells[0] = 0;
        cells[0][1048574] = 1'b1;
        if (cells[0][1048574] !== 1'b1 || cells[0][0] !== 1'b0) begin
            $display("FAIL packed width at limit");
            $finish(1);
        end
        $display("PASS syn036 packed at");
        $finish(0);
    end
endmodule
