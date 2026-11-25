`timescale 1ns/1ns
interface ifc;
  logic [7:0] d;
endinterface

module tb;
  ifc i0();
  virtual ifc n;
  event e;
  initial begin
    n.d[3:0] <= repeat (2) @e 4'h1;
    $display("issued");
    #5 -> e;
    #1 -> e;
    #5 $display("end");
  end
endmodule
