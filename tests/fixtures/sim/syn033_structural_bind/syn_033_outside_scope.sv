// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_outside_scope.sv
// IEEE 1800-2009 §23.11: bound port actuals resolve in the target scope.
`default_nettype none
module syn033_probe(input logic a);
endmodule

module syn033_target;
endmodule

module tb;
  logic only_in_tb;
  syn033_target dut();
endmodule

bind tb.dut syn033_probe wrong_scope(.a(only_in_tb));
