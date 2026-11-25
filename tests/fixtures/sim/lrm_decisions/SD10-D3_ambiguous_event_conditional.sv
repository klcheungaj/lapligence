// Decision SD10-D3: a conditional operator with event operands and an
// ambiguous predicate yields the event when both operands name one, and null
// otherwise.
//
// IEEE 1800-2009 11.4.11 (SystemVerilog-1800-2009.txt L15213-15216):
//   "Otherwise, both the first expression and second expression shall be
//   evaluated, and their results shall be combined element by element. If the
//   elements match, the element is returned. If they do not match, then the
//   default-uninitialized value for that element's type shall be returned."
//   Null is the value Table 7-1 (L8753) gives for an event that is not there;
//   Table 6-7 gives a variable a new event. Neither is one of the operands.
module tb;
  event a, b, c;
  logic x;

  initial begin
    c = x ? a : b;
    $display("%b %b", c == a, c == b);
    c = x ? a : a;
    $display("%b", c == a);
    x = 1;
    c = x ? a : b;
    $display("%b", c == a);
    $display("llg %b", (1'bz ? a : b) == null);
    $finish;
  end
endmodule
