// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/extern_missing.sv
// IEEE 1800-2009 §23.5: an extern module without a body is a frontend error.
extern module syn018_missing (input logic a, output logic y);

module tb;
  logic a;
  logic y;
  syn018_missing missing(.a(a), .y(y));
endmodule
