// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_duplicate_bind.sv
// IEEE 1800-2009 §23.11: two binds cannot create the same instance in a target.
module syn033_probe(input logic a);
endmodule

module syn033_target(input logic a);
endmodule

module tb;
  logic a;
  syn033_target dut(.a(a));
endmodule

bind tb.dut syn033_probe repeated(.a(a));
bind tb.dut syn033_probe repeated(.a(a));
