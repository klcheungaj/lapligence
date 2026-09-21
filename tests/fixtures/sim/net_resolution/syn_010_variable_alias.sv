// llg-test-fixture: tests/fixtures/sim/net_resolution/syn_010_variable_alias.sv
// IEEE 1800-2009 10.11: variables are not legal net alias lvalues.
module tb;
  logic variable_signal;
  wire net_signal;
  alias variable_signal = net_signal;
endmodule
