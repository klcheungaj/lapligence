// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_structural_bind.sv
// IEEE 1800-2009 §23.11: a module-type bind injects a finite combinational
// observer into every elaborated target instance and preserves its parameter.
module syn033_observer #(parameter bit INVERT = 1'b0) (
  input logic a,
  output logic observed
);
  assign observed = a ^ INVERT;
endmodule

module syn033_target #(parameter bit SEED = 1'b0) (
  input logic a,
  output logic y
);
  logic module_bound;
  logic selected_bound;
  assign y = module_bound ^ SEED;
endmodule

module tb;
  logic a0;
  logic a1;
  logic y0;
  logic y1;

  syn033_target #(.SEED(1'b0)) dut0(.a(a0), .y(y0));
  syn033_target #(.SEED(1'b1)) dut1(.a(a1), .y(y1));

  initial begin
    a0 = 1'b0;
    a1 = 1'b0;
    #1;
    $display("bind=%b/%b selected=%b/%b", y0, y1, dut0.selected_bound, dut1.selected_bound);
    a0 = 1'b1;
    a1 = 1'b1;
    #1;
    $display("bind=%b/%b selected=%b/%b", y0, y1, dut0.selected_bound, dut1.selected_bound);
    $finish;
  end
endmodule

// Module-type binding applies to both independent target instances.
bind syn033_target syn033_observer #(.INVERT(1'b1)) by_type(
  .a(a),
  .observed(module_bound)
);

// Instance binding applies only to dut1 and uses a different parameter.
bind tb.dut1 syn033_observer #(.INVERT(1'b0)) by_instance(
  .a(a),
  .observed(selected_bound)
);
