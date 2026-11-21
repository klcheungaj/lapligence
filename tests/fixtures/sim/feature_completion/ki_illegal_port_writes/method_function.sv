// A class function called in an expression writes `s`. See readme.md.
`timescale 1ns/1ps
module drv(input logic clk, output logic o);
  always @(posedge clk) o <= 1'b0;
endmodule
module tb;
  logic clk = 1'b0;
  logic s;
  drv d(clk, s);
  class D;
    function bit f();
      s = 1'b1;
      return 1'b1;
    endfunction
  endclass
  always #5 clk = ~clk;
  initial begin
    static D h = new;
    fork #12 if (h.f()) $display("called"); join_none
    #13 $display("s=%b", s);
    $finish;
  end
endmodule
