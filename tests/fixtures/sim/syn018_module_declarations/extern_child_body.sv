// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/extern_child_body.sv
// IEEE 1800-2009 §23.5: this definition supplies the body for the matching
// extern declaration in extern_child.sv.
module syn018_extern_child #(
  parameter int W = 4
) (
  input logic [W-1:0] a,
  output logic [W-1:0] y
);
  assign y = a + W;
endmodule
