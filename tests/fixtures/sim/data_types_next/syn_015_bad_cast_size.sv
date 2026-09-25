// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_cast_size.sv
// IEEE 1800-2009 §6.24.3: an explicit bit-stream cast requires equal source and target sizes.
module tb;
    typedef logic [6:0] lane_t;
    typedef lane_t trio_t [0:2];
    typedef logic [13:0] flat_t;
    trio_t trio;
    flat_t flat;
    initial flat = flat_t'(trio);
endmodule
