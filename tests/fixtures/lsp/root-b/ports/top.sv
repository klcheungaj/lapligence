// llg-lsp-fixture: root-b/ports/top.sv
module port_top;
  logic clk;
  logic data;
  logic q;

  port_child u_child (
    .clk
      (clk),
    .data
      (data),
    .q
      (q)
  );
endmodule
