// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_interface_module.sv
// IEEE 1800-2009 §23.11: an interface target accepts interface/checker
// instantiations, so this module bind is an edition-validity control.
interface syn033_if(input logic a);
  logic bound;
endinterface

module syn033_bad_module(input logic a, output logic observed);
  assign observed = a;
endmodule

module tb;
  logic a;
  syn033_if bus(.a(a));
endmodule

bind syn033_if syn033_bad_module bad(.a(a), .observed(bound));
