`timescale 1ns/1ns
class C;
  int q[1:0][$];
endclass
module tb;
  int q[2][$];
  string sq[3][$];
  int d[2][];
  C c;
  task automatic late(ref int t[$]);
    #1 t.push_back(1);
    $display("%0t in task size=%0d", $time, q[1].size());
    #1 t.push_back(2);
    t[0] = 5;
    #1;
  endtask
  task automatic names(ref string t[$], input string s);
    #1 t.push_back(s);
    #1 t.push_front({s, s});
  endtask
  task automatic grow(ref int t[]);
    #1 t = new[3](t);
    t[2] = 9;
  endtask
  task automatic twice(ref int t[$], input int v);
    #v t.push_back(v);
  endtask
  task automatic local_array();
    int lq[2][$];
    fork
      begin
        wait (lq[0].size() == 1);
        $display("%0t local woke %p", $time, lq[0]);
      end
    join_none
    late(lq[0]);
    $display("%0t local after %p", $time, lq[0]);
  endtask
  initial begin
    c = new;
    fork
      begin
        wait (q[1].size() == 1);
        $display("%0t woke size=1 %p", $time, q[1]);
        wait (q[1].size() == 2);
        $display("%0t woke size=2 %p", $time, q[1]);
        wait (q[1][0] == 5);
        $display("%0t woke q[1][0]=5", $time);
      end
      begin
        wait (sq[2].size() == 2);
        $display("%0t sq size=2 %p", $time, sq[2]);
      end
      begin
        wait (d[0].size() == 3);
        $display("%0t d grew %p", $time, d[0]);
      end
    join_none
    #0;
    fork
      late(q[1]);
      names(sq[2], "x");
      grow(d[0]);
    join
    $display("%0t after %p %p %p", $time, q[1], sq[2], d[0]);
    // two tasks hold the same element at once
    fork
      begin
        wait (q[0].size() == 1);
        $display("%0t first of two %p", $time, q[0]);
        wait (q[0].size() == 2);
        $display("%0t second of two %p", $time, q[0]);
      end
    join_none
    fork
      twice(q[0], 1);
      twice(q[0], 2);
    join
    // a class property through a handle
    fork
      begin
        wait (c.q[1].size() == 1);
        $display("%0t class woke %p", $time, c.q[1]);
      end
    join_none
    twice(c.q[1], 1);
    local_array();
    $finish;
  end
endmodule
