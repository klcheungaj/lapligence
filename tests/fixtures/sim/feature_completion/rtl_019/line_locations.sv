// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/line_locations.sv
// IEEE 1800-2009 22.12, 16.14, 20.10: messages from tasks resumed after a
// delay, from an included task body and from a concurrent assertion keep the
// physical location; file-based locations append the `line position.
module child #(parameter int ID = 0);
`include "line_task.svh"
  initial late_error(ID);
endmodule
module tb;
  bit clk;
  child #(.ID(1)) u0();
  child #(.ID(2)) u1();
`line 40 "orig_tb.sv" 0
  task automatic resumed(input int id);
    #4;
    $error("resumed %0d", id);
  endtask
  a_never: assert property (@(posedge clk) clk == 1'b1);
  initial begin
    fork resumed(3); join_none
    #1 clk = 1;
    #5 $finish;
  end
endmodule
