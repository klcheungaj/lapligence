// Decision S36-D8 (negative): an element of a dynamic array is a dynamic
// variable and cannot be the actual of a ref formal of a deferred action.
//
// IEEE 1800-2009 16.4 (SystemVerilog-1800-2009.txt L21226-21227): "It shall
// be an error to pass automatic or dynamic variables as actuals to a ref or
// const ref formal."
// 6.21 (L7007-7008): "members or elements of dynamic variables—class
// properties and dynamically sized variables".
//
// llg reports a compile error, so the expected output is empty. A whole
// static dynamic array passed to a ref formal is not affected.
module tb;
  int d [];
  task automatic report(ref int x);
    $display("x=%0d", x);
  endtask
  initial begin
    d = new[2];
    assert #0 (1'b0) else report(d[0]);
    #1 $finish;
  end
endmodule
