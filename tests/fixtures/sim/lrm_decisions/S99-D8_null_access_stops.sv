// Decision S99-D8: an access through a null virtual interface stops the run
// at once: nothing after the failed access runs, including the rest of the
// statement that made it. The display that reads the member prints nothing,
// the statements after it do not run and the final procedure does not run
// either; the run ends with a failure status. An access through a null class
// handle stops the same way but runs the final procedures (S99-D11).
//
// IEEE 1800-2009 25.9 (SystemVerilog-1800-2009.txt L44893-44895): "A virtual
// interface shall be initialized before referencing a component of the
// virtual interface; it has the value null before it is initialized.
// Attempting to use a null virtual interface shall result in a fatal
// run-time error." 8.4 L9653-9654: "Accessing nonstatic members (see 8.8) or
// virtual methods (see 8.19) via a null object handle is illegal. The result
// of an illegal access via a null object is indeterminate, and
// implementations may issue an error." The text does not say whether the
// failing statement completes or whether final procedures run; llg follows
// industry-standard behaviour: the error takes effect at the access and, for
// a virtual interface, ends the run without the final procedures.
interface bus_if;
  logic [7:0] a;
endinterface

module tb;
  bus_if bi();
  virtual bus_if v;
  initial begin
    $display("before");
    $display("in the failing statement %h", v.a);
    $display("after");
    $finish;
  end
  final $display("final");
endmodule
