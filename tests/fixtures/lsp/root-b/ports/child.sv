// llg-lsp-fixture: root-b/ports/child.sv
module port_child(
  input logic clk,
  input logic data,
  output logic q
);
  assign q = data;
endmodule
