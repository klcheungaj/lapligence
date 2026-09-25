// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_unpack_size.sv
// IEEE 1800-2009 §11.4.14.3: unpacking needs at least as many source bits as the target stream.
module tb;
    logic [6:0] pair [0:1];
    initial {>>{pair}} = 7'h01;
endmodule
