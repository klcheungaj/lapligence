module tb;
  reg [3:0] a, b;
  initial begin
    b = 4'd1;
    if ((a = b) != 0) $display("x");
  end
endmodule
