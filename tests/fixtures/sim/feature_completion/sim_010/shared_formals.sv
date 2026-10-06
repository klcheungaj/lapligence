// SIM-010: a by-value input formal is an automatic variable of the
// activation, so a detached fork branch shares it with the task body
// (SV 9.3.2, 13.3), including after the task returns.
module tb;
  task automatic t(input int v, output int o);
    fork
      begin #2 $display("branch v=%0d", v); v = 9; end
    join_none
    v = 7;
    #3 $display("parent v=%0d", v);
    o = v;
  endtask

  task automatic loop_spawn(input int n);
    for (int i = 0; i < 2; i++)
      fork #1 $display("n=%0d", n); join_none
    n = n + 100;
    #2;
  endtask

  task automatic waiter(input logic go);
    fork #1 go = 1; join_none
    @(posedge go);
    $display("go %0d", $time);
  endtask

  task automatic outlive(input string s, input real r);
    fork #4 $display("outlive %s %0.1f %0d", s, r, $time); join_none
    s = {s, "!"};
    r = r * 2;
  endtask

  initial begin
    int r;
    t(1, r);
    $display("out %0d", r);
    loop_spawn(5);
    waiter(0);
    outlive("hi", 1.25);
    #5 $finish;
  end
endmodule
