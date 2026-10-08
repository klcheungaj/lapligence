// SIM-019 negative: index results of a string-keyed array are a string queue (SV 7.12.1).
module tb;
  int sa[string];
  int r[$];
  initial begin
    sa["k"] = 1;
    r = sa.find_index with (item > 0);
  end
endmodule
