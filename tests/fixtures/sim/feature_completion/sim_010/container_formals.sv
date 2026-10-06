// SIM-010: a container formal that a fork branch names is shared with the
// branch (SV 9.3.2, 13.3); an output container is copied out at return.
module tb;
  task automatic t(input int q[$]);
    fork #1 $display("join %0d", q.size()); join
  endtask
  task automatic d(input int q[$], output int o[$]);
    fork begin #1 $display("branch %0d", q.size()); q.push_back(8); o.push_back(3); end join_none
    q.push_back(9);
    #2 $display("parent %0d %0d", q.size(), q[q.size()-1]);
  endtask
  initial begin
    int r[$];
    t('{1, 2});
    d('{1}, r);
    $display("out %0d %0d", r.size(), r[0]);
    $finish;
  end
endmodule
