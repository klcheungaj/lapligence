module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic out = 0;

  default clocking cb @(posedge clk);
    output out;
  endclocking

  initial begin
    ##0 cb.out <= 1'b1;
    $display("same t=%0t out=%0d", $time, out);
    #1 $display("first t=%0t out=%0d", $time, out);
    ##0 cb.out <= 1'b0;
    #1 $display("second t=%0t out=%0d", $time, out);
    #3 $display("settled t=%0t out=%0d", $time, out);
    #1 $finish;
  end

  always #2 clk = ~clk;
endmodule
