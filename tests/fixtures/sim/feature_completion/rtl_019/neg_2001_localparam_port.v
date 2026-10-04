// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/neg_2001_localparam_port.v
// IEEE 1364-2001 A.1.3: the parameter port list holds parameter declarations.
module tb #(parameter P = 1, localparam Q = 2) (); initial $finish; endmodule
