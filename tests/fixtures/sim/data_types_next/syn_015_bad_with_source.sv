// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_with_source.sv
// IEEE 1800-2009 §11.4.14.4: a with range needs a one-dimensional unpacked source array.
module tb;
    logic [6:0] grid [0:1][0:1];
    logic [27:0] bits;
    initial bits = {>>{grid with [1:1]}};
endmodule
