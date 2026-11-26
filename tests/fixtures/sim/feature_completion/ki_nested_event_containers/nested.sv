// Queues, dynamic and associative arrays of event queues, and fixed arrays
// of event queues (6.17, 7.5, 7.8, 7.10): in a module, a class property and
// an automatic subroutine variable. Each element is an event handle that
// waits, triggers, compares and is replaced like any event variable.
`timescale 1ns/1ns
class C;
  event cq[$][$];
endclass

module tb;
  event e1, e2, e3, e4, e5, e6, e7, e8;
  event w[$][$];
  event d[2][$];
  event am[int][$];
  event dd[][$];

  task automatic waiter(int k);
    event lw[$][$];
    lw.push_back({});
    lw[0].push_back(e7);
    @(lw[0][0]);
    $display("lw %0d at %0t", k, $time);
  endtask

  initial begin
    C o = new;
    w.push_back({e1});
    w.push_back({e2, e3});
    w[0].push_back(e4);
    d[1].push_back(e5);
    d[0] = {e1, e6};
    am[5] = {e2};
    am[5].push_back(e8);
    dd = new[2];
    dd[1] = {e3};
    o.cq.push_back({e6, e2});
    fork
      begin @(w[1][0]); $display("w10 at %0t", $time); end
      begin @(w[0][1]); $display("w01 at %0t", $time); end
      begin @(d[1][0]); $display("d10 at %0t", $time); end
      begin @(d[0][1]); $display("d01 at %0t", $time); end
      begin @(am[5][1]); $display("am51 at %0t", $time); end
      begin @(dd[1][0]); $display("dd10 at %0t", $time); end
      begin @(o.cq[0][0]); $display("cq00 at %0t", $time); end
      begin wait (w[0][0].triggered); $display("w00 triggered at %0t", $time); end
      waiter(1);
    join_none
    #1 -> e2;
    #1 -> e4;
    #1 -> e5;
    #1 -> e6;
    #1 -> e8;
    #1 -> dd[1][0];
    #1 -> e7;
    #1 -> w[0][0];
    #1;
    $display("%0d %0d %0d %0d %0d %0d", w.size(), w[1].size(), d[0].size(), am[5].size(),
             dd[1].size(), o.cq[0].size());
    $display("%0d %0d %0d", w[1][1] == e3, w[0][0] == e2, d[0][1] == e6);
    w[1][0] = null;
    $display("%0d", w[1][0] == null);
    void'(w[0].pop_front());
    $display("%0d %0d", w[0].size(), w[0][0] == e4);
    foreach (w[i, j]) $display("w[%0d][%0d]", i, j);
    w.delete(0);
    $display("%0d", w.size());
    $finish;
  end
endmodule
