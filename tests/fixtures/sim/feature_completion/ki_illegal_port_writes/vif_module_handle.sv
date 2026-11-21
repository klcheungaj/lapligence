// A module's handle writes input port variable `a` of an unconnected port.
// See readme.md.
`timescale 1ns/1ps
interface ifc(input logic a);
endinterface
module tb;
  ifc i();
  virtual ifc v;
  initial begin
    v = i;
    #1 v.a = 1'b1;
    $display("a=%b", i.a);
    $finish;
  end
endmodule
