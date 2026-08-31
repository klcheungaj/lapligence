// llg-lsp-fixture: workspace/top.sv
module top;
  logic clk;
  child #(.WIDTH(8)) u_child(.clk(clk));
endmodule
