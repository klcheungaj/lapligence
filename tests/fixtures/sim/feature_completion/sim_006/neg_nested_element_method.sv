module tb;
  int q[$][$];
  initial begin
    q.push_back({1});
    q[0].push_back(2);
  end
endmodule
