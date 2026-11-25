module tb;
  initial begin : b
    localparam L = 1;
    $display("%0d", L);
  end
  defparam b.L = 2;
endmodule
