// llg-test-fixture: tests/fixtures/sim/syn033_structural_bind/syn_033_generated_bind.sv
// IEEE 1800-2009 §23.11: instance binds select elaborated generate paths.
module syn033_generated_observer #(parameter bit INVERT = 1'b0) (
  input logic a,
  output logic observed
);
  assign observed = a ^ INVERT;
endmodule

module syn033_generated_target #(parameter bit SEED = 1'b0) (
  input logic a,
  output logic y
);
  logic bound;
  assign y = bound ^ SEED;
endmodule

module tb;
  logic a;
  logic [2:0] y;

  for (genvar i = 0; i < 2; i++) begin : rows
    syn033_generated_target #(.SEED(i == 1)) dut(.a(a), .y(y[i]));
  end
  if (1) begin : chosen
    syn033_generated_target #(.SEED(1'b1)) dut(.a(a), .y(y[2]));
  end

  initial begin
    a = 1'b0;
    #1;
    $display("generated=%b", y);
    a = 1'b1;
    #1;
    $display("generated=%b", y);
    $finish;
  end
endmodule

bind tb.rows[0].dut syn033_generated_observer #(.INVERT(1'b0)) bound_probe(
  .a(a), .observed(bound)
);
bind tb.rows[1].dut syn033_generated_observer #(.INVERT(1'b1)) bound_probe(
  .a(a), .observed(bound)
);
bind tb.chosen.dut syn033_generated_observer #(.INVERT(1'b0)) bound_probe(
  .a(a), .observed(bound)
);
