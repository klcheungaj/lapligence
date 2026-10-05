module tb;
  int w[*];
  initial begin
    w[1] = 2;
    foreach (w[i]) $display("%0d", w[i]);
  end
endmodule
