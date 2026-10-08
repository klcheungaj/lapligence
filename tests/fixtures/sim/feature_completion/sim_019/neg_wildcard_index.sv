// SIM-019 negative: index locators are not defined for a wildcard index (SV 7.12.1).
module tb;
  int w[*];
  int r[$];
  initial begin
    w[1] = 2;
    r = w.find_index with (item > 0);
  end
endmodule
