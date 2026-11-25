// An application acts in a callback that one statement runs.
`timescale 1ns/1ns
module tb;
  class obj; int x; function new(int v); x = v; endfunction endclass
  int v, steps, fv;
  obj o;
  event e;
  int dyn[];
  logic [7:0] q;
  always @(posedge q[0]) steps += 100;
  function automatic int f(int x); fv = x; return x + 1; endfunction
  initial begin
    $arm;
    #2;
    case (1)
      $test$plusargs("stmt=new"): begin o = new(3); steps++; $display("after new"); end
      $test$plusargs("stmt=fork"): begin fork #1 steps++; #2 steps++; join_none steps++; $display("after fork"); end
      $test$plusargs("stmt=error"): begin $error("reported"); steps++; $display("after error"); end
      $test$plusargs("stmt=assert"): begin a1: assert (v == 5); steps++; $display("after assert"); end
      $test$plusargs("stmt=force"): begin force v = 3; steps++; $display("after force"); end
      $test$plusargs("stmt=release"): begin release v; steps++; $display("after release"); end
      $test$plusargs("stmt=func"): begin v = 3; steps = f(4); $display("after func"); end
      $test$plusargs("stmt=dyn"): begin dyn = new[5]; steps++; $display("after dyn"); end
      $test$plusargs("stmt=write"): begin v = 3; steps++; $display("after write"); end
      $test$plusargs("stmt=nba"): begin q <= 8'd1; steps++; $display("after nba"); end
      $test$plusargs("stmt=forkj"): begin fork begin v = 7; end join $display("after forkj"); end
      $test$plusargs("stmt=forkany"): begin fork begin v = 7; end #5; join_any v = 8; $display("after forkany"); end
      $test$plusargs("stmt=event"): begin v = 9; -> e; #0; $display("after event"); end
      $test$plusargs("stmt=mbox"): begin mailbox #(int) m; m = new; v = 9; m.put(3); $display("after mbox"); end
      $test$plusargs("stmt=null"): begin o = null; v = 3; #0; $display("after null"); end
      default: ;
    endcase
    $display("statements go on steps=%0d", steps);
    fork #1; join_none
    wait fork;
    #1 $display("waits go on");
    #3 $finish;
  end
  final $display("[%0t] final steps=%0d", $time, steps);
endmodule
