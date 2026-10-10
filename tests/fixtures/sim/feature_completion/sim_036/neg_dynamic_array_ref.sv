// SIM-036 A03 negative: an element of a dynamic array is a dynamic variable
// and cannot be passed to a ref formal of a deferred action (IEEE 1800-2009
// 16.4; decision S36-D8).
module tb;
  int d [];
  task automatic report(ref int x);
    $display("x=%0d", x);
  endtask
  initial begin
    d = new[2];
    assert #0 (1'b0) else report(d[0]);
    #1 $finish(0);
  end
endmodule
