// llg-lsp-fixture: workspace/child.sv
module child #(parameter int WIDTH = 4)(input logic clk);
  logic
    [WIDTH-1:0]
    payload;
  logic [WIDTH-1:0] memory [0:1];
  leaf #(.WIDTH(WIDTH-2)) u_leaf(.clk(clk));
endmodule
