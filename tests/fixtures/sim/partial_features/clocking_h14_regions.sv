module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic out = 1;

  default clocking cb @(posedge clk);
    output #0 out;
  endclocking

  initial begin
    ##1 cb.out <= 1;
    #1 $finish;
  end

  always @(posedge clk) out <= 0;

  always @(out)
    $display("change t=%0t out=%0d", $time, out);

  always #1 clk = ~clk;
endmodule
