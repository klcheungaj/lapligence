// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/macro_error.sv
// IEEE 1800-2009 22.5.1: an error inside a macro body is reported at the
// macro use site, in both editions.
`define READ_MISSING(dst) assign dst = undeclared_in_macro
module tb;
  wire [3:0] value;
  `READ_MISSING(value);
  initial $finish;
endmodule
