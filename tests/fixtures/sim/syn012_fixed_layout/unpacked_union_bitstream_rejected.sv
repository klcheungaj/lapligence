// llg-test-fixture: tests/fixtures/sim/syn012_fixed_layout/unpacked_union_bitstream_rejected.sv
// IEEE 1800-2009 §6.24.3: an unpacked union with unequal member widths is
// not implicitly a bit-stream type for a packed cast.
module tb;
    typedef union { logic [7:0] narrow; logic [15:0] wide; } union_t;
    union_t value;
    logic [15:0] packed_value;
    initial packed_value = logic [15:0]'(value);
endmodule
