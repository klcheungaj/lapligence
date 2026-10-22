// Decision S99-D11: an access through a null class handle (a property read,
// here inside a display) is a run-time error that stops the run at once:
// the display that reads the property prints nothing and the statements
// after it do not run. Unlike a null virtual interface access (S99-D8), the
// final procedures still run; the run ends with a failure status.
//
// IEEE 1800-2009 8.4 (SystemVerilog-1800-2009.txt L9653-9654): "Accessing
// nonstatic members (see 8.8) or virtual methods (see 8.19) via a null object
// handle is illegal. The result of an illegal access via a null object is
// indeterminate, and implementations may issue an error." The text does not
// say whether the run continues or whether final procedures run; llg follows
// industry-standard behaviour.
class item;
  int x;
endclass

module tb;
  item c;
  int k = 5;
  initial begin
    $display("before");
    $display("in the failing statement %0d", c.x);
    k = 6;
    $display("after");
    $finish;
  end
  final $display("final k=%0d", k);
endmodule
