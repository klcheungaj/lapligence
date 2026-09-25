// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_stream_target_size.sv
// IEEE 1800-2009 §11.4.14: a stream larger than its fixed-size target is an error.
module tb;
    logic [6:0] pair [0:1];
    logic [6:0] narrow;
    initial narrow = {>>{pair}};
endmodule
