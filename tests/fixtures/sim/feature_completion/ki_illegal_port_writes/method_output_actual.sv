// A class task a process calls passes `s`, the actual of output port `o`,
// as the output actual of another class task. See readme.md.
`timescale 1ns/1ps
module drv(input logic clk, output logic o);
  always @(posedge clk) o <= 1'b0;
endmodule
module tb;
  logic clk = 1'b0;
  logic s;
  drv d(clk, s);
  class D;
    task set(output logic x);
      x = 1'b1;
    endtask
    task run();
      #12 set(s);
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
