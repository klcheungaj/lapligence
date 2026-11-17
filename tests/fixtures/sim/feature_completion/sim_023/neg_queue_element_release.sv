// SV 6.21: a queue element is not a release target either.
module tb;
  int q[$];
  initial begin
    q.push_back(1);
    release q[0];
  end
endmodule
