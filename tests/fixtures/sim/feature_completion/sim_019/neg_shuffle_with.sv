// SIM-019 negative: shuffle takes no with clause (SV 7.12.2).
module tb;
  int q[$];
  initial begin
    q.push_back(1);
    q.shuffle() with (item);
  end
endmodule
