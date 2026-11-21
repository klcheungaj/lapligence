// Input port variable `a` as a member of a concatenation target. See readme.md.
`timescale 1ns/1ps
interface ifc(input logic a);
  logic c;
endinterface
class W;
  virtual ifc v;
  task run();
    #1 {v.c, v.a} = 2'b11;
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
