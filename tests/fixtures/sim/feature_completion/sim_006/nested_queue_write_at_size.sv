module tb;
  int qq[$][$];
  int q[$];
  initial begin
    qq.push_back({});
    qq[0].push_back(1);
    qq[0][1] = 5;
    $display("at size: %p size %0d", qq[0], qq[0].size());
    qq[0][3] = 9;
    $display("past end: %p size %0d", qq[0], qq[0].size());
    q.push_back(1);
    q[1] = 5;
    q[3] = 9;
    $display("plain: %p", q);
    $display("done");
    $finish;
  end
endmodule
