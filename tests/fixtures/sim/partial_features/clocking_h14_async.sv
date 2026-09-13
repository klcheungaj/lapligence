module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic data = 0;
  logic out = 0;

  default clocking cb @(posedge clk);
    output #2 out;
  endclocking

  initial begin
    #1 data = 1;
    cb.out <= data;
    #1 $display("pending t=%0t out=%0d data=%0d", $time, out, data);
    #1 $display("edge t=%0t out=%0d data=%0d", $time, out, data);
    #3 $display("drive t=%0t out=%0d data=%0d", $time, out, data);
    #1 $finish;
  end

  always #3 clk = ~clk;
endmodule
