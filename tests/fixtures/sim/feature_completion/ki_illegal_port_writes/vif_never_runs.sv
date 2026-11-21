// Input port variable `a` written in a method nothing calls. See readme.md.
`timescale 1ns/1ps
interface ifc(input logic a);
endinterface
class W;
  virtual ifc v;
  task never();
    v.a = 1'b1;
  endtask
  task run();
    #1 $display("a=%b", v.a);
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
