// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_005_inside_unpacked_aggregate_rejected.sv
// IEEE 1800-2009 §11.4.13: an unpacked structure is not an integral inside
// set item and must be rejected as one frontend fault.
module tb;
    typedef struct {
        logic [7:0] first;
        logic [7:0] second;
    } pair_t;

    pair_t aggregate_value;
    logic [7:0] value;
    logic result;

    initial begin
        aggregate_value = '{8'h11, 8'h22};
        value = 8'h22;
        result = value inside {aggregate_value};
    end
endmodule
