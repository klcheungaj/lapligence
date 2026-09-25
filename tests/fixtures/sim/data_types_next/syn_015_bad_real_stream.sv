// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_bad_real_stream.sv
// IEEE 1800-2009 §11.4.14.1: a nonintegral real operand is not streamed.
module tb;
    real value;
    logic [63:0] bits;
    initial bits = {>>{value}};
endmodule
