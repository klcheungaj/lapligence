// A class task a process calls writes `s`, the actual of output port `o`. See readme.md.
`timescale 1ns/1ps
module drv(input logic clk, output logic o);
  always @(posedge clk) o <= 1'b0;
endmodule
module tb;
  logic clk = 1'b0;
  logic s;
  drv d(clk, s);
  class D;
    task run();
      #12 s = 1'b1;
    endtask
  endclass
  always #5 clk = ~clk;
  initial begin
    static D h = new;
    fork h.run(); join_none
    #13 $display("s=%b", s);
    $finish;
  end
endmodule
