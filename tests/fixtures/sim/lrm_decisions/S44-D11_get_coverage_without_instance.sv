// Decision S44-D11: get_coverage() called through a null covergroup handle is the
// type coverage: the method is static and needs no instance. A null class
// handle that has to be read to name the covergroup handle is a null access.
//
// IEEE 1800-2009 19.8 (SystemVerilog-1800-2009.txt L33569-33572): "The
//   get_coverage() method returns the cumulative (or type) coverage, which
//   considers the contribution of all instances of a particular coverage item;
//   and it is a static method that is available on both types (via the ::
//   operator) and instances (using the . operator)."
// 8.4 (L9653): "Accessing nonstatic members (see 8.8) or virtual methods (see
//   8.19) via a null object handle is illegal."
//
// Some simulators report a null access.
module tb;
  bit [1:0] value;

  covergroup cg;
    coverpoint value;
  endgroup

  class holder;
    covergroup inner;
      coverpoint value;
    endgroup
    function new();
      inner = new;
    endfunction
  endclass

  cg empty, full;
  holder none;

  initial begin
    $display("no instance %0.2f", empty.get_coverage());
    full = new;
    full.sample();
    value = 1;
    full.sample();
    $display("type %0.2f %b", empty.get_coverage(), empty == null);
    $display("holder %0.2f", none.inner.get_coverage());
    $display("not reached");
  end

  final $display("final");
endmodule
