// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/extern_mismatch.sv
// IEEE 1800-2009 §23.5: one port is deliberately missing from the matching
// definition, so the frontend must reject this single signature fault.
extern module syn018_bad #(parameter int W = 4) (
  input logic [W-1:0] a,
  output logic [W-1:0] y
);

module syn018_bad #(parameter int W = 4) (
  input logic [W-1:0] a
);
endmodule

module tb;
  logic [3:0] a;
  syn018_bad bad(.a(a));
endmodule
