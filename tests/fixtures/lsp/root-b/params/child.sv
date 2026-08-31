// llg-lsp-fixture: root-b/params/child.sv
module ml_pchild #(
  parameter int W = 8,
  parameter int D = 3
)(
  input logic clk,
  output logic [7:0] q
);
  assign q = {7'b0, clk};
endmodule
