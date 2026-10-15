// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_property_local.sv
// SIM-039 negative: a property local initialized at the attempt start cannot be read by a later property operand (unsupported form, IEEE 1800-2009 16.10).
module tb;
  logic clk = 0, a = 0;
  logic [7:0] d = 0;
  property p;
    logic [7:0] v = d;
    always (a |-> d == v);
  endproperty
  c1: assert property (@(posedge clk) p);
  initial #20 $finish;
endmodule
