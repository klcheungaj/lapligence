`timescale 1ns/1ns
// SV 1800-2009 14.16: an output clockvar bound to a select of a wire drives
// only the selected bits through a clocking driver of its own, resolved with
// the wire's other driver (FIN-001 K2).
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  wire [7:0] w;
  logic [7:0] drv = 8'hzz;
  assign w = drv;
  clocking cb @(posedge clk);
    output x = w[3:0];
  endclocking
  initial begin
    #6 cb.x <= 4'hA;
    #10 $display("%0t w=%h", $time, w);
    drv = 8'h5F;
    #1 $display("%0t w=%h", $time, w);
    drv = 8'h5z;
    #1 $display("%0t w=%h", $time, w);
    #20 $finish;
  end
endmodule
