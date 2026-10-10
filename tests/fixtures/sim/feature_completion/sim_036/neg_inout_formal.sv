// SIM-036 A03 negative: an inout formal of a deferred action is rejected
// (IEEE 1800-2009 16.4).
module tb;
  int x;
  task t(inout int a);
    a = 7;
  endtask
  initial begin
    assert #0 (1'b1) t(x);
    #1 $finish(0);
  end
endmodule
