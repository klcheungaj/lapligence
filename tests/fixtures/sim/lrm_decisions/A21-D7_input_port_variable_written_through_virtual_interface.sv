// Decision A21-D7: a write through a virtual interface to a variable
// declared as an input port of the interface is a compile error, reported
// at the write. The handle's interface type decides, not the instance it
// holds when the write runs, so the error does not depend on whether the
// write runs. Here `v.a = 1'b1` writes input port variable `a`; the design
// must not run and prints nothing.
//
// IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt L41468-41469,
// L41481): "A continuous assignment shall be implied when a variable is
// connected to an input port declaration. Assignments to variables declared
// as input ports shall be illegal."
`timescale 1ns/1ps
interface ifc(input logic a);
endinterface
class W;
  virtual ifc v;
  task run();
    #1 v.a = 1'b1;
    $display("a=%b", v.a);
  endtask
endclass
module tb;
  logic a = 1'b0;
  ifc i(a);
  initial begin
    static W w = new;
    w.v = i;
    w.run();
    $finish;
  end
endmodule
