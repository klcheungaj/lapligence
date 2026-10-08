// SIM-019 negative: find requires a with expression (SV 7.12.1).
module tb;
  int q[$];
  int r[$];
  initial begin
    q.push_back(1);
    r = q.find();
  end
endmodule
