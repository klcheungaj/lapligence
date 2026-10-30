// A fork started by a permanent process inherits its status: the child
// blocked on a never-triggered event does not make the run a deadlock.
module tb;
  event ev;
  logic clk = 1'b0;
  always @(posedge clk) fork
    @(ev);
  join_none
  initial #1 $display("initial done");
  final $display("final");
endmodule
