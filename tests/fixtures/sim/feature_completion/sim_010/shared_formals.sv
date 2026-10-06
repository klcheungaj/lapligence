// SIM-010: a by-value formal is an automatic variable of the activation, so
// a detached fork branch shares it with the task body (SV 9.3.2, 13.3),
// including after the task returns; outputs are copied out at return.
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

  task automatic outs(output int r, inout int io);
    fork #1 begin r = 5; io = io + 1; end join_none
    r = 1;
    #2 $display("outs r=%0d io=%0d", r, io);
  endtask

  task automatic sout(output string o);
    o = "a";
    fork #1 o = "b"; join_none
    #2 $display("sout in %s", o);
  endtask

  task automatic outlive(input string s, input real r);
    fork #4 $display("outlive %s %0.1f %0d", s, r, $time); join_none
    s = {s, "!"};
    r = r * 2;
  endtask

  initial begin
    int r, a, b = 3;
    string so;
    t(1, r);
    $display("out %0d", r);
    loop_spawn(5);
    waiter(0);
    outs(a, b);
    $display("a=%0d b=%0d", a, b);
    sout(so);
    $display("sout %s", so);
    outlive("hi", 1.25);
    #5 $finish;
  end
endmodule
