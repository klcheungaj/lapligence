module tb;
  parameter string SM [2][2] = '{'{"a", "b"}, '{"c", "d"}};
  int i, j;
  initial $display("%s", SM[i][j]);
endmodule
