// llg-lsp-fixture: root-a/bindings/params/child.sv
module p_pchild #(
  parameter int W = 8,
  parameter int D = 3
)(
  input logic clk,
  output logic [7:0] q
);
  assign q = '0;
endmodule
