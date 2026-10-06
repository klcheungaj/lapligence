// SIM-016: a task's class-handle locals and by-value handle formals are
// shared with its fork branches (SV 9.3.2, 13.5.1): a later assignment by
// the task is seen by a running branch.
module tb;
  class pkt;
    int id;
    function new(int i); id = i; endfunction
  endclass

  task automatic local_handle();
    pkt p = new(1);
    fork #2 $display("branch id=%0d", p.id); join_none
    p = new(2);
    #3;
  endtask

  task automatic formal_handle(input pkt h);
    fork #1 $display("formal id=%0d", h.id); join_none
    h = new(7);
    #2;
  endtask

  task automatic joined_handle(input pkt h);
    fork #1 $display("join id=%0d", h.id); join
  endtask

  initial begin
    pkt x = new(5);
    local_handle();
    formal_handle(x);
    joined_handle(x);
    $display("caller id=%0d", x.id);
    $finish;
  end
endmodule
