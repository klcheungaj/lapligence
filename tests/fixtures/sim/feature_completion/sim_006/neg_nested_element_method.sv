module tb;
  int q[$][$];
  initial begin
    q.push_back({1});
    if (q[0].pop_front() == 1) $display("one");
  end
endmodule
