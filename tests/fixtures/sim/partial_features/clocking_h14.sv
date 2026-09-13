module tb;
  timeunit 1ns;
  timeprecision 1ns;

  logic clk = 0;
  logic input_data = 0;
  logic out_zero = 0;
  logic out_two = 0;
  logic inout_source = 0;
  wire inout_data;
  assign inout_data = inout_source;

  default clocking cb @(posedge clk);
    input #1step input_data;
    output #0 out_zero;
    output #2 out_two;
    inout inout_data;
  endclocking

  initial begin
    #1 input_data = 1;
    ##1 cb.out_zero <= input_data;
    $display("drive1 t=%0t zero=%0d two=%0d", $time, out_zero, out_two);
    ##1 cb.out_two <= 1;
    $display("drive2 t=%0t zero=%0d two=%0d", $time, out_zero, out_two);
    #3 $display("skew t=%0t zero=%0d two=%0d", $time, out_zero, out_two);
    ##2 cb.inout_data <= 1;
    $display("inout t=%0t raw=%0d sample=%0d", $time, inout_data, cb.inout_data);
    #3 $finish;
  end

  always begin
    #2 clk = ~clk;
    #5 clk = ~clk;
  end
endmodule
