// SIM-019 negative: sort takes no argument without a with clause (SV 7.12.2).
module tb;
  int q[$];
  initial begin
    q.push_back(1);
    q.sort(1);
  end
endmodule
