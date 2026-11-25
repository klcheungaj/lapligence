module tb;
  for (genvar i = 0; i < 1; i++) begin : g
    parameter P = 1;
    initial $display("%0d", P);
  end
  defparam g[0].P = 2;
endmodule
