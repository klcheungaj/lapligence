module tb;
  initial begin : b
    begin : c localparam K = 30; $display("%0d", K); end
    $display("%0d", K);
  end
endmodule
