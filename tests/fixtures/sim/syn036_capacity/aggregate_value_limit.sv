// llg-test-fixture: tests/fixtures/sim/syn036_capacity/aggregate_value_limit.sv
// IEEE 1800-2009 §§7.4.2 and 7.7: a fixed-array formal is a value context;
// SYN-036 keeps its flattened payload bounded by the packed value capacity.
module tb;
    typedef logic [16:0] cell_t;
    typedef cell_t row_t [0:65535];
    row_t cells;

    function automatic row_t identity(input row_t source);
        identity = source;
    endfunction

    initial begin
        cells = identity(cells);
        $finish(0);
    end
endmodule
