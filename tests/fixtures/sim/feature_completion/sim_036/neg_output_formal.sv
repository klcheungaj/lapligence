// SIM-036 A03 negative: deferred action arguments are input, ref or
// const ref only (IEEE 1800-2009 16.4); an output formal is rejected.
module tb;
  int x;
  task t(output int a);
    a = 7;
  endtask
  initial begin
    assert #0 (1'b1) t(x);
    #1 $finish(0);
  end
endmodule
