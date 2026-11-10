// Decision S07-D17: a negative `new[]` size, or one with X or Z bits, is a
// run-time error that ends the run at that `new[]`, as industry-standard
// behaviour does. The run below stops at `d = new[n]` (case 1) after its
// first two lines, with exit status 1; `+case=2` stops at the copy form
// `new[n](d)` and `+case=3` at the size with X bits, each after the same two
// lines. `new[0]` is legal and empties the array.
//
// IEEE 1800-2009 7.5.1 (SystemVerilog-1800-2009.txt L8293-8294): "The
// desired size of the dynamic array. The type of this operand is longint. It
// shall be an error if the value of this operand is negative. If this operand
// is zero, the array shall become empty." The text does not say what an
// operand with X or Z bits does or how the run continues after the error.
module tb;
  int d[];
  int c;
  int n;
  logic [3:0] xs;
  initial begin
    if (!$value$plusargs("case=%d", c)) c = 1;
    n = -1;
    xs = 4'bx01x;
    d = new[3];
    $display("size %0d", d.size());
    d = new[0];
    $display("new[0] size %0d", d.size());
    d = new[3];
    if (c == 1) d = new[n];
    if (c == 2) d = new[n](d);
    if (c == 3) d = new[xs];
    $display("after size %0d", d.size());
    $finish;
  end
endmodule
