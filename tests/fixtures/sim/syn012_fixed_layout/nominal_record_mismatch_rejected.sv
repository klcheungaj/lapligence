// llg-test-fixture: tests/fixtures/sim/syn012_fixed_layout/nominal_record_mismatch_rejected.sv
// IEEE 1800-2009 §6.22: two distinct unpacked structure types are not
// assignment-compatible merely because their members have the same shape.
module tb;
    typedef struct { logic [7:0] value; } left_t;
    typedef struct { logic [7:0] value; } right_t;
    left_t left;
    right_t right;
    initial left = right;
endmodule
