// Decision S09-D6: an event control on a member selected through a
// virtual-interface `ref` formal follows the interface the actual variable
// names: after the caller assigns the variable another interface, the
// waiting task waits on that interface's member (and no edge is created by
// the rebinding itself when both members hold the same value).
//
// IEEE 1800-2009 25.9 (SystemVerilog-1800-2009.txt L44868): "A virtual
//   interface is a variable that represents an interface instance."
// IEEE 1800-2009 13.5.2 (L19138-19141): "When the argument is passed by
//   reference, both the caller and the subroutine share the same
//   representation of the argument; therefore, any changes made to the
//   argument, within either the caller or the subroutine, shall be visible to
//   each other."
// IEEE 1800-2009 9.4.2 (L11837-11838): "Changing the value of object data
//   members, aggregate elements, or the size of a dynamically sized array
//   referenced by a method or function shall cause the event expression to be
//   reevaluated."
//
// llg reading (the text does not say how a pending edge control on
// `v.clk` treats a rebinding of `v`; llg re-evaluates the expression through
// the current handle, as it does for class handles).
interface ifc;
  logic clk = 0;
endinterface

module tb;
  ifc i0();
  ifc i1();
  virtual ifc vi;

  task automatic tick(ref virtual ifc v);
    @(posedge v.clk);
    $display("tick at %0d", $time);
  endtask

  initial begin
    vi = i0;
    fork
      tick(vi);
    join_none
    #1 vi = i1;
    #1 i0.clk = 1;
    #1 i1.clk = 1;
    #1 $finish;
  end
endmodule
