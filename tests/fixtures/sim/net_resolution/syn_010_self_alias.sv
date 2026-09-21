// llg-test-fixture: tests/fixtures/sim/net_resolution/syn_010_self_alias.sv
// IEEE 1800-2009 10.11: a net cannot alias itself.
module tb;
  wire signal;
  alias signal = signal;
endmodule
