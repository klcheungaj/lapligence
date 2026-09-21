// llg-test-fixture: tests/fixtures/sim/syn012_fixed_layout/packed_union_width_rejected.sv
// IEEE 1800-2009 §7.3.1: an untagged packed union requires equal-width
// members; this is a single declaration fault.
module tb;
    typedef union packed {
        logic [7:0] narrow;
        logic [15:0] wide;
    } illegal_t;
    illegal_t value;
    initial value.narrow = 8'h5a;
endmodule
