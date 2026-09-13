module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic data = 0;
  logic out = 0;

  default clocking cb @(posedge clk);
    output #1 out;
  endclocking

  initial begin
    #1 data = 1;
    #3 data = 0;
  end

  initial begin
    #1 cb.out <= ##2 data;
    $display("issued t=%0t out=%0d data=%0d", $time, out, data);
    #2 $display("after t=%0t out=%0d data=%0d", $time, out, data);
    #1 $finish;
  end

  always begin
    #2 clk = ~clk;
    #5 clk = ~clk;
  end
endmodule
