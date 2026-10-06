// SIM-009: a level `wait` on a task's `ref` formal re-evaluates on the bound
// actual's changes, whether a specialization binds a module variable
// (directly or through a forwarding task) or the call is expanded for a
// caller automatic or an element actual (SV 9.4.3, 13.5.2, 9.3.2).
module tb;
  int flag;
  int arr[4];

  task automatic level(ref int r, input int v);
    wait (r >= v);
    $display("level %0d at %0d", v, $time);
  endtask

  task automatic forward(ref int r);
    level(r, 3);
  endtask

  task automatic own(int v);
    fork #2 v = 9; join_none
    wait (v == 9);
    $display("own at %0d", $time);
  endtask

  initial begin
    automatic int loc = 0;
    fork begin #1 loc = 1; #1 flag = 2; #1 flag = 3; #1 arr[1] = 4; end join_none
    level(loc, 1);
    level(flag, 2);
    forward(flag);
    level(arr[1], 4);
    own(0);
    $finish;
  end
endmodule
