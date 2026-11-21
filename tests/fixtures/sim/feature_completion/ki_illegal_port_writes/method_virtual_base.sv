// Positive: the override that writes `s` is called through a base-class handle. See readme.md.
`timescale 1ns/1ps
module drv(input logic clk, output logic o);
  always @(posedge clk) o <= 1'b0;
endmodule
module tb;
  logic clk = 1'b0;
  logic s;
  drv d(clk, s);
  class D;
    virtual task run();
    endtask
  endclass
  class E extends D;
    virtual task run();
      #12 s = 1'b1;
    endtask
  endclass
  always #5 clk = ~clk;
  initial begin
    static E e = new;
    static D b = e;
    fork b.run(); join_none
    #13 $display("s=%b", s);
    $finish;
  end
endmodule
