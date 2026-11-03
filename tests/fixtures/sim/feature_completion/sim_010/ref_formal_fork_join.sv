// FIN-001 G3 (F09-04): branches of a `fork ... join` inside a task read,
// write and wait on the task's packed, real, string and event `ref` formals
// through the caller's actuals (SV 9.3.2 restricts only join_any/join_none;
// SV 13.5.2): a write in one branch wakes a wait in another branch and in
// other processes, and the caller sees every change after the join.
`timescale 1ns / 1ns
module tb;
  int a = 1;
  int arr[4] = '{10, 20, 30, 40};
  int q[$] = '{5, 6};
  real x = 1.5;
  string s = "ab";
  event ev;
  logic [7:0] w = 8'h0f;

  task automatic bump(ref int r, input int by);
    fork
      #1 r = r + by;
      begin
        @(r);
        $display("%0t saw %0d", $time, r);
      end
      #2 $display("%0t branch reads %0d", $time, r);
    join
    $display("%0t after join %0d", $time, r);
  endtask

  task automatic mixed(ref real rr, ref string rs, ref logic [7:0] rw);
    fork
      #1 rr = rr * 2;
      #1 rs = {rs, "c"};
      #1 rw[7:4] = 4'ha;
      begin
        #2;
        fork
          #1 rw[0] = 1'b0;
          #1 rs = {rs, "!"};
        join
      end
    join
    $display("%0t mixed %0.2f %s %h", $time, rr, rs, rw);
  endtask

  task automatic signal(ref event re);
    fork
      #1 ->re;
      begin
        @re;
        $display("%0t event seen in branch", $time);
      end
    join
  endtask

  // Automatic actuals reach the task only through their references.
  task automatic locals();
    int li = 7;
    real lr = 0.25;
    string ls = "x";
    logic [7:0] lw = 8'h00;
    event le;
    bump(li, 1);
    mixed(lr, ls, lw);
    signal(le);
    $display("%0t locals %0d %0.2f %s %h", $time, li, lr, ls, lw);
  endtask

  // Other processes record when the branches' writes and trigger wake them.
  time a_woke, ev_woke;
  int a_seen;
  initial begin
    @(a);
    a_woke = $time;
    a_seen = a;
  end
  initial begin
    @ev ev_woke = $time;
  end

  initial begin
    bump(a, 10);
    $display("%0t a=%0d", $time, a);
    bump(arr[2], 3);
    $display("%0t arr=%0d %0d", $time, arr[2], arr[3]);
    bump(q[1], 4);
    $display("%0t q=%0d %0d", $time, q[0], q[1]);
    mixed(x, s, w);
    $display("%0t x=%0.2f s=%s w=%h", $time, x, s, w);
    signal(ev);
    locals();
    $display("%0t done; a woke %0t with %0d, ev woke %0t", $time, a_woke, a_seen, ev_woke);
    $finish;
  end
endmodule
