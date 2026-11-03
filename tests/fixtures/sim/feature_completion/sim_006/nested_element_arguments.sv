// FIN-001 follow-up: a queue or dynamic array nested in a container element
// passed whole to an input formal of a function or task (SV 13.5.1: the
// value is copied in at the call).
module tb;
  int qq[$][$];
  int dq[][$];
  int qd[$][];
  int aq[int][$];
  int sq[string][$];
  int x;
  function automatic int total(int q[$]);
    int s = 0;
    foreach (q[k]) s += q[k];
    return s;
  endfunction
  function automatic int dsum(int d[]);
    return d.sum();
  endfunction
  task automatic show(input string tag, input int q[$], input int n);
    $display("%s size %0d first %0d n %0d", tag, q.size(), q[0], n);
  endtask
  initial begin
    qq.push_back('{1, 2, 3});
    qq.push_back('{4});
    dq = new[2];
    dq[1] = '{5, 6};
    qd.push_back('{7, 8});
    aq[3] = '{9};
    sq["k"] = '{10, 11};
    x = total(qq[0]) + total(dq[1]);
    $display("sum %0d", x);
    $display("disp %0d %0d", total(qq[1]), dsum(qd[0]));
    show("aq", aq[3], total(sq["k"]));
    x = total(qq[x - 17]);
    $display("idx %0d", x);
    qq[1].push_back(total(qq[0]));
    $display("qq %0d %0d %0d", qq[1].size(), qq[1][1], total(qq[1]));
    $finish;
  end
endmodule
