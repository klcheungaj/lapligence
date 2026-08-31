// llg-lsp-fixture: root-b/params/top.sv
module ml_ptop;
  localparam int W = 1;
  logic clk;
  logic [7:0] t_q;

  ml_pchild #(
    .W(4),
    .D(W)
  ) u_mp (
    .clk(clk),
    .q(t_q)
  );
endmodule
