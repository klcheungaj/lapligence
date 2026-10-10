// SIM-036 A03 negative: a class property is a dynamic variable and cannot
// be passed to a ref formal of a deferred action (IEEE 1800-2009 16.4).
module tb;
  class C;
    int v;
  endclass
  C h;
  task automatic report(ref int x);
    $display("x=%0d", x);
  endtask
  initial begin
    h = new;
    assert #0 (1'b0) else report(h.v);
    #1 $finish(0);
  end
endmodule
