// Decision SD10-D4: a run-time error that ends the run during static
// initialization runs no final procedure, unlike a $finish or a $fatal there
// (S22-D9).
//
// IEEE 1800-2009 9.2.3 (SystemVerilog-1800-2009.txt L11261):
//   "A final procedure executes when simulation ends due to an explicit or
//   implicit call to $finish."
// IEEE 1800-2009 8.4 (L9653-9654):
//   "The result of an illegal access via a null object is indeterminate, and
//   implementations may issue an error."
module tb;
  class C;
    int x;
  endclass
  C c;
  function int read();
    $display("read");
    return c.x;
  endfunction
  function int later();
    $display("later");
    return 1;
  endfunction
  int v = read();
  int w = later();
  initial $display("initial %0d %0d", v, w);
  final $display("final %0d %0d", v, w);
endmodule
