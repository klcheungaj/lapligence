// The other definition of dup_top.
module dup_top (input logic clk);
  logic held_here;
  leaf u_inner (.data(clk), .ready(held_here));
  defparam u_inner.EXTRA = 2;
  wire read_held = u_inner.held;
  if (1) begin : scope
    logic in_scope;
  end : scope
  assign scope.in_scope = u_inner.held[0];
endmodule
