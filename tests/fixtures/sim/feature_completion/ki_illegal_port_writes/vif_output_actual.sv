// Input port variable `a` as the output actual of a call through the handle. See readme.md.
`timescale 1ns/1ps
interface ifc(input logic a);
  logic c;
endinterface
class W;
  virtual ifc v;
  task set(output logic o); o = 1'b1; endtask
  task run();
    #1 set(v.a);
    $display("c=%b a=%b", v.c, v.a);
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
