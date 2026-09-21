// llg-test-fixture: tests/fixtures/sim/sequential_predicates/bad_matches_if.sv
// LRM: IEEE 1800-2009 12.6, 12.6.2-12.6.3.
module tb;
    typedef struct packed { logic [7:0] data; } payload_t;
    payload_t value;
    initial begin
        value = '{data: 8'h5a};
        if (value matches '{data: 8'h5a})
            $display("must reject structure pattern");
        $finish(0);
    end
endmodule
