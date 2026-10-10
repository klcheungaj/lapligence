// SIM-036 A03 negative: `assert final` is not part of IEEE 1800-2009, which
// defines only the #0 deferred form (16.4, Syntax 16-2).
module tb;
  initial begin
    assert final (1'b1);
    $finish(0);
  end
endmodule
