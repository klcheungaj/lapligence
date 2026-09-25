// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_associative_stream.sv
// IEEE 1800-2009 §§6.24.3, 11.4.14.3: an associative array is not a fixed bit-stream target.
module tb;
    logic [7:0] table_value [int];
    logic [15:0] bits;
    initial {>>{table_value}} = bits;
endmodule
