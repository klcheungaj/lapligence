// Decision S28-D10: get_randstate and set_randstate through a null class handle
// address the RNG of the calling process; srandom through one is a null access.
//
// IEEE 1800-2009 8.4 (SystemVerilog-1800-2009.txt L9653-9654): "Accessing
//   nonstatic members (see 8.8) or virtual methods (see 8.19) via a null object
//   handle is illegal. The result of an illegal access via a null object is
//   indeterminate, and implementations may issue an error."
// 18.13.4 (L31100-31101): "The get_randstate() method returns a copy of the
//   internal state of the RNG associated with the given object."
//
// The text leaves the result open; llg follows the industry-standard
// behaviour.
module tb;
  class packet;
    rand int v;
  endclass

  packet empty;
  string state;
  int unsigned first, second;

  initial begin
    state = empty.get_randstate();
    first = $urandom;
    empty.set_randstate(state);
    second = $urandom;
    $display("process state restored %b", first == second);
    $display("changed by the process %b", state != empty.get_randstate());
    $finish(0);
  end
endmodule
