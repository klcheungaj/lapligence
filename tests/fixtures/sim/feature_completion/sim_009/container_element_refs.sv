// SIM-009-A03: waited `ref` formals bound to queue, dynamic-array and
// associative-array elements, container `ref` formals and a fork-shared
// automatic queue follow the selected storage on the typed call path, so the
// tasks recurse (SV 9.4.2, 9.4.3, 13.5.2).
module tb;
  int q[$];
  int d[];
  int aa[int];
  logic lq[$];

  task automatic climb(ref int r, input int n);
    wait (r >= n);
    $display("climb %0d at %0d", n, $time);
    if (n < 2) climb(r, n + 1);
  endtask

  task automatic level(ref int r, input int v, input string tag);
    wait (r == v);
    $display("%s %0d at %0d", tag, v, $time);
  endtask

  task automatic rise(ref logic r, input int n);
    @(posedge r);
    $display("rise %0d at %0d", n, $time);
    if (n > 0) rise(r, n - 1);
  endtask

  task automatic grow(ref int x[$], input int n);
    wait (x.size() > n);
    $display("grow %0d at %0d", x.size(), $time);
    @(x[0]);
    $display("head %0d at %0d", x[0], $time);
  endtask

  task automatic shared_wait();
    int lq2[$];
    lq2.push_back(1);
    fork
      begin
        @(lq2[0]);
        $display("shared %0d at %0d", lq2[0], $time);
      end
      #1 lq2[0] = 7;
    join
  endtask

  initial begin
    q.push_back(0);
    q.push_back(0);
    d = new[2];
    aa[5] = 0;
    lq.push_back(0);
    lq.push_back(0);
    fork
      climb(q[1], 1);
      level(d[1], 3, "dyn");
      level(aa[5], 4, "assoc");
      rise(lq[1], 1);
    join_none
    #1 q[1] = 1;
    #1 q[0] = 9;
    #1 q[1] = 2;
    #1 d[0] = 3;
    #1 d[1] = 3;
    #1 aa[6] = 4;
    #1 aa[5] = 4;
    #1 lq[1] = 1;
    #1 lq[0] = 1;
    #1 lq[1] = 0;
    #1 lq[1] = 1;
    #1;
    fork
      grow(q, 2);
      begin
        #1 q.push_back(5);
        #1 q[0] = 4;
      end
    join
    shared_wait();
    $finish;
  end
endmodule
