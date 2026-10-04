// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/include_escape.sv
// RTL-018 A03 negative (owner policy P-05): the named file exists in the
// repository but lies outside the source directory and every admitted include
// root, so admission must not read it.
`include "../../../../../Cargo.toml"
module tb; initial $finish; endmodule
