module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic [1:0] out_a = 0;
  logic [1:0] out_b = 0;

  default clocking cb @(posedge clk);
    output #0 out_a;
    output #0 out_b;
  endclocking

  initial begin
    ##1 cb.out_a[0] <= 1'b1;
    ##1 cb.out_b[1:0] <= 2'b10;
    #1 $display("targets t=%0t a=%0d b=%0d", $time, out_a, out_b);
    #1 $finish;
  end

  always #2 clk = ~clk;
endmodule
