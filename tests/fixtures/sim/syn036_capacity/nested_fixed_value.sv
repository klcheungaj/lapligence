// llg-test-fixture: tests/fixtures/sim/syn036_capacity/nested_fixed_value.sv
// IEEE 1800-2009 §7.4.2: multidimensional fixed values preserve each cell.
module tb;
    typedef logic [6:0] row_t [0:1];
    typedef row_t matrix_t [0:1];
    matrix_t cells;

    function automatic matrix_t copy(input matrix_t source);
        copy = source;
    endfunction

    initial begin
        cells[0][0] = 7'h12;
        cells[0][1] = 7'h23;
        cells[1][0] = 7'h34;
        cells[1][1] = 7'h45;
        cells = copy(cells);
        if (cells[0][0] !== 7'h12 || cells[0][1] !== 7'h23 ||
            cells[1][0] !== 7'h34 || cells[1][1] !== 7'h45) begin
            $display("FAIL nested fixed value");
            $finish(1);
        end
        $display("PASS syn036 nested value");
        $finish(0);
    end
endmodule
