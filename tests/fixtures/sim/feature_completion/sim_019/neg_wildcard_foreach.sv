// SIM-019 negative: foreach cannot iterate a wildcard index (SV 7.8.1).
module tb;
  int w[*];
  int t;
  initial begin
    w[1] = 2;
    foreach (w[k]) t += w[k];
  end
endmodule
