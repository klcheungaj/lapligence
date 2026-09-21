// llg-test-fixture: tests/fixtures/sim/sequential_predicates/bad_matches_conditional.sv
// LRM: IEEE 1800-2009 12.6, 12.6.2-12.6.3.
module tb;
    typedef struct packed { logic [7:0] data; } payload_t;
    payload_t value;
    logic result;
    initial begin
        value = '{data: 8'h5a};
        result = 1'b0 &&& value matches '{data: 8'h5a} ? 1'b1 : 1'b0;
        $display("must reject even an unreachable structure pattern: %b", result);
        $finish(0);
    end
endmodule
