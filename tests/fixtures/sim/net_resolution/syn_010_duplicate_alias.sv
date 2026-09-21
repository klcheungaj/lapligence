// llg-test-fixture: tests/fixtures/sim/net_resolution/syn_010_duplicate_alias.sv
// IEEE 1800-2009 10.11: the same net bits cannot be aliased twice.
module tb;
  wire left;
  wire right;
  alias left = right;
  alias left = right;
endmodule
