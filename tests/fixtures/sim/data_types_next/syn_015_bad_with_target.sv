// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_with_target.sv
// IEEE 1800-2009 §11.4.14.4: a with range needs a one-dimensional unpacked target array.
module tb;
    logic [6:0] grid [0:1][0:1];
    initial {>>{grid with [0:0]}} = 14'h3fff;
endmodule
