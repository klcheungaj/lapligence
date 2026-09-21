// llg-test-fixture: tests/fixtures/sim/sequential_predicates/bad_matches_incompatible.sv
// LRM: IEEE 1800-2009 12.6, 12.6.2-12.6.3.
module tb;
    typedef struct packed { logic [7:0] data; logic [3:0] tag; } payload_t;
    payload_t value;
    initial begin
        value = '{data: 8'h5a, tag: 4'h3};
        if (value matches '{data: '{low: 4'h5}, tag: 4'h3})
            $display("incompatible structure member pattern must reject");
        $finish(0);
    end
endmodule
