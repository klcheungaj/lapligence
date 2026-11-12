// ADV-013 A02: a recursion with local variable formals that never ends and
// starts two instances per level: its live instances double every tick
// until the sequence thread budget stops the run with a report, never a
// hang (16.13.17; LLG_SEQUENCE_THREAD_LIMIT).
`timescale 1ns/1ns
module tb;
  logic clk = 0, s = 0;
  always #5 clk = ~clk;
  property grow(local input int n);
    (n >= 0) and (1'b1 |=> grow(n + 1)) and (1'b1 |=> grow(n + 2));
  endproperty
  g: assert property (@(posedge clk) s |-> grow(0)) else $display("g F %0t", $time);
  initial begin
    #12 s = 1'b1;
    #10 s = 1'b0;
    #200 $display("done");
    $finish;
  end
endmodule
