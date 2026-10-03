// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_002/packed_above.v
// IEEE 1364-2001 §§3.3.1, 3.10 / IEEE 1800-2009 §§6.9, 7.4.2 permit
// this element; the backend rejects 1,048,576 packed bits as a resource limit.
module tb;
    reg [1048575:0] cells [0:0];
    initial cells[0] = 0;
endmodule
