module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic data = 0;
  logic sel = 0;
  logic [1:0] out = 0;

  default clocking cb @(posedge clk);
    output #0 out;
  endclocking

  initial begin
    #1 data = 1;
    #2 sel = 1;
  end

  initial begin
    #1 cb.out[sel] <= ##2 data;
    $display("selected issued t=%0t out=%b sel=%0d", $time, out, sel);
    #1 $display("selected after t=%0t out=%b sel=%0d", $time, out, sel);
    #1 $finish;
  end

  always begin
    #2 clk = ~clk;
    #5 clk = ~clk;
  end
endmodule
