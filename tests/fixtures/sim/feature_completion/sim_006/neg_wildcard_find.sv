module tb;
  int w[*];
  int r[$];
  initial begin
    w[1] = 2;
    r = w.find_index with (item > 1);
  end
endmodule
