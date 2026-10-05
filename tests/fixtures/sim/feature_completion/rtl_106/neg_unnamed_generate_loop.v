module tb;
  wire [1:0] w;
  genvar g;
  generate
    for (g = 0; g < 2; g = g + 1) begin
      assign w[g] = 1'b0;
    end
  endgenerate
endmodule
