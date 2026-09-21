// llg-test-fixture: tests/fixtures/sim/net_resolution/syn_010_incompatible_alias.sv
// IEEE 1800-2009 10.11: alias members must have a common net type.
module tb;
  wand left;
  wor right;
  alias left = right;
endmodule
