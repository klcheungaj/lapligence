// llg-test-fixture: tests/fixtures/sim/syn036_capacity/flat_value_below.sv
// IEEE 1800-2009 §7.4.2: two fixed packed cells flatten to 1,048,574 bits.
module tb;
    typedef logic [524286:0] cell_t;
    typedef cell_t array_t [0:1];
    array_t cells;

    function automatic array_t copy(input array_t source);
        copy = source;
    endfunction

    initial begin
        cells[0] = 0;
        cells[1] = 0;
        cells[1][524286] = 1'b1;
        cells = copy(cells);
        if (cells[1][524286] !== 1'b1 || cells[0][0] !== 1'b0) begin
            $display("FAIL flattened fixed value below limit");
            $finish(1);
        end
        $display("PASS syn036 flat below");
        $finish(0);
    end
endmodule
