// llg-test-fixture: tests/fixtures/sim/syn036_capacity/flat_value_at.sv
// IEEE 1800-2009 §7.4.2: three fixed packed cells flatten to 1,048,575 bits.
module tb;
    typedef logic [349524:0] cell_t;
    typedef cell_t array_t [0:2];
    array_t cells;

    function automatic array_t copy(input array_t source);
        copy = source;
    endfunction

    initial begin
        cells[0] = 0;
        cells[1] = 0;
        cells[2] = 0;
        cells[2][349524] = 1'b1;
        cells = copy(cells);
        if (cells[2][349524] !== 1'b1 || cells[0][0] !== 1'b0) begin
            $display("FAIL flattened fixed value at limit");
            $finish(1);
        end
        $display("PASS syn036 flat at");
        $finish(0);
    end
endmodule
