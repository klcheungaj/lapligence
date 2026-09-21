// llg-test-fixture: tests/fixtures/sim/syn018_module_declarations/extern_child.sv
// IEEE 1800-2009 §23.5: the extern module declaration is matched with the
// definition supplied by extern_child_body.sv.
extern module syn018_extern_child #(
  parameter int W = 4
) (
  input logic [W-1:0] a,
  output logic [W-1:0] y
);
