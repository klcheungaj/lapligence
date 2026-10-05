module tb;
  reg [1:0] sel;
  reg y;
`line 30 "orig_lint.v" 0
  always @* begin
    case (sel)
      2'd0: y = 1'b0;
      2'd1: y = 1'b1;
    endcase
  end
  initial begin
    sel = 2'd1;
    #1 $display("y=%b", y);
  end
endmodule
