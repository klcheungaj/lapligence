module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic data = 0;

  clocking cb_step @(posedge clk);
    input #1step data;
  endclocking

  clocking cb_zero @(posedge clk);
    input #0 data;
  endclocking

  clocking cb_two @(posedge clk);
    input #2 data;
  endclocking

  initial begin
    #2 data = 0;
    #3 data = 1;
    #13 $finish;
  end

  always #5 clk = ~clk;

  always @(posedge clk) begin
    #1 $display("t=%0t raw=%0d step=%0d zero=%0d two=%0d", $time, data,
                cb_step.data, cb_zero.data, cb_two.data);
  end
endmodule
