// SIM-036 A03 negative: an automatic variable passed to a ref formal of a
// deferred action is an error (IEEE 1800-2009 16.4).
module tb;
  task automatic report(ref int x);
    $display("x=%0d", x);
  endtask
  task automatic t();
    int local_value;
    assert #0 (1'b0) else report(local_value);
  endtask
  initial begin
    t();
    #1 $finish(0);
  end
endmodule
